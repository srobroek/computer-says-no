# Tasks: Lazy Auto-Starting Daemon

**Input**: Design documents from `/specs/005-lazy-daemon/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup

**Purpose**: Add dependencies, create module files, add config fields

- [ ] T001 Add `nix = { version = "0.29", features = ["signal", "process"] }` to `Cargo.toml`. Run `cargo check` to verify dependency resolves
- [ ] T002 [P] Create empty `src/daemon.rs` module file. Add `mod daemon;` to `src/main.rs`
- [ ] T003 [P] Create empty `src/client.rs` module file. Add `mod client;` to `src/main.rs`
- [ ] T004 Add `idle_timeout` field (u64, default 300) to `AppConfig` in `src/config.rs`. Add `CSN_IDLE_TIMEOUT` env var support. Add `[daemon]` section with `idle_timeout` to `FileConfig`

---

## Phase 2: Foundational — Socket Protocol + Daemon Server

**Purpose**: Core daemon infrastructure that all user stories depend on

- [ ] T005 Define request/response types in `src/daemon.rs`: `DaemonRequest` (command + args), `DaemonResponse` (ok + result/error) as per `contracts/socket-protocol.md`. Derive `Serialize`/`Deserialize`
- [ ] T006 Implement daemon socket server in `src/daemon.rs`: `run_daemon(config: &AppConfig)` — bind `UnixListener` at socket path from `config.cache_dir`, write PID file, accept connections, read JSON line, dispatch to handler, write JSON response. Use `tokio::select!` with shutdown channel. Daemon reads `config.idle_timeout` for the idle timer
- [ ] T007 Implement idle timeout in `src/daemon.rs`: `IdleTracker` struct with `Arc<AtomicU64>` last-request timestamp. Background task checks every 30s, sends shutdown signal via `tokio::sync::broadcast` when idle exceeds `config.idle_timeout`
- [ ] T008 Implement graceful shutdown in `src/daemon.rs`: on shutdown signal (idle timeout or SIGTERM), stop accepting connections, remove socket file, remove PID file, exit cleanly
- [ ] T009 Implement request dispatch in `src/daemon.rs`: match `command` field to classify/embed/similarity, call existing `classifier::classify_text`, `engine.embed_one`, `model::cosine_similarity`. Reuse `McpHandler`-style Mutex pattern for engine access

**Checkpoint**: `csn daemon` can be started manually, serves requests on socket, exits on idle.

---

## Phase 3: User Story 1 — Transparent Fast Classification (Priority: P1)

**Goal**: `csn classify` transparently routes through daemon when available, auto-starts daemon when not.

**Independent Test**: Run `csn classify "test" --set corrections --json` twice. First starts daemon, second uses warm daemon (<30ms).

- [ ] T010 [US1] Implement daemon client in `src/client.rs`: `try_daemon_request(request: &DaemonRequest) -> Option<DaemonResponse>` — connect to socket, send JSON line, read JSON response. Return `None` on any connection error
- [ ] T011 [US1] Implement stale socket detection in `src/client.rs`: `is_daemon_alive(pid_path: &Path) -> bool` — read PID file, check process alive via `nix::sys::signal::kill(pid, None)`. Cleanup stale socket + PID files if dead
- [ ] T012 [US1] Implement daemon spawn in `src/client.rs`: `spawn_daemon() -> Result<()>` — fork `csn daemon` as detached process via `Command::new(current_exe()).arg("daemon").stdin(null).stdout(null).stderr(null).process_group(0).spawn()`. Acquire `flock` on lock file to prevent races. Wait for socket to appear with exponential backoff (10ms, 20ms, 40ms... max 500ms total)
- [ ] T013 [US1] Implement routing logic in `src/client.rs`: `classify_via_daemon(text, set, config) -> Result<Option<ClassifyResult>>` — try connect → if fail, check alive → if stale cleanup → spawn daemon → retry connect. Return `None` if daemon unavailable (caller falls back to in-process)
- [ ] T014 [US1] Modify `cmd_classify` in `src/main.rs`: before in-process classification, call `classify_via_daemon()`. If it returns `Some(result)`, print and return. If `None`, fall back to existing in-process path
- [ ] T015 [US1] Add hidden `Daemon` subcommand to `Command` enum in `src/main.rs`. Implement `cmd_daemon()` that calls `daemon::run_daemon(&config)`. Hide from help with `#[command(hide = true)]`
- [ ] T016 [US1] Unit test in `src/daemon.rs`: verify `DaemonRequest`/`DaemonResponse` round-trip serialization matches contract
- [ ] T017 [US1] Unit test in `src/client.rs`: verify `is_daemon_alive` returns false for non-existent PID

**Checkpoint**: `csn classify` auto-starts daemon and uses warm path on subsequent calls.

---

## Phase 4: User Story 2 — Daemon Self-Exits on Idle (Priority: P2)

**Goal**: Daemon exits after configurable idle timeout, cleans up files.

**Independent Test**: Start daemon, wait for timeout, verify process exited and socket removed.

- [ ] T018 [US2] Integration test in `tests/integration_test.rs`: spawn `csn daemon` with short idle timeout (e.g., 2s via `CSN_IDLE_TIMEOUT=2`), verify socket appears, send one request, wait 5s, verify socket file removed and process exited. Mark `#[ignore]` (requires model download)

**Checkpoint**: Daemon auto-exits after idle timeout.

---

## Phase 5: User Story 4 — Embed and Similarity via Daemon (Priority: P2)

**Goal**: `csn embed` and `csn similarity` also route through the daemon.

**Independent Test**: With daemon running, call `csn embed "test"` and `csn similarity "a" "b"`. Verify <30ms.

- [ ] T019 [P] [US4] Implement `embed_via_daemon(text, config) -> Result<Option<Value>>` in `src/client.rs`. Same pattern as `classify_via_daemon`
- [ ] T020 [P] [US4] Implement `similarity_via_daemon(a, b, config) -> Result<Option<f32>>` in `src/client.rs`. Same pattern as `classify_via_daemon`
- [ ] T021 [US4] Modify `cmd_embed` in `src/main.rs`: try daemon first, fall back to in-process
- [ ] T022 [US4] Modify `cmd_similarity` in `src/main.rs`: try daemon first, fall back to in-process

**Checkpoint**: All three data commands (classify, embed, similarity) use daemon when available.

---

## Phase 6: User Story 3 — Manual Daemon Control (Priority: P3)

**Goal**: `csn stop` command to manually stop the daemon.

**Independent Test**: Start daemon via classify, run `csn stop`, verify process exited.

- [ ] T023 [US3] Add `Stop` subcommand to `Command` enum in `src/main.rs`. Implement `cmd_stop()`: read PID file, send SIGTERM via `nix::sys::signal::kill(pid, Signal::SIGTERM)`, wait briefly, clean up socket + PID files if still present
- [ ] T024 [US3] Handle SIGTERM in daemon: register signal handler in `src/daemon.rs` via `tokio::signal::unix::signal(SignalKind::terminate())`, feed into the shutdown `broadcast` channel alongside idle timeout

**Checkpoint**: `csn stop` gracefully stops the daemon.

---

## Phase 7: Polish & Cross-Cutting

**Purpose**: Tests, docs, cleanup

- [ ] T025 [P] Run `just check` (clippy + fmt + test + build). Fix any issues
- [ ] T026 [P] Update CLAUDE.md: add daemon/client modules, update architecture description, add `csn stop` to commands
- [ ] T027 [P] Update `docs/spec-dependency-graph.md`: add spec 005, mark dependencies
- [ ] T028 [MANUAL] Benchmark with `hyperfine`: warm-path (<30ms, SC-001), cold-start (<500ms, SC-002), and verify byte-identical results between daemon and in-process (SC-004)
- [ ] T029 Verify `csn mcp` still functions correctly after daemon code is added (SC-005). Run existing MCP integration test or manual MCP Inspector check

---

## Task Dependencies

<!-- Machine-readable. Generated by /speckit.tasks, updated by /speckit.iterate.apply -->
<!-- Do not edit manually unless you also update GitHub issue dependencies -->

```toml
[graph]
# Phase 1: Setup
[graph.T001]
blocked_by = []

[graph.T002]
blocked_by = ["T001"]

[graph.T003]
blocked_by = ["T001"]

[graph.T004]
blocked_by = []

# Phase 2: Foundational
[graph.T005]
blocked_by = ["T002"]

[graph.T006]
blocked_by = ["T005", "T004"]

[graph.T007]
blocked_by = ["T006"]

[graph.T008]
blocked_by = ["T007"]

[graph.T009]
blocked_by = ["T006"]

# Phase 3: US1 — Transparent Fast Classification
[graph.T010]
blocked_by = ["T003", "T005"]

[graph.T011]
blocked_by = ["T010"]

[graph.T012]
blocked_by = ["T011"]

[graph.T013]
blocked_by = ["T012"]

[graph.T014]
blocked_by = ["T009", "T013"]

[graph.T015]
blocked_by = ["T006"]

[graph.T016]
blocked_by = ["T005"]

[graph.T017]
blocked_by = ["T011"]

# Phase 4: US2 — Idle Self-Exit
[graph.T018]
blocked_by = ["T008", "T015"]

# Phase 5: US4 — Embed + Similarity
[graph.T019]
blocked_by = ["T010"]

[graph.T020]
blocked_by = ["T010"]

[graph.T021]
blocked_by = ["T019", "T009"]

[graph.T022]
blocked_by = ["T020", "T009"]

# Phase 6: US3 — Manual Stop
[graph.T023]
blocked_by = ["T008"]

[graph.T024]
blocked_by = ["T008"]

# Phase 7: Polish
[graph.T025]
blocked_by = ["T014", "T018", "T021", "T022", "T023", "T024"]

[graph.T026]
blocked_by = ["T025"]

[graph.T027]
blocked_by = ["T025"]

[graph.T028]
blocked_by = ["T014"]

[graph.T029]
blocked_by = ["T015"]
```
