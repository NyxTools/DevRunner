# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

- Build: `cargo build`
- Run: `cargo run -- [--path <dir>] [--config <file>]`
- Install locally: `cargo install --path .`
- Test: `cargo test` (CI runs this, but the crate currently has zero `#[test]` functions — any test added is the first)
- Lint/format: no `.cargo/config.toml`, clippy config, or rustfmt config present; use `cargo clippy` / `cargo fmt` defaults

CI (`.github/workflows/rust.yml`) runs `cargo build --verbose` and `cargo test --verbose` on push/PR to `main`.

## Architecture

Single Rust binary (`devrunner`), no library crate — all modules are `mod` declarations in [src/main.rs](src/main.rs), not `pub` exports, so nothing here is currently embeddable/reusable outside this bin.

Flow: `main.rs` resolves target dir → `config::load_config` → `scanner::scan_directory` → `app::run_app` (blocks for the life of the TUI).

- **scanner.rs** — walks the target dir (`walkdir`, max depth 3, skips `node_modules`/`target`), finds `package.json` (emits a `Service` per `dev`/`start`/`build` script found) and `Cargo.toml` (emits one `cargo run` service per non-workspace package). This is auto-discovery; it does not know about `custom_scripts` from config — see gap below.
- **config.rs** — loads `.devrunner.json`/`.devrunner.toml` (or explicit `--config` path), trying JSON then TOML. Defines `custom_scripts` and `ignore_paths` fields, but **neither is currently wired into `scanner` or `app`** — the config is loaded in `main.rs` and then discarded (`let _config = ...`). Treat `custom_scripts`/`ignore_paths` as dead/unimplemented, not as working features, unless you're the one wiring them in.
- **models.rs** — `Service` (name, path, project_type, command, runtime-only `status`/`logs` skipped from serde) and `ServiceStatus` (Stopped/Running(pid)/Failed/Completed).
- **process.rs** — `ProcessManager::spawn_service` spawns each service via `cmd /C` (Windows) or `sh -c` (Unix), tees stdout/stderr line-by-line into timestamped log strings, and pushes everything through an `UnboundedSender<Event>`. One `tokio::spawn` per service; stdout/stderr each get their own nested task.
- **events.rs** — the `Event` enum (`Tick`, `Key`, `Mouse`, `ServiceLog`, `ServiceStatus`, `Quit`) is the single channel type connecting input polling, the tick timer, and process output back into the main loop in `app.rs`.
- **app.rs** — owns the `App` state (service list, selection, CPU history) and the actual event loop: a dedicated `std::thread` polls crossterm input, a `tokio::spawn` fires `Tick` every 500ms, and `ProcessManager` events arrive on the same `mpsc` channel. All three producers feed one consumer loop (`rx.recv().await`) that mutates `App` and redraws every iteration. If you add a new async event source, wire it into this same channel rather than creating a parallel one.
- **ui.rs** — pure rendering (`ratatui`): three-column layout (services list / logs / CPU+mem sparkline via `sysinfo`), redrawn from `App` state each loop tick. No logic beyond display lives here.

Only one service can be "selected" and interacted with at a time in the TUI; multiple services can run concurrently in the background but there's no bulk start/stop/kill — only start (Enter/`s`) is wired to a key.

## In-progress direction

See [docs/superpowers/specs/2026-07-26-agent-terminal-engine-design.md](docs/superpowers/specs/2026-07-26-agent-terminal-engine-design.md) for the active design: splitting `ProcessManager`/scanner/config into a `devrunner-core` lib, run as a shared background daemon (`devrunnerd`) over a local socket, with the current TUI, a `--json` CLI, and an MCP server as three thin clients of the same engine. Not yet implemented — current code is still the single-binary TUI described above.
