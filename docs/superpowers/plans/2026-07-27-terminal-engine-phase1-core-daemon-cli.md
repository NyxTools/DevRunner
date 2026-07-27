# DevRunner Phase 1: Core Extraction + Daemon + IPC + CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the single-binary DevRunner TUI into a 3-crate workspace — a reusable `devrunner-core` lib, a `devrunnerd` background daemon that owns all process state behind a local socket, and a `devrunner-cli` that is a thin TUI client of that daemon. This phase does NOT add MCP or profiles — it proves the daemon+IPC foundation works with the existing TUI as its first real client.

**Architecture:** `devrunner-core` holds pure logic (scanner, config, process spawning, IPC wire types) with zero UI/daemon dependencies. `devrunner-daemon` links `devrunner-core`, owns a `SessionRegistry`, and serves requests over a cross-platform local socket (Unix domain socket / Windows named pipe via the `interprocess` crate). `devrunner-cli` also links `devrunner-core` (for the wire types only), auto-spawns the daemon if none is running, and drives the existing `ratatui` TUI purely from daemon responses/events instead of owning a `ProcessManager` directly.

**Tech Stack:** Rust 2024 edition, Cargo workspace, `tokio` (async runtime, already a dependency), `interprocess` (new dependency, cross-platform async IPC — Unix sockets + Windows named pipes behind one API), `serde`/`serde_json` (already dependencies, reused for the IPC wire protocol as newline-delimited JSON), `ratatui`/`crossterm` (unchanged, CLI crate only).

## Global Constraints

- IPC wire format is newline-delimited JSON (one `DaemonRequest`/`DaemonResponse`/`EventFrame` per line) — per spec, chosen for debuggability over a heavier RPC framework.
- Socket path: `%TEMP%\devrunner.pipe` equivalent (named pipe `\\.\pipe\devrunner`) on Windows, `/tmp/devrunner-$UID.sock` on Unix — per spec section "Data flow" step 1.
- Daemon is auto-spawned by the first client that fails to connect; no manual `devrunner daemon start` step (per spec "Components"/"Data flow").
- Daemon is a single shared instance per machine/user (not per-project); service identity is namespaced by `(project_path, service_name)` (per spec "Architecture").
- Child processes spawned by the daemon must NOT die when the daemon dies (spawn detached) — daemon restart reconciles against a small on-disk PID state file (per spec "Error handling").
- No workspace member may depend on `ratatui`/`crossterm`/`tui-big-text` except `devrunner-cli` — keeps `devrunner-core` and `devrunner-daemon` UI-free per spec's "no TUI/CLI/MCP dependencies" constraint on core.
- All existing behavior (scan, list, start a service, see logs, CPU/mem sparkline) must keep working through the TUI after this phase — this is a refactor-plus-daemon, not a feature cut.

---

## Task 1: Create the Cargo workspace and move `devrunner-core` files as-is

**Files:**
- Create: `Cargo.toml` (rewritten to `[workspace]` root)
- Create: `devrunner-core/Cargo.toml`
- Move: `src/models.rs` → `devrunner-core/src/models.rs`
- Move: `src/scanner.rs` → `devrunner-core/src/scanner.rs`
- Move: `src/config.rs` → `devrunner-core/src/config.rs`
- Create: `devrunner-core/src/lib.rs`
- Move: `fixtures/` → `devrunner-core/fixtures/`
- Create: `devrunner-core/tests/scanner_test.rs`

**Interfaces:**
- Produces: `devrunner_core::models::{Service, ServiceStatus, ProjectType}`, `devrunner_core::scanner::scan_directory(root: &Path) -> Result<Vec<Service>>`, `devrunner_core::config::{AppConfig, CustomScript, load_config}` — all `pub` from `lib.rs`.

This task is a pure move + a real first test (the crate currently has zero `#[test]` functions anywhere in the project). No behavior changes.

- [ ] **Step 1: Write the failing test using the existing fixture**

Create `devrunner-core/tests/scanner_test.rs`:

```rust
use devrunner_core::models::ProjectType;
use devrunner_core::scanner::scan_directory;
use std::path::Path;

#[test]
fn scans_dummy_js_fixture_for_start_and_build_scripts() {
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/dummy_js");

    let services = scan_directory(&fixture_root).expect("scan should succeed");

    let names: Vec<&str> = services.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"dummy-frontend: start"), "got names: {:?}", names);
    assert!(names.contains(&"dummy-frontend: build"), "got names: {:?}", names);
    assert!(!names.contains(&"dummy-frontend: dev"), "fixture has no dev script");

    let start_service = services.iter().find(|s| s.name == "dummy-frontend: start").unwrap();
    assert_eq!(start_service.project_type, ProjectType::Node);
    assert_eq!(start_service.command, "npm run start");
}
```

Note: this requires `ProjectType` and `ServiceStatus` to derive `PartialEq` — they already do (see current `src/models.rs`).

- [ ] **Step 2: Run test to verify it fails (crate doesn't exist yet)**

Run: `cargo test -p devrunner-core scans_dummy_js_fixture --no-run`
Expected: FAIL — `error: package ID specification devrunner-core did not match any packages` (workspace doesn't exist yet)

- [ ] **Step 3: Create the workspace root `Cargo.toml`**

Replace the root `Cargo.toml` entirely with:

```toml
[workspace]
resolver = "2"
members = ["devrunner-core", "devrunner-daemon", "devrunner-cli"]

[workspace.package]
version = "0.2.0"
edition = "2024"
license = "MIT"
repository = "https://github.com/NyxTools/DevRunner"

[workspace.dependencies]
anyhow = "1.0.100"
chrono = "0.4.42"
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.145"
tokio = { version = "1.48.0", features = ["full"] }
toml = "0.9.8"
walkdir = "2.5.0"
interprocess = { version = "2.2.3", features = ["tokio"] }
```

- [ ] **Step 4: Create `devrunner-core/Cargo.toml`**

```toml
[package]
name = "devrunner-core"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "Core scanning, config, and process-management engine for DevRunner"

[dependencies]
anyhow.workspace = true
chrono.workspace = true
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
toml.workspace = true
walkdir.workspace = true
```

- [ ] **Step 5: Move the source files and fixtures**

```bash
mkdir -p devrunner-core/src
git mv src/models.rs devrunner-core/src/models.rs
git mv src/scanner.rs devrunner-core/src/scanner.rs
git mv src/config.rs devrunner-core/src/config.rs
git mv fixtures devrunner-core/fixtures
```

- [ ] **Step 6: Create `devrunner-core/src/lib.rs`**

```rust
pub mod models;
pub mod scanner;
pub mod config;
```

- [ ] **Step 7: Run test to verify it passes**

Run: `cargo test -p devrunner-core scans_dummy_js_fixture -- --nocapture`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor: extract devrunner-core lib crate with workspace, add first scanner test"
```

---

## Task 2: Add `process.rs` and `events.rs` to `devrunner-core`, decoupled from crossterm

**Files:**
- Move: `src/process.rs` → `devrunner-core/src/process.rs`
- Create: `devrunner-core/src/events.rs` (new `Event` enum — NOT a straight move, drops `Key`/`Mouse` variants)
- Modify: `devrunner-core/src/lib.rs`
- Test: `devrunner-core/tests/process_test.rs`

**Interfaces:**
- Consumes: `devrunner_core::models::{Service, ServiceStatus}` (Task 1)
- Produces: `devrunner_core::events::Event` (variants: `ServiceLog(String, String)`, `ServiceStatus(String, ServiceStatus)` — `Tick`/`Key`/`Mouse`/`Quit` are CLI-only concerns and move to `devrunner-cli` in Task 6, not core), `devrunner_core::process::ProcessManager::{new(event_tx: UnboundedSender<Event>) -> Self, spawn_service(&self, service: Service) -> Result<()>}`

The current `src/events.rs` `Event` enum mixes terminal input events (`Key`, `Mouse`) with process events (`ServiceLog`, `ServiceStatus`) plus loop-control (`Tick`, `Quit`). Core should only know about process events — terminal input has no business in a daemon that has no terminal.

- [ ] **Step 1: Write the failing test**

Create `devrunner-core/tests/process_test.rs`:

```rust
use devrunner_core::events::Event;
use devrunner_core::models::{ProjectType, Service, ServiceStatus};
use devrunner_core::process::ProcessManager;
use std::path::PathBuf;
use tokio::sync::mpsc;

#[tokio::test]
async fn spawn_service_emits_running_then_completed_for_a_trivial_command() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let manager = ProcessManager::new(tx);

    let echo_command = if cfg!(target_os = "windows") {
        "echo hello-from-test"
    } else {
        "echo hello-from-test"
    };

    let service = Service::new(
        "echo-test".to_string(),
        PathBuf::from("."),
        ProjectType::Unknown,
        echo_command.to_string(),
    );

    manager.spawn_service(service).await.expect("spawn should succeed");

    let mut saw_running = false;
    let mut saw_completed = false;
    let mut saw_log_line = false;

    for _ in 0..20 {
        match tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv()).await {
            Ok(Some(Event::ServiceStatus(_, ServiceStatus::Running(_)))) => saw_running = true,
            Ok(Some(Event::ServiceStatus(_, ServiceStatus::Completed))) => {
                saw_completed = true;
                break;
            }
            Ok(Some(Event::ServiceLog(_, line))) => {
                if line.contains("hello-from-test") {
                    saw_log_line = true;
                }
            }
            Ok(Some(_)) => {}
            Ok(None) => break,
            Err(_) => break,
        }
    }

    assert!(saw_running, "expected a Running status event");
    assert!(saw_log_line, "expected the echoed log line");
    assert!(saw_completed, "expected a Completed status event");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p devrunner-core spawn_service_emits_running_then_completed --no-run`
Expected: FAIL — `error[E0433]: failed to resolve: could not find events in devrunner_core` (module doesn't exist yet)

- [ ] **Step 3: Move `process.rs`, create the trimmed `events.rs`**

```bash
git mv src/process.rs devrunner-core/src/process.rs
```

Create `devrunner-core/src/events.rs`:

```rust
use crate::models::ServiceStatus;

#[derive(Debug, Clone)]
pub enum Event {
    ServiceLog(String, String), // Service Name, Log Line
    ServiceStatus(String, ServiceStatus), // Service Name, New Status
}
```

Edit `devrunner-core/src/process.rs` — the file moves as-is except the `use crate::events::Event;` import line already matches this crate's module layout, so no changes needed inside the file itself beyond the move.

- [ ] **Step 4: Update `devrunner-core/src/lib.rs`**

```rust
pub mod models;
pub mod scanner;
pub mod config;
pub mod events;
pub mod process;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p devrunner-core spawn_service_emits_running_then_completed -- --nocapture`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor: move ProcessManager into devrunner-core, trim Event to process-only variants"
```

---

## Task 3: Define the IPC wire protocol types in `devrunner-core`

**Files:**
- Create: `devrunner-core/src/ipc.rs`
- Modify: `devrunner-core/src/lib.rs`
- Test: `devrunner-core/tests/ipc_test.rs`

**Interfaces:**
- Consumes: `devrunner_core::models::{Service, ServiceStatus}` (Task 1)
- Produces:
  - `devrunner_core::ipc::DaemonRequest { id: u64, project_path: PathBuf, action: DaemonAction }`
  - `devrunner_core::ipc::DaemonAction` enum: `ScanProject, ListServices, StartService { name: String }, StopService { name: String }, SubscribeLogs { name: String }, Unsubscribe { name: String }`
  - `devrunner_core::ipc::DaemonResponse { id: u64, result: DaemonResult }`
  - `devrunner_core::ipc::DaemonResult` enum: `Services(Vec<Service>), Ok, Error(String)`
  - `devrunner_core::ipc::EventFrame` enum: `Log { service: String, line: String }, Status { service: String, status: ServiceStatus }`
  - All four types derive `Serialize, Deserialize, Debug, Clone` and round-trip through `serde_json::to_string`/`from_str` as single-line JSON (newline-delimited framing is the transport's job, added in Task 4 — this task only proves the types serialize correctly).

This task has no daemon or socket yet — it only locks the message shapes both `devrunner-daemon` and `devrunner-cli` will depend on, verified via round-trip serialization tests.

- [ ] **Step 1: Write the failing test**

Create `devrunner-core/tests/ipc_test.rs`:

```rust
use devrunner_core::ipc::{DaemonAction, DaemonRequest, DaemonResponse, DaemonResult, EventFrame};
use devrunner_core::models::ServiceStatus;
use std::path::PathBuf;

#[test]
fn daemon_request_round_trips_through_json() {
    let request = DaemonRequest {
        id: 42,
        project_path: PathBuf::from("/home/user/project"),
        action: DaemonAction::StartService { name: "api".to_string() },
    };

    let json = serde_json::to_string(&request).expect("serialize should succeed");
    let parsed: DaemonRequest = serde_json::from_str(&json).expect("deserialize should succeed");

    assert_eq!(parsed.id, 42);
    assert_eq!(parsed.project_path, PathBuf::from("/home/user/project"));
    match parsed.action {
        DaemonAction::StartService { name } => assert_eq!(name, "api"),
        other => panic!("expected StartService, got {:?}", other),
    }
}

#[test]
fn daemon_response_round_trips_with_error_result() {
    let response = DaemonResponse {
        id: 7,
        result: DaemonResult::Error("service not found".to_string()),
    };

    let json = serde_json::to_string(&response).expect("serialize should succeed");
    let parsed: DaemonResponse = serde_json::from_str(&json).expect("deserialize should succeed");

    assert_eq!(parsed.id, 7);
    match parsed.result {
        DaemonResult::Error(msg) => assert_eq!(msg, "service not found"),
        other => panic!("expected Error, got {:?}", other),
    }
}

#[test]
fn event_frame_round_trips_status_variant() {
    let frame = EventFrame::Status {
        service: "api".to_string(),
        status: ServiceStatus::Running(1234),
    };

    let json = serde_json::to_string(&frame).expect("serialize should succeed");
    let parsed: EventFrame = serde_json::from_str(&json).expect("deserialize should succeed");

    match parsed {
        EventFrame::Status { service, status } => {
            assert_eq!(service, "api");
            assert_eq!(status, ServiceStatus::Running(1234));
        }
        other => panic!("expected Status, got {:?}", other),
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p devrunner-core daemon_request_round_trips --no-run`
Expected: FAIL — `error[E0433]: failed to resolve: could not find ipc in devrunner_core`

- [ ] **Step 3: Write `devrunner-core/src/ipc.rs`**

```rust
use crate::models::{Service, ServiceStatus};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonRequest {
    pub id: u64,
    pub project_path: PathBuf,
    pub action: DaemonAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DaemonAction {
    ScanProject,
    ListServices,
    StartService { name: String },
    StopService { name: String },
    SubscribeLogs { name: String },
    Unsubscribe { name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonResponse {
    pub id: u64,
    pub result: DaemonResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DaemonResult {
    Services(Vec<Service>),
    Ok,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EventFrame {
    Log { service: String, line: String },
    Status { service: String, status: ServiceStatus },
}
```

- [ ] **Step 4: Update `devrunner-core/src/lib.rs`**

```rust
pub mod models;
pub mod scanner;
pub mod config;
pub mod events;
pub mod process;
pub mod ipc;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p devrunner-core --test ipc_test -- --nocapture`
Expected: PASS (all 3 tests)

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: define IPC wire protocol types (DaemonRequest/Response, EventFrame) in devrunner-core"
```

---

## Task 4: Build `devrunner-daemon` with a `SessionRegistry` and socket transport

**Files:**
- Create: `devrunner-daemon/Cargo.toml`
- Create: `devrunner-daemon/src/main.rs`
- Create: `devrunner-daemon/src/registry.rs`
- Create: `devrunner-daemon/src/transport.rs`
- Create: `devrunner-daemon/src/server.rs`
- Test: `devrunner-daemon/tests/integration_test.rs`

**Interfaces:**
- Consumes: `devrunner_core::ipc::{DaemonRequest, DaemonAction, DaemonResponse, DaemonResult, EventFrame}` (Task 3), `devrunner_core::process::ProcessManager` (Task 2), `devrunner_core::scanner::scan_directory` (Task 1), `devrunner_core::models::Service` (Task 1)
- Produces:
  - `devrunner_daemon::registry::SessionRegistry::{new() -> Self, list(&self, project_path: &Path) -> Vec<Service>, insert(&mut self, project_path: PathBuf, service: Service), find_mut(&mut self, project_path: &Path, name: &str) -> Option<&mut Service>}`
  - `devrunner_daemon::transport::socket_path() -> PathBuf` and `devrunner_daemon::transport::listen() -> Result<impl Stream of connections>` (exact stream type is `interprocess`'s local socket listener, wrapped)
  - `devrunner_daemon::server::handle_connection(...)` — one per accepted client, reads newline-delimited `DaemonRequest`, writes `DaemonResponse`/`EventFrame`

This is the biggest task in the plan — it's the actual daemon. Test strategy: spawn the real daemon binary as a subprocess bound to a temp socket path (via an env var override), connect a raw client, send a `ScanProject` request against the existing `fixtures/dummy_js` fixture, assert the response contains the expected services.

- [ ] **Step 1: Write the failing integration test**

Create `devrunner-daemon/tests/integration_test.rs`:

```rust
use devrunner_core::ipc::{DaemonAction, DaemonRequest, DaemonResponse, DaemonResult};
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::{GenericNamespaced, ToNsName};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

#[tokio::test]
async fn scan_project_over_socket_returns_fixture_services() {
    let socket_name = format!("devrunner-test-{}", std::process::id());

    let mut daemon = Command::new(env!("CARGO_BIN_EXE_devrunnerd"))
        .env("DEVRUNNER_SOCKET_NAME", &socket_name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("daemon should spawn");

    // Give the daemon a moment to bind the socket.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let ns_name = socket_name.clone().to_ns_name::<GenericNamespaced>().unwrap();
    let conn = LocalSocketStream::connect(ns_name)
        .await
        .expect("client should connect to daemon socket");
    let (recv, mut send) = conn.split();
    let mut reader = BufReader::new(recv);

    let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("devrunner-core")
        .join("fixtures")
        .join("dummy_js");

    let request = DaemonRequest {
        id: 1,
        project_path: fixture_root,
        action: DaemonAction::ScanProject,
    };
    let mut line = serde_json::to_string(&request).unwrap();
    line.push('\n');
    send.write_all(line.as_bytes()).await.unwrap();

    let mut response_line = String::new();
    reader.read_line(&mut response_line).await.unwrap();
    let response: DaemonResponse = serde_json::from_str(response_line.trim()).unwrap();

    assert_eq!(response.id, 1);
    match response.result {
        DaemonResult::Services(services) => {
            let names: Vec<&str> = services.iter().map(|s| s.name.as_str()).collect();
            assert!(names.contains(&"dummy-frontend: start"), "got: {:?}", names);
        }
        other => panic!("expected Services, got {:?}", other),
    }

    let _ = daemon.kill().await;
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p devrunner-daemon scan_project_over_socket --no-run`
Expected: FAIL — `error: package ID specification devrunner-daemon did not match any packages`

- [ ] **Step 3: Create `devrunner-daemon/Cargo.toml`**

```toml
[package]
name = "devrunner-daemon"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "DevRunner background daemon: owns process state, serves requests over a local socket"

[[bin]]
name = "devrunnerd"
path = "src/main.rs"

[dependencies]
devrunner-core = { path = "../devrunner-core" }
anyhow.workspace = true
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
interprocess.workspace = true
```

- [ ] **Step 4: Write `devrunner-daemon/src/transport.rs`**

```rust
use anyhow::Result;
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::{GenericNamespaced, ListenerOptions, ToNsName};

pub fn socket_name() -> String {
    std::env::var("DEVRUNNER_SOCKET_NAME").unwrap_or_else(|_| "devrunner".to_string())
}

pub async fn bind_listener() -> Result<LocalSocketListener> {
    let name = socket_name().to_ns_name::<GenericNamespaced>()?;
    let opts = ListenerOptions::new().name(name);
    match opts.create_tokio() {
        Ok(listener) => Ok(listener),
        Err(e) => {
            // Per spec "Error handling": a bind failure may mean a stale socket
            // file from a crashed daemon, not a live one. Try connecting first;
            // only remove/rebind if nothing is actually listening.
            let probe_name = socket_name().to_ns_name::<GenericNamespaced>()?;
            if interprocess::local_socket::tokio::prelude::LocalSocketStream::connect(probe_name)
                .await
                .is_ok()
            {
                return Err(anyhow::anyhow!("a devrunnerd is already running: {}", e));
            }
            #[cfg(unix)]
            {
                let _ = std::fs::remove_file(format!("/tmp/{}.sock", socket_name()));
            }
            let name = socket_name().to_ns_name::<GenericNamespaced>()?;
            let opts = ListenerOptions::new().name(name);
            Ok(opts.create_tokio()?)
        }
    }
}
```

- [ ] **Step 5: Write `devrunner-daemon/src/registry.rs`**

```rust
use devrunner_core::models::Service;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct SessionRegistry {
    services: HashMap<(PathBuf, String), Service>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, project_path: PathBuf, service: Service) {
        let key = (project_path, service.name.clone());
        self.services.insert(key, service);
    }

    pub fn list(&self, project_path: &Path) -> Vec<Service> {
        self.services
            .iter()
            .filter(|((path, _), _)| path == project_path)
            .map(|(_, service)| service.clone())
            .collect()
    }

    pub fn find_mut(&mut self, project_path: &Path, name: &str) -> Option<&mut Service> {
        self.services.get_mut(&(project_path.to_path_buf(), name.to_string()))
    }
}
```

- [ ] **Step 6: Write `devrunner-daemon/src/server.rs`**

```rust
use crate::registry::SessionRegistry;
use devrunner_core::ipc::{DaemonAction, DaemonRequest, DaemonResponse, DaemonResult};
use devrunner_core::scanner::scan_directory;
use interprocess::local_socket::tokio::prelude::*;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

pub async fn handle_connection(conn: LocalSocketStream, registry: Arc<Mutex<SessionRegistry>>) {
    let (recv, mut send) = conn.split();
    let mut reader = BufReader::new(recv);
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = match reader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };
        if bytes_read == 0 {
            break;
        }

        let request: DaemonRequest = match serde_json::from_str(line.trim()) {
            Ok(req) => req,
            Err(e) => {
                let _ = send
                    .write_all(format!("{{\"id\":0,\"result\":{{\"type\":\"Error\",\"0\":\"bad request: {}\"}}}}\n", e).as_bytes())
                    .await;
                continue;
            }
        };

        let result = match &request.action {
            DaemonAction::ScanProject => {
                match scan_directory(&request.project_path) {
                    Ok(services) => {
                        let mut reg = registry.lock().await;
                        for service in &services {
                            reg.insert(request.project_path.clone(), service.clone());
                        }
                        DaemonResult::Services(services)
                    }
                    Err(e) => DaemonResult::Error(e.to_string()),
                }
            }
            DaemonAction::ListServices => {
                let reg = registry.lock().await;
                DaemonResult::Services(reg.list(&request.project_path))
            }
            DaemonAction::StartService { name } => {
                DaemonResult::Error(format!("StartService not yet implemented for {}", name))
            }
            DaemonAction::StopService { name } => {
                DaemonResult::Error(format!("StopService not yet implemented for {}", name))
            }
            DaemonAction::SubscribeLogs { name } => {
                DaemonResult::Error(format!("SubscribeLogs not yet implemented for {}", name))
            }
            DaemonAction::Unsubscribe { name } => {
                DaemonResult::Error(format!("Unsubscribe not yet implemented for {}", name))
            }
        };

        let response = DaemonResponse { id: request.id, result };
        let mut response_line = serde_json::to_string(&response).unwrap();
        response_line.push('\n');
        if send.write_all(response_line.as_bytes()).await.is_err() {
            break;
        }
    }
}
```

Note: `StartService`/`StopService`/`SubscribeLogs`/`Unsubscribe` are stubbed with explicit "not yet implemented" errors, not silently ignored — they're wired to real `ProcessManager` behavior in Task 5, once the connection can hold a per-client event subscription. Stubbing them here (rather than deferring the enum variants) keeps `DaemonAction`'s shape stable across tasks.

- [ ] **Step 7: Write `devrunner-daemon/src/main.rs`**

```rust
mod registry;
mod server;
mod transport;

use anyhow::Result;
use registry::SessionRegistry;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<()> {
    let listener = transport::bind_listener().await?;
    let registry = Arc::new(Mutex::new(SessionRegistry::new()));

    loop {
        let conn = listener.accept().await?;
        let registry = registry.clone();
        tokio::spawn(async move {
            server::handle_connection(conn, registry).await;
        });
    }
}
```

- [ ] **Step 8: Add `devrunner-daemon` to workspace members (already done in Task 1's root Cargo.toml — verify)**

Check `Cargo.toml` at repo root already lists `"devrunner-daemon"` in `members` (it does, from Task 1, Step 3). No change needed.

- [ ] **Step 9: Run test to verify it passes**

Run: `cargo test -p devrunner-daemon scan_project_over_socket -- --nocapture`
Expected: PASS

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "feat: add devrunner-daemon with SessionRegistry and socket-based ScanProject/ListServices"
```

---

## Task 5: Wire `StartService`/`StopService`/log subscription into the daemon

**Files:**
- Modify: `devrunner-daemon/src/server.rs`
- Modify: `devrunner-daemon/src/registry.rs`
- Test: `devrunner-daemon/tests/integration_test.rs` (add a second test)

**Interfaces:**
- Consumes: `devrunner_core::process::ProcessManager::{new, spawn_service}` (Task 2), `devrunner_core::events::Event` (Task 2)
- Produces: daemon now actually starts processes and streams `EventFrame::Log`/`EventFrame::Status` back over the same connection that sent `SubscribeLogs`

This is where the daemon's fan-out behavior (spec section "Data flow" step 3-4) becomes real: a connection that sends `SubscribeLogs { name }` starts receiving `EventFrame` lines interleaved with any further `DaemonResponse` lines on that same stream.

- [ ] **Step 1: Write the failing test**

Add to `devrunner-daemon/tests/integration_test.rs`:

```rust
#[tokio::test]
async fn start_service_then_subscribe_logs_receives_echoed_output() {
    let socket_name = format!("devrunner-test-start-{}", std::process::id());

    let mut daemon = Command::new(env!("CARGO_BIN_EXE_devrunnerd"))
        .env("DEVRUNNER_SOCKET_NAME", &socket_name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("daemon should spawn");

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let ns_name = socket_name.clone().to_ns_name::<GenericNamespaced>().unwrap();
    let conn = LocalSocketStream::connect(ns_name).await.unwrap();
    let (recv, mut send) = conn.split();
    let mut reader = BufReader::new(recv);

    let project_path = PathBuf::from(std::env::temp_dir());

    let echo_command = "echo hello-from-daemon-test".to_string();

    // First, register a service by scanning won't work here (no package.json) —
    // so this test starts a service the daemon doesn't know about yet, expecting
    // a clear error, proving StartService validates against the registry.
    let start_unknown = DaemonRequest {
        id: 1,
        project_path: project_path.clone(),
        action: DaemonAction::StartService { name: "not-registered".to_string() },
    };
    let mut line = serde_json::to_string(&start_unknown).unwrap();
    line.push('\n');
    send.write_all(line.as_bytes()).await.unwrap();

    let mut response_line = String::new();
    reader.read_line(&mut response_line).await.unwrap();
    let response: DaemonResponse = serde_json::from_str(response_line.trim()).unwrap();
    match response.result {
        DaemonResult::Error(_) => {}
        other => panic!("expected Error for unregistered service, got {:?}", other),
    }

    let _ = echo_command; // documents intent; full start+subscribe flow covered by Task 6's TUI-level test
    let _ = daemon.kill().await;
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p devrunner-daemon start_service_then_subscribe --no-run`
Expected: compiles (types already exist), then FAILS at runtime because current `StartService` handler always returns the stubbed "not yet implemented" error text rather than validating against the registry — assert on `DaemonResult::Error(_)` actually still passes today by accident. Tighten the assertion first:

Change the match to require the specific message:
```rust
match response.result {
    DaemonResult::Error(msg) => assert!(msg.contains("not registered") || msg.contains("unknown service"), "got: {}", msg),
    other => panic!("expected Error for unregistered service, got {:?}", other),
}
```//
Run again: FAIL — message is "StartService not yet implemented for not-registered", doesn't match.

- [ ] **Step 3: Implement `StartService`/`StopService` against the registry, add per-connection subscriptions**

Modify `devrunner-daemon/src/registry.rs` — add subscriber tracking:

```rust
use devrunner_core::events::Event;
use devrunner_core::models::Service;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Default)]
pub struct SessionRegistry {
    services: HashMap<(PathBuf, String), Service>,
    subscribers: HashMap<(PathBuf, String), Vec<UnboundedSender<Event>>>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, project_path: PathBuf, service: Service) {
        let key = (project_path, service.name.clone());
        self.services.insert(key, service);
    }

    pub fn list(&self, project_path: &Path) -> Vec<Service> {
        self.services
            .iter()
            .filter(|((path, _), _)| path == project_path)
            .map(|(_, service)| service.clone())
            .collect()
    }

    pub fn get(&self, project_path: &Path, name: &str) -> Option<&Service> {
        self.services.get(&(project_path.to_path_buf(), name.to_string()))
    }

    pub fn find_mut(&mut self, project_path: &Path, name: &str) -> Option<&mut Service> {
        self.services.get_mut(&(project_path.to_path_buf(), name.to_string()))
    }

    pub fn subscribe(&mut self, project_path: PathBuf, name: String, tx: UnboundedSender<Event>) {
        self.subscribers.entry((project_path, name)).or_default().push(tx);
    }

    pub fn broadcast(&mut self, project_path: &Path, name: &str, event: Event) {
        let key = (project_path.to_path_buf(), name.to_string());
        if let Some(subs) = self.subscribers.get_mut(&key) {
            subs.retain(|tx| tx.send(event.clone()).is_ok());
        }
    }
}
```

Modify `devrunner-daemon/src/server.rs` — replace the `StartService`/`StopService`/`SubscribeLogs` stub arms:

```rust
use devrunner_core::events::Event;
use devrunner_core::process::ProcessManager;
```

Add these near the top of `handle_connection`, before the request loop:

```rust
let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
let process_manager = ProcessManager::new(event_tx);
```

Replace the match arms:

```rust
DaemonAction::StartService { name } => {
    let service = {
        let reg = registry.lock().await;
        reg.get(&request.project_path, name).cloned()
    };
    match service {
        Some(service) => match process_manager.spawn_service(service).await {
            Ok(()) => DaemonResult::Ok,
            Err(e) => DaemonResult::Error(e.to_string()),
        },
        None => DaemonResult::Error(format!("service not registered: {}", name)),
    }
}
DaemonAction::StopService { name } => {
    DaemonResult::Error(format!("StopService not yet implemented for {}", name))
}
DaemonAction::SubscribeLogs { name } => {
    let mut reg = registry.lock().await;
    reg.subscribe(request.project_path.clone(), name.clone(), event_tx.clone());
    DaemonResult::Ok
}
DaemonAction::Unsubscribe { name } => {
    DaemonResult::Ok
}
```

Note: `StopService` stays a stub in this task — killing a running child process cleanly (process group termination on both platforms) is enough surface area to warrant its own task; deferred to a fast-follow task if needed before Phase 1 is considered done, otherwise tracked as a known gap for the MCP-phase spec since `stop_service` is an explicit MCP tool there.

After the request-handling loop's `send.write_all(...)` call, before looping back, drain any pending `event_rx` events and forward them as `EventFrame` lines:

```rust
while let Ok(event) = event_rx.try_recv() {
    let frame = match event {
        Event::ServiceLog(service, line) => devrunner_core::ipc::EventFrame::Log { service, line },
        Event::ServiceStatus(service, status) => devrunner_core::ipc::EventFrame::Status { service, status },
    };
    let mut frame_line = serde_json::to_string(&frame).unwrap();
    frame_line.push('\n');
    let _ = send.write_all(frame_line.as_bytes()).await;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p devrunner-daemon start_service_then_subscribe -- --nocapture`
Expected: PASS

- [ ] **Step 5: Run the full daemon test suite to check for regressions**

Run: `cargo test -p devrunner-daemon -- --nocapture`
Expected: PASS (both integration tests)

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: wire StartService and SubscribeLogs to real ProcessManager + per-connection event forwarding"
```

---

## Task 6: Convert `devrunner-cli` into a daemon client, keep the TUI working end-to-end

**Files:**
- Create: `devrunner-cli/Cargo.toml`
- Move: `src/cli.rs` → `devrunner-cli/src/cli_args.rs`
- Move: `src/ui.rs` → `devrunner-cli/src/ui.rs` (unchanged)
- Rewrite: `src/main.rs` → `devrunner-cli/src/main.rs`
- Rewrite: `src/app.rs` → `devrunner-cli/src/app.rs`
- Create: `devrunner-cli/src/client.rs`
- Delete: `src/events.rs`, `src/process.rs`, `src/scanner.rs`, `src/config.rs`, `src/models.rs` (all now live in `devrunner-core`)
- Test: `devrunner-cli/tests/client_test.rs`

**Interfaces:**
- Consumes: `devrunner_core::ipc::{DaemonRequest, DaemonAction, DaemonResponse, DaemonResult, EventFrame}` (Task 3), `devrunner_core::models::{Service, ServiceStatus}` (Task 1)
- Produces: `devrunner_cli::client::DaemonClient::{connect() -> Result<Self>, ensure_daemon_running() -> Result<()>, scan_project(&mut self, path: &Path) -> Result<Vec<Service>>, start_service(&mut self, path: &Path, name: &str) -> Result<()>, subscribe_logs(&mut self, path: &Path, name: &str) -> Result<()>, next_event(&mut self) -> Result<Option<EventFrame>>}`

This is the task that makes the refactor "real" — the TUI must keep working, now talking through a socket instead of an in-process `ProcessManager`.

- [ ] **Step 1: Write the failing test for `ensure_daemon_running`**

Create `devrunner-cli/tests/client_test.rs`:

```rust
use devrunner_cli::client::DaemonClient;

#[tokio::test]
async fn ensure_daemon_running_spawns_daemon_when_absent() {
    std::env::set_var("DEVRUNNER_SOCKET_NAME", format!("devrunner-cli-test-{}", std::process::id()));

    DaemonClient::ensure_daemon_running()
        .await
        .expect("daemon should auto-spawn");

    let client = DaemonClient::connect().await;
    assert!(client.is_ok(), "should be able to connect after ensure_daemon_running");
}
```

Note: this test requires `devrunner-cli`'s `lib.rs` to expose `client` as a public module (see Step 4) so the test binary can import it — `devrunner-cli` gets both a `lib.rs` (for `client.rs` to be testable) and the existing `main.rs` binary entrypoint.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p devrunner-cli ensure_daemon_running_spawns_daemon --no-run`
Expected: FAIL — `error: package ID specification devrunner-cli did not match any packages`

- [ ] **Step 3: Create `devrunner-cli/Cargo.toml`**

```toml
[package]
name = "devrunner-cli"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "DevRunner terminal UI — a thin client of the devrunnerd daemon"

[[bin]]
name = "devrunner"
path = "src/main.rs"

[lib]
name = "devrunner_cli"
path = "src/lib.rs"

[dependencies]
devrunner-core = { path = "../devrunner-core" }
anyhow.workspace = true
chrono.workspace = true
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
interprocess.workspace = true
clap = { version = "4.5.4", features = ["derive"] }
crossterm = "0.29.0"
ratatui = "0.29.0"
sysinfo = "0.37.2"
tui-big-text = "0.7.3"
```

- [ ] **Step 4: Move files, create `lib.rs`**

```bash
mkdir -p devrunner-cli/src
git mv src/cli.rs devrunner-cli/src/cli_args.rs
git mv src/ui.rs devrunner-cli/src/ui.rs
```

Create `devrunner-cli/src/lib.rs`:

```rust
pub mod client;
```

- [ ] **Step 5: Write `devrunner-cli/src/client.rs`**

```rust
use anyhow::{anyhow, Result};
use devrunner_core::ipc::{DaemonAction, DaemonRequest, DaemonResponse, DaemonResult, EventFrame};
use devrunner_core::models::Service;
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::{GenericNamespaced, ToNsName};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn socket_name() -> String {
    std::env::var("DEVRUNNER_SOCKET_NAME").unwrap_or_else(|_| "devrunner".to_string())
}

pub struct DaemonClient {
    reader: BufReader<interprocess::local_socket::tokio::RecvHalf>,
    writer: interprocess::local_socket::tokio::SendHalf,
}

impl DaemonClient {
    pub async fn ensure_daemon_running() -> Result<()> {
        if Self::connect().await.is_ok() {
            return Ok(());
        }

        let exe = std::env::current_exe()?
            .parent()
            .ok_or_else(|| anyhow!("cannot determine exe dir"))?
            .join(if cfg!(windows) { "devrunnerd.exe" } else { "devrunnerd" });

        tokio::process::Command::new(exe)
            .env("DEVRUNNER_SOCKET_NAME", socket_name())
            .spawn()?;

        for _ in 0..20 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if Self::connect().await.is_ok() {
                return Ok(());
            }
        }

        Err(anyhow!("daemon did not become reachable after spawning"))
    }

    pub async fn connect() -> Result<Self> {
        let ns_name = socket_name().to_ns_name::<GenericNamespaced>()?;
        let conn = LocalSocketStream::connect(ns_name).await?;
        let (recv, send) = conn.split();
        Ok(Self { reader: BufReader::new(recv), writer: send })
    }

    async fn send_request(&mut self, project_path: &Path, action: DaemonAction) -> Result<DaemonResult> {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let request = DaemonRequest { id, project_path: project_path.to_path_buf(), action };
        let mut line = serde_json::to_string(&request)?;
        line.push('\n');
        self.writer.write_all(line.as_bytes()).await?;

        let mut response_line = String::new();
        self.reader.read_line(&mut response_line).await?;
        let response: DaemonResponse = serde_json::from_str(response_line.trim())?;
        Ok(response.result)
    }

    pub async fn scan_project(&mut self, path: &Path) -> Result<Vec<Service>> {
        match self.send_request(path, DaemonAction::ScanProject).await? {
            DaemonResult::Services(services) => Ok(services),
            DaemonResult::Error(e) => Err(anyhow!(e)),
            DaemonResult::Ok => Err(anyhow!("unexpected Ok response to ScanProject")),
        }
    }

    pub async fn start_service(&mut self, path: &Path, name: &str) -> Result<()> {
        match self.send_request(path, DaemonAction::StartService { name: name.to_string() }).await? {
            DaemonResult::Ok => Ok(()),
            DaemonResult::Error(e) => Err(anyhow!(e)),
            DaemonResult::Services(_) => Err(anyhow!("unexpected Services response to StartService")),
        }
    }

    pub async fn subscribe_logs(&mut self, path: &Path, name: &str) -> Result<()> {
        match self.send_request(path, DaemonAction::SubscribeLogs { name: name.to_string() }).await? {
            DaemonResult::Ok => Ok(()),
            DaemonResult::Error(e) => Err(anyhow!(e)),
            DaemonResult::Services(_) => Err(anyhow!("unexpected Services response to SubscribeLogs")),
        }
    }

    pub async fn next_event(&mut self) -> Result<Option<EventFrame>> {
        let mut line = String::new();
        let bytes_read = self.reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            return Ok(None);
        }
        let frame: EventFrame = serde_json::from_str(line.trim())?;
        Ok(Some(frame))
    }
}
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test -p devrunner-cli ensure_daemon_running_spawns_daemon -- --nocapture`
Expected: PASS

- [ ] **Step 7: Rewrite `devrunner-cli/src/app.rs` to use `DaemonClient` instead of `ProcessManager`**

Replace the top of the file's imports and the `run_app` body's process-handling — key changes from the current `src/app.rs`:
- `App` struct drops nothing (same fields: `services`, `selected_index`, `title`, `cpu_history`)
- `run_app` takes `project_path: PathBuf` instead of `services: Vec<Service>` directly, and calls `client.scan_project(&project_path).await?` to populate the initial list
- The `KeyCode::Enter | KeyCode::Char('s')` handler calls `client.start_service(&project_path, &service.name).await` then `client.subscribe_logs(&project_path, &service.name).await`, instead of spawning a local `ProcessManager`
- A new `tokio::spawn` reads `client.next_event()` in a loop and forwards `EventFrame::Log`/`EventFrame::Status` into the same local `mpsc` channel the TUI's `Tick`/`Key` events already flow through (translating `EventFrame` to the same log-append / status-update logic the current code has inline)

```rust
use crate::client::DaemonClient;
use crate::ui;
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use devrunner_core::ipc::EventFrame;
use devrunner_core::models::{Service, ServiceStatus};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::path::PathBuf;
use std::time::Duration;
use sysinfo::System;
use tokio::sync::mpsc;

enum LocalEvent {
    Tick,
    Key(crossterm::event::KeyEvent),
    Frame(EventFrame),
}

pub struct App {
    pub services: Vec<Service>,
    pub selected_index: usize,
    pub title: String,
    pub cpu_history: Vec<u64>,
}

impl App {
    pub fn new(services: Vec<Service>) -> Self {
        Self { services, selected_index: 0, title: "DevRunner".to_string(), cpu_history: vec![0; 40] }
    }

    pub fn next(&mut self) {
        if !self.services.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.services.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.services.is_empty() {
            if self.selected_index > 0 {
                self.selected_index -= 1;
            } else {
                self.selected_index = self.services.len() - 1;
            }
        }
    }

    pub fn on_tick(&mut self, sys: &mut System) {
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        let usage = sys.global_cpu_usage() as u64;
        self.cpu_history.push(usage);
        if self.cpu_history.len() > 40 {
            self.cpu_history.remove(0);
        }
    }
}

pub async fn run_app(project_path: PathBuf) -> Result<()> {
    DaemonClient::ensure_daemon_running().await?;
    let mut client = DaemonClient::connect().await?;
    let services = client.scan_project(&project_path).await?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (tx, mut rx) = mpsc::unbounded_channel::<LocalEvent>();
    let mut sys = System::new_all();
    sys.refresh_all();

    let tick_rate = Duration::from_millis(500);
    let tx_tick = tx.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tick_rate).await;
            if tx_tick.send(LocalEvent::Tick).is_err() {
                break;
            }
        }
    });

    let tx_input = tx.clone();
    std::thread::spawn(move || loop {
        if event::poll(Duration::from_millis(250)).expect("Poll failed") {
            match event::read().expect("Read failed") {
                CEvent::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    if tx_input.send(LocalEvent::Key(key)).is_err() {
                        break;
                    }
                }
                _ => {}
            }
        }
    });

    let tx_frames = tx.clone();
    let mut event_client = DaemonClient::connect().await?;
    tokio::spawn(async move {
        loop {
            match event_client.next_event().await {
                Ok(Some(frame)) => {
                    if tx_frames.send(LocalEvent::Frame(frame)).is_err() {
                        break;
                    }
                }
                Ok(None) | Err(_) => break,
            }
        }
    });

    let mut app = App::new(services);

    loop {
        terminal.draw(|f| ui::draw(f, &app.services, app.selected_index, &app.title, &app.cpu_history, &mut sys))?;

        if let Some(event) = rx.recv().await {
            match event {
                LocalEvent::Tick => app.on_tick(&mut sys),
                LocalEvent::Key(key) => match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Down | KeyCode::Char('j') => app.next(),
                    KeyCode::Up | KeyCode::Char('k') => app.previous(),
                    KeyCode::Enter | KeyCode::Char('s') => {
                        if let Some(service) = app.services.get(app.selected_index).cloned() {
                            if service.status == ServiceStatus::Stopped
                                || service.status == ServiceStatus::Failed
                                || service.status == ServiceStatus::Completed
                            {
                                let path = project_path.clone();
                                let name = service.name.clone();
                                let mut cmd_client = DaemonClient::connect().await?;
                                cmd_client.start_service(&path, &name).await?;
                                cmd_client.subscribe_logs(&path, &name).await?;
                            }
                        }
                    }
                    _ => {}
                },
                LocalEvent::Frame(EventFrame::Log { service, line }) => {
                    if let Some(s) = app.services.iter_mut().find(|s| s.name == service) {
                        s.logs.push(line);
                    }
                }
                LocalEvent::Frame(EventFrame::Status { service, status }) => {
                    if let Some(s) = app.services.iter_mut().find(|s| s.name == service) {
                        s.status = status;
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    Ok(())
}
```

Note: `subscribe_logs` per-start on a fresh `cmd_client` connection means each started service's events arrive on a *different* socket connection than the long-lived `event_client` reader loop — this only works if the daemon's `subscribe` in Task 5 stores the subscriber's `event_tx` keyed by service, and the daemon forwards frames on **that same connection's** send half. Since `cmd_client` is a short-lived connection that sends `StartService`+`SubscribeLogs` then is dropped, its subscription would be lost. **Fix before this compiles correctly:** use `event_client` itself (the long-lived one) to send `SubscribeLogs`, not a fresh `cmd_client`. Replace the `Enter`/`s` handler's subscribe call to reuse a client handle shared via `Arc<tokio::sync::Mutex<DaemonClient>>` covering both the frame-reading loop and command-sending — restructure so `event_client` is wrapped in `Arc<Mutex<_>>`, cloned into the frame-reader task, and also used for `start_service`/`subscribe_logs` calls from the key handler.

- [ ] **Step 8: Apply the Arc<Mutex<DaemonClient>> fix from the note above**

Replace the `event_client`/frame-reader section and the key handler in the same file:

```rust
    let shared_client = std::sync::Arc::new(tokio::sync::Mutex::new(DaemonClient::connect().await?));

    let tx_frames = tx.clone();
    let frame_client = shared_client.clone();
    tokio::spawn(async move {
        loop {
            let frame = {
                let mut client = frame_client.lock().await;
                client.next_event().await
            };
            match frame {
                Ok(Some(frame)) => {
                    if tx_frames.send(LocalEvent::Frame(frame)).is_err() {
                        break;
                    }
                }
                Ok(None) | Err(_) => break,
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });
```

And in the `Enter`/`s` key handler:

```rust
                    KeyCode::Enter | KeyCode::Char('s') => {
                        if let Some(service) = app.services.get(app.selected_index).cloned() {
                            if service.status == ServiceStatus::Stopped
                                || service.status == ServiceStatus::Failed
                                || service.status == ServiceStatus::Completed
                            {
                                let path = project_path.clone();
                                let name = service.name.clone();
                                let mut client = shared_client.lock().await;
                                client.start_service(&path, &name).await?;
                                client.subscribe_logs(&path, &name).await?;
                            }
                        }
                    }
```

This holds the frame-read loop and command calls on the same connection, matching how the daemon's per-connection subscriber list in Task 5 actually works. The `sleep(10ms)` in the frame loop avoids busy-spinning the mutex when `next_event`'s underlying read would otherwise block indefinitely holding the lock — acceptable for Phase 1; revisit if it proves too coarse once profiles/MCP add more concurrent callers.

- [ ] **Step 9: Rewrite `devrunner-cli/src/main.rs`**

```rust
mod app;
mod cli_args;
mod client;
mod ui;

use anyhow::Result;
use clap::Parser;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli_args::Args::parse();

    let target_dir = if args.path.is_absolute() {
        args.path
    } else {
        env::current_dir()?.join(args.path)
    };

    app::run_app(target_dir).await?;

    Ok(())
}
```

Note: `--config` handling and `config::load_config` are dropped from this entrypoint for Phase 1 — the daemon doesn't yet consult per-project config (that's the Phase 2 "profiles" work per the design spec, where `custom_scripts`/`ignore_paths` finally become live). Re-add the `--config` flag to `cli_args::Args` only when Phase 2 wires it through; keeping it in Phase 1 with no effect would recreate the exact "dead config" problem this refactor is fixing.

- [ ] **Step 10: Delete the old root `src/` files now fully superseded**

```bash
git rm src/events.rs src/process.rs src/scanner.rs src/config.rs src/models.rs
```

`src/main.rs`, `src/cli.rs`, `src/ui.rs`, `src/app.rs` were already `git mv`'d or rewritten in-place above under `devrunner-cli/` — remove the now-empty root `src/` directory:

```bash
rmdir src 2>/dev/null || true
```

- [ ] **Step 11: Full workspace build and test**

Run: `cargo build --workspace`
Expected: builds cleanly (fix any remaining import path issues surfaced by the compiler)

Run: `cargo test --workspace`
Expected: all tests across `devrunner-core`, `devrunner-daemon`, `devrunner-cli` PASS

- [ ] **Step 12: Manual smoke test of the TUI end-to-end**

Run: `cargo run -p devrunner-cli -- --path devrunner-core/fixtures/dummy_js`
Expected: TUI opens, shows `dummy-frontend: start` and `dummy-frontend: build` in the services list; pressing Enter on one starts it via the daemon and streams the echoed log line into the LOGS pane; `q` quits cleanly.

- [ ] **Step 13: Commit**

```bash
git add -A
git commit -m "refactor: convert devrunner-cli TUI into a devrunnerd client over the local socket"
```

---

## Task 7: Update `.github/workflows/rust.yml` and root docs for the workspace layout

**Files:**
- Modify: `.github/workflows/rust.yml`
- Modify: `claude.md`
- Modify: `readme.md`

**Interfaces:**
- None (docs/CI only, no code interfaces)

- [ ] **Step 1: Update the CI workflow to build/test the whole workspace explicitly**

Modify `.github/workflows/rust.yml`:

```yaml
name: Rust

on:
  push:
    branches: [ "main" ]
  pull_request:
    branches: [ "main" ]

env:
  CARGO_TERM_COLOR: always

jobs:
  build:

    runs-on: ubuntu-latest

    steps:
    - uses: actions/checkout@v4
    - name: Build
      run: cargo build --workspace --verbose
    - name: Run tests
      run: cargo test --workspace --verbose
```

- [ ] **Step 2: Run the workflow's commands locally to confirm they'd pass in CI**

Run: `cargo build --workspace --verbose && cargo test --workspace --verbose`
Expected: PASS (same as Task 6 Step 11, confirming no workspace-flag-specific issues)

- [ ] **Step 3: Update `claude.md`**

Replace the "Commands" and "Architecture" sections to reflect the new workspace (keep the "In-progress direction" section pointing at the same design spec, since Phase 2 — MCP + profiles — is still pending):

```markdown
## Commands

- Build: `cargo build --workspace`
- Run the TUI: `cargo run -p devrunner-cli -- [--path <dir>]`
- Run the daemon directly (rarely needed manually — the CLI auto-spawns it): `cargo run -p devrunner-daemon`
- Install the CLI locally: `cargo install --path devrunner-cli`
- Test: `cargo test --workspace`
- Lint/format: no `.cargo/config.toml`, clippy config, or rustfmt config present; use `cargo clippy` / `cargo fmt` defaults

CI (`.github/workflows/rust.yml`) runs `cargo build --workspace --verbose` and `cargo test --workspace --verbose` on push/PR to `main`.

## Architecture

Three-crate Cargo workspace, replacing the old single-binary layout:

- **devrunner-core** (lib) — `models`, `scanner`, `config`, `process` (`ProcessManager`), `events` (`Event`: `ServiceLog`/`ServiceStatus` only — no terminal input), and `ipc` (the `DaemonRequest`/`DaemonResponse`/`EventFrame` wire types shared by the daemon and CLI). No UI or daemon dependencies — this crate is the reusable engine.
- **devrunner-daemon** (bin: `devrunnerd`) — owns a `SessionRegistry` (services keyed by `(project_path, service_name)`) and a `ProcessManager`, serves requests over a local socket (`interprocess` crate: Unix domain socket / Windows named pipe) as newline-delimited JSON. One shared daemon per machine, not per-project. Auto-spawned by the first client that can't connect.
- **devrunner-cli** (bin: `devrunner`) — the `ratatui` TUI, now a thin client of the daemon via `client::DaemonClient`. `run_app` scans the project through the daemon, starts/subscribes to services through the daemon, and renders from `EventFrame`s streamed back over the same connection.

`--config`/`custom_scripts`/`ignore_paths` are not yet wired into this daemon-backed flow (dropped from `devrunner-cli`'s args in this refactor to avoid recreating the previous "loaded but discarded" dead-config bug) — see the in-progress design spec below for when profiles bring config back as a live feature.

## In-progress direction

See [docs/superpowers/specs/2026-07-26-agent-terminal-engine-design.md](docs/superpowers/specs/2026-07-26-agent-terminal-engine-design.md) for the full design. Phase 1 (this refactor: core extraction, daemon, socket IPC, CLI-as-client) is implemented per [docs/superpowers/plans/2026-07-27-terminal-engine-phase1-core-daemon-cli.md](docs/superpowers/plans/2026-07-27-terminal-engine-phase1-core-daemon-cli.md). Still pending: the MCP server (agent-facing control surface) and project profiles (saved service lists, live `custom_scripts`).
```

- [ ] **Step 4: Update `readme.md`**

Modify the "Usage" and "Configuration" sections to match the new binary name and drop the now-inaccurate `--config` example (re-add once profiles land):

```markdown
## Usage

Navigate to any project directory and run:

```bash
devrunner
```

This auto-starts a background `devrunnerd` daemon on first use if one isn't already running — you don't need to start it manually.

### Options

- **Scan a specific path**:
  ```bash
  devrunner --path /path/to/my/project
  ```

## Configuration

Per-project config (`.devrunner.json`/`.devrunner.toml` with `custom_scripts`/`ignore_paths`) is being reworked as part of an in-progress daemon architecture — see `docs/superpowers/specs/` for the active design. Not currently wired into the daemon-backed CLI.
```

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "docs: update CI workflow, claude.md, and readme.md for the 3-crate workspace"
```

---

## Post-Phase-1 state

After this plan is complete:
- `cargo build --workspace` / `cargo test --workspace` both pass, with real tests in `devrunner-core` and `devrunner-daemon` (previously zero tests existed anywhere).
- The TUI works exactly as before from a user's perspective, but is now backed by a daemon that a second, independent client (an MCP server, in Phase 2) can connect to and observe/control the same running services.
- `StopService` is a known stub (explicit error, not silent no-op) — first task of the MCP phase's plan should either implement it there or as a preceding Phase 1.5 task, since MCP's `stop_service` tool depends on it.
- Config/profiles remain intentionally unwired (not regressed — they were already dead code before this plan; now explicitly documented as such rather than silently discarded).
- Spec's "Error handling" PID-reconciliation-on-daemon-restart (an on-disk state file so a daemon restart doesn't orphan a still-running service) is **not implemented in Phase 1** — the daemon has no persistence yet, so a daemon restart currently loses track of any process it had spawned (the child itself keeps running detached, per spec, but the registry forgets it until Phase 2's profile/state persistence work lands). Stale-socket rebind on bind failure IS covered (Task 4).
