# Implementation Plan: MCP Server + Architecture Simplification

**Branch**: `004-mcp-sse` | **Date**: 2026-04-02 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/004-mcp-sse/spec.md`

## Summary

Add `csn mcp` subcommand providing an MCP server over stdio transport. Remove the daemon (`csn serve`), REST endpoints, `--standalone` flag, file watcher, and HTTP client. CLI commands run in-process. MCP tools map 1:1 to existing classifier/embedding functions.

## Technical Context

**Language/Version**: Rust 2024 edition (1.92)
**Primary Dependencies**: rust-mcp-sdk (stdio transport, tool macros), existing: fastembed 5.13, clap 4, burn 0.20
**Dependencies to remove**: axum 0.8, reqwest 0.12, notify 7, tower (transitive)
**Storage**: TOML config, TOML reference sets, binary MLP weight cache (unchanged)
**Testing**: cargo test (unit + integration)
**Target Platform**: macOS (primary), Linux
**Project Type**: CLI + MCP server
**Performance Goals**: MCP startup < 5s (model + MLP cache load), tool calls < 100ms
**Constraints**: Stdio MCP is single-client, sequential tool calls

## Constitution Check

No constitution file exists. No violations to check.

## Project Structure

### Documentation (this feature)

```text
specs/004-mcp-sse/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   └── mcp-tools.md
└── tasks.md
```

### Source Code

```text
src/
├── main.rs          # CLI: remove serve/standalone, add mcp subcommand
├── mcp.rs           # NEW: MCP server handler, tool definitions, stdio transport
├── classifier.rs    # Unchanged (classify, classify_with_mlp)
├── mlp.rs           # Unchanged (training, inference, cache)
├── model.rs         # Unchanged (EmbeddingEngine, cosine_similarity)
├── reference_set.rs # Unchanged (TOML loading)
├── config.rs        # Remove host/port, keep model/cache/mlp config
├── benchmark.rs     # Unchanged
├── dataset.rs       # Unchanged
├── embedding_cache.rs # Unchanged

# REMOVED:
# ├── server.rs      # REST daemon (axum routes, AppState, handlers)
# ├── watcher.rs     # File watcher (notify, hot-reload)

tests/
├── benchmark_test.rs     # Unchanged
└── integration_test.rs   # Rewrite: MCP tool tests instead of REST endpoint tests
```

**Structure Decision**: Single binary, new `src/mcp.rs` module. Remove `src/server.rs` and `src/watcher.rs`. No new directories needed.

## Implementation Phases

### Phase 1: Remove daemon and REST (FR-008, FR-009, FR-010)

1. Remove `src/server.rs` (REST endpoints, AppState, axum handlers)
2. Remove `src/watcher.rs` (file watcher, hot-reload)
3. Remove `serve` subcommand from `src/main.rs`
4. Remove `--standalone` flag from classify/embed/similarity subcommands
5. Remove `host` and `port` from `AppConfig` in `src/config.rs`
6. Remove `axum`, `reqwest`, `notify`, `tower-*` from `Cargo.toml`
7. Update CLI handlers: remove HTTP client paths, keep in-process logic
8. Update `Cargo.lock`

### Phase 2: Add MCP server (FR-001, FR-002, FR-003, FR-004, FR-005, FR-007)

1. Add `rust-mcp-sdk` dependency to `Cargo.toml`
2. Create `src/mcp.rs`: MCP handler struct holding EmbeddingEngine + reference sets + trained models
3. Define 4 MCP tools using `#[mcp_tool]` macro: classify, list_sets, embed, similarity
4. Implement `ServerHandler` trait: `handle_list_tools_request` returns tool schemas, `handle_call_tool_request` dispatches to existing functions
5. Add `mcp` subcommand to `src/main.rs`: load config, initialize engine + sets + MLP, start stdio MCP server
6. Implement MCP error handling (FR-006): unknown reference set, missing params

### Phase 3: Update tests (SC-004)

1. Remove REST integration tests from `tests/integration_test.rs`
2. Add MCP integration tests: spawn `csn mcp` as subprocess, send JSON-RPC via stdin, verify stdout responses
3. Verify CLI commands still work without `--standalone`
4. Run `just check` (clippy + fmt + test + build)

### Phase 4: Polish

1. Update CLAUDE.md with architecture changes
2. Update dep graph (004 done)
3. Verify binary size decreased
