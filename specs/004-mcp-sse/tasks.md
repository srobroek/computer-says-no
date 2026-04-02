# Tasks: MCP Server + Architecture Simplification

**Input**: Design documents from `/specs/004-mcp-sse/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup

**Purpose**: Add MCP dependency, remove daemon/REST dependencies

- [ ] T001 Add `rust-mcp-sdk` and `async-trait` dependencies to `Cargo.toml`. Remove `axum`, `reqwest`, `notify` dependencies. Run `cargo check` to update `Cargo.lock` (will have compile errors — expected until code is removed)
- [ ] T002 [P] Delete `src/server.rs` (REST daemon: AppState, routes, handlers). Remove `mod server;` from `src/main.rs`
- [ ] T003 [P] Delete `src/watcher.rs` (file watcher, hot-reload). Remove `mod watcher;` from `src/main.rs`

---

## Phase 2: Simplify CLI

**Purpose**: Remove daemon-related code from CLI, drop --standalone flag

- [ ] T004 Remove `Serve` variant from `Command` enum in `src/main.rs`. Remove the `cmd_serve` match arm and `serve()` call
- [ ] T005 Remove `--standalone` flag from `Classify`, `Embed`, `Similarity` subcommands in `src/main.rs`. Remove HTTP client paths (`cmd_classify_remote`, `cmd_embed_remote`, `cmd_similarity_remote`, `check_daemon`, `daemon_url`). Keep only the in-process logic (formerly standalone)
- [ ] T006 Remove `host` and `port` fields from `AppConfig` in `src/config.rs`. Remove `CSN_HOST`, `CSN_PORT` env var parsing and `FileConfig` fields. Remove `DEFAULT_HOST`, `DEFAULT_PORT` constants

**Checkpoint**: `cargo check` compiles cleanly. CLI commands work in-process. No REST/daemon/watcher code remains.

---

## Phase 3: User Story 1+2 — Classify + List Sets via MCP (Priority: P1)

**Goal**: Core MCP tools — classify text and discover reference sets.

**Independent Test**: Configure `csn mcp` in Claude Code, call `classify` and `list_sets` tools.

- [ ] T007 Create `src/mcp.rs` module file. Define `McpHandler` struct holding `EmbeddingEngine`, `Vec<ReferenceSet>`, `Vec<TrainedModel>`, and `AppConfig`. Add `mod mcp;` to `src/main.rs`
- [ ] T008 [US1] Define `ClassifyTool` struct in `src/mcp.rs` using `#[mcp_tool(name = "classify", description = "...")]` with fields `text: String` and `reference_set: String`. Implement `call_tool()` that calls `classifier::classify_text()` and returns JSON result via `CallToolResult::text_content`
- [ ] T009 [US2] Define `ListSetsTool` struct in `src/mcp.rs` using `#[mcp_tool(name = "list_sets", description = "...")]` with no fields. Implement `call_tool()` that iterates reference sets and returns JSON array of {name, mode, phrase_count}
- [ ] T010 Create `tool_box!(CsnTools, [ClassifyTool, ListSetsTool, EmbedTool, SimilarityTool])` enum in `src/mcp.rs` (EmbedTool and SimilarityTool can be stubs initially). Implement `ServerHandler` trait on `McpHandler`: `handle_list_tools_request` returns `CsnTools::tools()`, `handle_call_tool_request` dispatches via `CsnTools::try_from(params)` match
- [ ] T011 Add `Mcp` subcommand to `Command` enum in `src/main.rs`. Implement `cmd_mcp()`: load config, init EmbeddingEngine, load reference sets, train MLP models, create `McpHandler`, create `StdioTransport`, start MCP server via `server_runtime::create_server`. Add `eprintln!` status messages during loading (stderr, not stdout — stdout is MCP protocol)
- [x] T012 [US1] Unit test in `src/mcp.rs`: verify `ClassifyTool` returns valid JSON with match, confidence, top_phrase, scores fields for a known input (use synthetic embeddings like spec 003 tests)
- [x] T013 [US2] Unit test in `src/mcp.rs`: verify `ListSetsTool` returns correct set metadata

**Checkpoint**: `csn mcp` starts, `classify` and `list_sets` tools work.

---

## Phase 4: User Story 3+4 — Embed + Similarity via MCP (Priority: P2)

**Goal**: Embedding and similarity tools for advanced use cases.

**Independent Test**: Call `embed` and `similarity` tools via MCP, verify results.

- [x] T014 [P] [US3] Define `EmbedTool` struct in `src/mcp.rs` using `#[mcp_tool(name = "embed", description = "...")]` with field `text: String`. Implement `call_tool()` that calls `engine.embed_one()` and returns JSON with embedding, dimensions, model name
- [x] T015 [P] [US4] Define `SimilarityTool` struct in `src/mcp.rs` using `#[mcp_tool(name = "similarity", description = "...")]` with fields `a: String` and `b: String`. Implement `call_tool()` that embeds both texts and computes `cosine_similarity`, returns JSON with similarity score and model name
- [x] T016 [US3] Unit test for `EmbedTool`: verify output has correct embedding dimensions
- [x] T017 [US4] Unit test for `SimilarityTool`: verify similarity of identical texts is ~1.0

**Checkpoint**: All 4 MCP tools functional.

---

## Phase 5: User Story 5+6 — CLI Cleanup + Zero-Config MCP (Priority: P1)

**Goal**: CLI works without daemon flags, MCP configuration is one line.

**Independent Test**: Run `csn classify` without `--standalone`, configure MCP in Claude Code.

- [x] T018 [US5] Verify all CLI subcommands work without `--standalone` flag. Run `csn classify "test" --set corrections`, `csn embed "test"`, `csn similarity "a" "b"`, `csn models`, `csn sets list`. Fix any remaining references to daemon/standalone logic
- [ ] T019 [US6] [MANUAL] Configure `csn mcp` in Claude Code or MCP Inspector. Verify tool discovery and all 4 tools callable. Document configuration in quickstart.md if needed

**Checkpoint**: End-to-end MCP + CLI working.

---

## Phase 6: Update Tests

**Purpose**: Replace REST integration tests with MCP tests

- [x] T020 Remove REST-based integration tests from `tests/integration_test.rs` (daemon subprocess tests that hit HTTP endpoints)
- [x] T021 Add MCP integration test in `tests/integration_test.rs`: spawn `csn mcp` as subprocess, send `initialize` JSON-RPC request via stdin, verify `tools/list` response contains 4 tools. Mark `#[ignore]` (requires model download)
- [x] T022 Run `just check` (clippy + fmt + test + build). Fix any issues

**Checkpoint**: All tests pass, clippy clean.

---

## Phase 7: Polish & Cross-Cutting

**Purpose**: Cleanup, docs, validation

- [x] T023 [P] Update CLAUDE.md: remove daemon/REST/watcher references, add MCP subcommand, update test count and architecture description
- [x] T024 [P] Update `docs/spec-dependency-graph.md`: mark 004 as done
- [x] T025 Run `cargo build --release` and compare binary size to pre-spec-004 binary (expect decrease from removing axum/reqwest/notify). Record results
- [ ] T026 [MANUAL] Verify MCP tools work end-to-end from Claude Code with real model

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

# Phase 2: Simplify CLI
[graph.T004]
blocked_by = ["T002"]

[graph.T005]
blocked_by = ["T002"]

[graph.T006]
blocked_by = ["T004", "T005"]

# Phase 3: US1+US2 — Classify + List Sets
[graph.T007]
blocked_by = ["T001"]

[graph.T008]
blocked_by = ["T007"]

[graph.T009]
blocked_by = ["T007"]

[graph.T010]
blocked_by = ["T008", "T009"]

[graph.T011]
blocked_by = ["T006", "T010"]

[graph.T012]
blocked_by = ["T008"]

[graph.T013]
blocked_by = ["T009"]

# Phase 4: US3+US4 — Embed + Similarity
[graph.T014]
blocked_by = ["T007"]

[graph.T015]
blocked_by = ["T007"]

[graph.T016]
blocked_by = ["T014"]

[graph.T017]
blocked_by = ["T015"]

# Phase 5: US5+US6 — CLI + Zero-Config
[graph.T018]
blocked_by = ["T006"]

[graph.T019]
blocked_by = ["T011", "T014", "T015"]

# Phase 6: Tests
[graph.T020]
blocked_by = ["T006"]

[graph.T021]
blocked_by = ["T011"]

[graph.T022]
blocked_by = ["T018", "T020", "T021"]

# Phase 7: Polish
[graph.T023]
blocked_by = ["T022"]

[graph.T024]
blocked_by = ["T022"]

[graph.T025]
blocked_by = ["T022"]

[graph.T026]
blocked_by = ["T019"]
```
