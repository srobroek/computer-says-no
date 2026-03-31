# Tasks: Core Binary with CLI and REST Daemon

**Input**: Design documents from `/specs/001-core-binary-cli/`
**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup

**Purpose**: Project structure, configuration, and crates.io metadata

- [X] T001 Update Cargo.toml with crates.io metadata (repository, homepage, keywords, categories, readme) and pin dependency features
- [X] T002 [P] Implement config module with 3-layer precedence (CLI > env > TOML > defaults) in src/config.rs per data-model AppConfig
- [X] T003 [P] Refactor src/model.rs: remove redundant model field, clean up allow(dead_code) annotations, ensure embed() works with shared Mutex pattern

**Checkpoint**: Config loading works, model wrapper is clean

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared state architecture and server infrastructure that ALL user stories depend on

**CRITICAL**: No user story work can begin until this phase is complete

- [X] T004 Refactor src/server.rs: replace Vec<ReferenceSet> with RwLock<Vec<ReferenceSet>> in AppState, add proper JSON error responses per contracts/rest-api.md
- [X] T005 Add graceful shutdown handler to server (SIGTERM/SIGINT via tokio::signal) in src/server.rs
- [X] T006 Refactor src/main.rs: integrate config.rs, wire config into serve command, resolve sets_dir from config precedence
- [X] T007 [P] Refactor src/reference_set.rs: add validation (reject empty phrase sets), verify blake3 embedding cache persistence (FR-014), detect dimension mismatches on model change, improve error messages for invalid TOML

**Checkpoint**: Foundation ready — daemon starts with config, shared state supports concurrent reads + atomic swap

---

## Phase 3: User Story 1 — Classify Text via CLI (Priority: P1) MVP

**Goal**: Developer classifies text via CLI command, connecting to running daemon as thin HTTP client

**Independent Test**: Run `csn classify "no, use X instead" --set corrections` against running daemon and verify match result

### Implementation for User Story 1

- [X] T008 [US1] Implement CLI classify as thin HTTP client in src/main.rs: POST to daemon /classify, display result (human-readable default, --json for machine-readable)
- [X] T009 [US1] Add daemon reachability check: if daemon unreachable, exit code 1 with warning to stderr suggesting `csn serve` in src/main.rs
- [X] T010 [US1] Add --json output flag to classify command with serde_json formatting per contracts/cli.md
- [X] T011 [US1] Handle error responses: non-existent reference set returns error listing available sets per contracts/rest-api.md
- [X] T012 [US1] Implement --standalone flag for classify/embed/similarity: load model in-process, classify without daemon, exit. Reuse EmbeddingEngine + classifier directly in src/main.rs

**Checkpoint**: `csn classify` works end-to-end against running daemon and in standalone mode. US1 is independently testable.

---

## Phase 4: User Story 2 — Serve REST API for Hooks (Priority: P1)

**Goal**: Daemon serves all REST endpoints per contracts/rest-api.md, returning JSON within 50ms warm

**Independent Test**: Start daemon, POST to /classify with JSON body, verify response matches contract

### Implementation for User Story 2

- [X] T013 [US2] Implement POST /classify endpoint with correct JSON response schema for both binary and multi-category modes per contracts/rest-api.md in src/server.rs
- [X] T014 [P] [US2] Implement POST /embed endpoint returning embedding vector, dimensions, model name per contracts/rest-api.md in src/server.rs
- [X] T015 [P] [US2] Implement POST /similarity endpoint returning cosine similarity score per contracts/rest-api.md in src/server.rs
- [X] T016 [P] [US2] Implement GET /health endpoint returning status, model, set count, uptime per contracts/rest-api.md in src/server.rs
- [X] T017 [P] [US2] Implement GET /sets endpoint returning reference set metadata list per contracts/rest-api.md in src/server.rs
- [X] T018 [US2] Add structured JSON error responses: 404 for missing set (with available names), 500 for embedding failures in src/server.rs

**Checkpoint**: All REST endpoints match contracts. US2 is independently testable with curl.

---

## Phase 5: User Story 3 — Manage Reference Sets with Hot-Reload (Priority: P2)

**Goal**: Developer drops a TOML file in reference sets dir, daemon detects and loads it within 1 second

**Independent Test**: With daemon running, create a new .toml file in sets dir, wait 1s, classify against the new set

### Implementation for User Story 3

- [X] T019 [US3] Implement file watcher using notify::RecommendedWatcher in src/watcher.rs: watch sets directory, filter for .toml changes, debounce within 500ms
- [X] T020 [US3] Wire watcher into daemon startup in src/server.rs: spawn watcher task, connect to AppState RwLock<Vec<ReferenceSet>> for atomic swap on reload
- [X] T021 [US3] Handle watcher edge cases: invalid TOML skipped with warning, deleted sets removed, zero-phrase sets rejected with error log
- [X] T022 [US3] Implement csn sets list as CLI command in src/main.rs: read sets directory directly (no daemon required), display name, mode, phrase count

**Checkpoint**: Hot-reload works. US3 is independently testable by adding/modifying TOML files while daemon runs.

---

## Phase 6: User Story 4 — Compute Embeddings and Similarity (Priority: P3)

**Goal**: Developer uses CLI for embedding vectors and similarity scores to debug reference sets

**Independent Test**: Run `csn embed "hello"` and `csn similarity "a" "b"` against running daemon

### Implementation for User Story 4

- [X] T023 [P] [US4] Implement csn embed as thin HTTP client in src/main.rs: POST to daemon /embed, display JSON result
- [X] T024 [P] [US4] Implement csn similarity as thin HTTP client in src/main.rs: POST to daemon /similarity, display score
- [X] T025 [US4] Implement csn models as local command in src/main.rs: list all ModelChoice variants with name and dimensions (no daemon required)

**Checkpoint**: All CLI commands work. US4 is independently testable.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Quality, edge cases, and distribution readiness

- [X] T026 Add unit tests for config.rs: test 3-layer precedence, missing file fallback, env var override
- [X] T027 [P] Add unit tests for watcher.rs: test debounce logic, invalid file handling
- [X] T028 Add integration test in tests/integration/: start daemon, classify via REST, verify response matches contract
- [X] T029 [P] Handle edge cases from spec: port-in-use detection with clear error, model download on first use, config file missing fallback
- [X] T030 Run cargo clippy -- -D warnings, cargo fmt --check, cargo test — fix any issues
- [ ] T031 Verify SC-001 (warm latency <50ms) with hyperfine benchmark against running daemon

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on T002 (config) and T003 (model refactor)
- **US1 (Phase 3)**: Depends on Phase 2 (server with shared state)
- **US2 (Phase 4)**: Depends on Phase 2. Can run in parallel with US1.
- **US3 (Phase 5)**: Depends on Phase 2 (RwLock state for atomic swap)
- **US4 (Phase 6)**: Depends on Phase 2 (daemon must be serving)
- **Polish (Phase 7)**: Depends on all user stories

### Parallel Opportunities

- T002 + T003: config and model refactor touch different files
- T013 + T014 + T015 + T016: REST endpoints are independent routes
- T022 + T023: embed and similarity CLI are independent commands
- T025 + T026: test files are independent
- US1 and US2 can be worked in parallel after Phase 2

### Within Each User Story

- Server-side implementation before CLI client (US2 before US1 in practice)
- Core endpoint before error handling
- Story complete before moving to next priority

---

## Implementation Strategy

### MVP First (User Story 1 + 2)

1. Complete Phase 1: Setup (config, Cargo.toml, model cleanup)
2. Complete Phase 2: Foundational (shared state, graceful shutdown)
3. Complete Phase 4: US2 (REST endpoints — server-side must exist before CLI can call it)
4. Complete Phase 3: US1 (CLI classify as HTTP client)
5. **STOP and VALIDATE**: Test classification end-to-end (daemon + CLI + curl)
6. Deploy/demo if ready

### Incremental Delivery

1. Setup + Foundational → Foundation ready
2. Add US2 (REST) → Test with curl → MVP daemon
3. Add US1 (CLI classify) → Test end-to-end → MVP complete
4. Add US3 (hot-reload) → Test file watching → Extensibility ready
5. Add US4 (embed/similarity) → Test CLI commands → Full feature set
6. Polish → Tests, benchmarks, edge cases → Release ready
