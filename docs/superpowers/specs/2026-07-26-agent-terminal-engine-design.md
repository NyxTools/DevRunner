# DevRunner: Shared Terminal Engine (Design)

Date: 2026-07-26
Status: Approved for planning

## Context

DevRunner is currently a single-binary Rust TUI (v0.1.1, last touched ~7 months ago)
that scans a project directory (depth 3) for `package.json` and `Cargo.toml`, lists
discovered services, and lets a user start one at a time in a terminal UI with live
log streaming and CPU/mem stats. Config supports `ignore_paths` and `custom_scripts`,
but `custom_scripts` is parsed and never wired into the scanner/app — dead code.
There are zero tests despite CI running `cargo test`.

### Why not just finish the human-only tool

Local task runners for humans are a solved, crowded space (`mprocs`, `overmind`,
`foreman`, `turborepo`, `pm2`, VS Code compound tasks). DevRunner's current TUI is
effectively a smaller `mprocs` clone. Shipping "save your commands, reopen, multi-start"
alone does not clear a differentiation bar.

### The actual gap

Coding agents (Claude Code, Cursor, Codex, autonomous multi-agent frameworks) run
shell commands through one-shot, blocking tool calls with no session concept. Starting
a background dev server and later checking on it is hacked together per-harness with
`nohup ... &`, temp log files, and polling. There is no standard local daemon that:

- exposes a stable process/session control surface (spawn, list, tail logs, status,
  stop, restart)
- lets multiple consumers (a human TUI *and* an agent) attach to the **same** running
  session at once
- persists named project profiles so re-invocation is instant, not a rescan
- returns structured status instead of raw terminal output an agent must scrape

DevRunner's existing `ProcessManager` + `mpsc` event model is structurally already
halfway to this — it just needs decoupling from the TUI's in-process event loop and
exposing over IPC to multiple kinds of clients.

## Goals

- Turn the current TUI-coupled process manager into a shared, persistent local
  daemon that both a human TUI and AI agents can drive.
- Expose an MCP server so any MCP-compatible agent (Claude Code, Cursor, etc.) can
  start/stop/inspect services with zero glue code.
- Add project profiles so a project's service list is saved once and reused, instead
  of rescanned every launch.
- Make `custom_scripts` a live, working feature instead of dead config.

## Non-goals (deferred)

- CI / GitHub Actions local workflow execution (e.g. via `act`). Distinct feature
  with its own design questions (runner choice, sandboxing, output parsing) — spec'd
  separately once the daemon foundation exists, as it can simply become another
  "service type" the engine runs.
- Remote/networked access (daemon is localhost-only, single machine/user).
- Auth/multi-user access control (not needed for a local single-user daemon).

## Architecture

```
┌─────────────┐  ┌──────────────┐  ┌──────────────┐
│   TUI       │  │  CLI (--json)│  │  MCP Server  │
└──────┬──────┘  └──────┬───────┘  └──────┬───────┘
       │   Unix socket / named pipe (IPC)  │
       └─────────────────┬─────────────────┘
                          ▼
                 ┌──────────────────┐
                 │    devrunnerd     │  (auto-spawned, detached)
                 │  ── engine ──     │
                 │  ProcessManager   │
                 │  ProfileStore     │
                 │  SessionRegistry  │
                 └──────────────────┘
                          │
                 spawns/tracks child processes
```

One shared daemon per machine/user (not per-project). Service/session IDs are
namespaced by project path, so an agent working across multiple repos, or a human
with several projects open, share a single daemon — this matches the "terminal
manager" framing: it manages all your terminals, not just one project's.

Every entrypoint — `devrunner` (TUI), `devrunner start api` (CLI), an MCP tool call —
is a thin client: connect to the socket, send a request, get a response or event
stream. The first client to find no daemon running auto-spawns it detached and
retries connecting (no manual `devrunner daemon start` step for humans or agents).

## Components

- **`devrunner-core`** (lib crate) — extracted from current `src/`: `models`,
  `scanner`, `process` (`ProcessManager`), `config`, plus new `profile` and
  `registry` modules. No TUI/CLI/MCP dependencies. The reusable engine.
- **`devrunnerd`** (bin) — owns the socket, holds a `SessionRegistry` (all running
  services across all projects, keyed by `project_path + service_name`), routes IPC
  requests to `ProcessManager`/`ProfileStore`, and fans out log/status events to
  every subscribed client watching a given service.
- **`devrunner-cli`** (bin, current `devrunner`) — `devrunner` with no args launches
  the TUI (auto-spawns daemon if needed, connects, renders). `devrunner start <name>`,
  `devrunner stop <name>`, `devrunner list --json`, `devrunner logs <name> --follow`
  give scriptable/agent-via-shell access without requiring MCP.
- **`devrunner-mcp`** (bin or feature) — MCP server (stdio transport) exposing:
  `list_services`, `start_service`, `stop_service`, `restart_service`, `get_status`,
  `get_logs(tail_n)`, `scan_project(path)`, `save_profile`. Talks to the daemon over
  the same socket as any other client — no special-cased server-side logic.
- **Profiles** — `~/.devrunner/profiles/<project-path-hash>.json`, storing the
  confirmed service list (name, path, command, project_type) plus `ignore_paths`.
  `scan_project` / launching in a project checks for an existing profile first;
  falls back to `scanner::scan_directory` only if missing or `--rescan` is passed.
  Adding/editing a custom command updates the profile — this is what finally makes
  `custom_scripts` live instead of dead config.

## Data flow

1. Client connects to the socket (`%TEMP%\devrunner.sock` on Windows via named pipe,
   `/tmp/devrunner-$UID.sock` on Unix). On connect failure, client spawns
   `devrunnerd --detach` and retries with backoff (max ~2s).
2. Requests/responses are newline-delimited JSON over the socket — simple, debuggable,
   consistent with existing `serde` usage; no need for a heavier RPC framework at this
   scale. Request: `{id, project_path, action, params}`. Response: `{id, result | error}`.
3. Long-lived subscriptions (log tail, status changes) reuse the same connection: a
   client sends `subscribe_logs {service}`; the daemon streams
   `{event: "log", service, line}` / `{event: "status", service, status}` frames until
   the client disconnects or sends `unsubscribe`. This replaces the current in-process
   `mpsc` channel in `app.rs` with the same event shape carried over IPC — the TUI's
   event loop changes very little.
4. The key new behavior: multiple subscribers to the same service. The daemon's
   `SessionRegistry` fans out each event to every subscribed connection, not just one
   — a human's TUI and an agent's MCP session can watch the same dev server at once.

## Error handling

- **Daemon crash/restart**: clients detect a broken pipe/socket and attempt one
  respawn+reconnect. Child processes the daemon spawned are not killed by the
  daemon's death (spawned detached); on restart, the daemon reconciles against a
  small on-disk state file of previously-known PIDs so a restart doesn't silently
  orphan a still-running service.
- **Stale profile**: `scan_project` validates each profile entry's path still
  exists; missing entries are dropped with a warning event, not a hard failure.
- **Command spawn failure**: unchanged from current `process.rs` — emits
  `ServiceStatus::Failed` plus an error log line, now relayed to all subscribed
  clients instead of just the local TUI.
- **Socket collision** (stale socket file from a crashed daemon): on bind failure,
  attempt to connect first to check for a live daemon; if that also fails, remove
  the stale socket file and rebind.

## Testing

- `devrunner-core` gets real unit tests (currently none exist despite CI running
  `cargo test`): scanner parsing against the existing `fixtures/` directory, profile
  load/save round-trip, registry fan-out logic against a fake transport.
- Daemon IPC protocol gets an integration test: spawn a real `devrunnerd` on a temp
  socket path, connect a test client, run a trivial command (`echo` / `cmd /c echo`),
  assert the expected log line and completed status arrive.
- MCP server layer: assert tool schema shape and that each tool call maps to the
  correct IPC request (mock the socket).

## Open questions for implementation planning

- Exact wire format details (field names, versioning) for the IPC protocol.
- Whether `devrunner-mcp` ships as a separate binary or a `--mcp` flag on
  `devrunner-cli`.
- Windows named-pipe crate choice (e.g. `tokio::net::windows::named_pipe` vs. a
  cross-platform IPC crate) to keep parity with the existing Unix socket path.
