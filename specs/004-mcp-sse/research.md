# Research: MCP Server + Architecture Simplification

## MCP SDK Selection

**Decision**: `rust-mcp-sdk` (rust-mcp-stack)
**Rationale**: High-quality Rust MCP SDK with stdio transport, `#[mcp_tool]` macro for auto-generating JSON Schema, async_trait-based `ServerHandler`, active maintenance, 236 code snippets in docs.
**Alternatives considered**:
- `prism-mcp-rs`: Lower benchmark score (39.6 vs 85.9), enterprise-focused, overkill for a CLI tool
- Manual JSON-RPC over stdio: No standard compliance guarantees, would need to implement protocol from scratch
- `rmcp`: Lighter but less documented

## Transport Decision

**Decision**: Stdio only (no SSE, no HTTP)
**Rationale**: Matches the ecosystem convention. Claude Code, Cursor, and other MCP clients spawn the server as a subprocess (`"command": "csn", "args": ["mcp"]`). Client manages lifecycle. No ports, no sockets, no daemon management.
**Alternatives considered**:
- SSE/HTTP (daemon model): Requires manual server management, port configuration, not how MCP servers typically work
- Unix socket: Non-standard for MCP, would require custom transport implementation

## Dependencies to Remove

| Dependency | Reason for removal |
|------------|-------------------|
| `axum 0.8` | REST server framework — no more REST endpoints |
| `reqwest 0.12` | HTTP client for CLI→daemon — CLI now runs in-process |
| `notify 7` | File watcher for hot-reload — MCP process is restarted for changes |
| `tower-*` (transitive) | Axum middleware — removed with axum |
| `tokio` features | May be simplified — check if MCP SDK needs full tokio or just basic runtime |

## Dependencies to Add

| Dependency | Version | Purpose |
|------------|---------|---------|
| `rust-mcp-sdk` | latest | MCP protocol, stdio transport, tool macros |
| `async-trait` | latest | Required by `ServerHandler` trait |
| `schemars` (via `rust-mcp-sdk`) | transitive | JSON Schema generation for tool parameters |

## MCP Tool Design

Each tool maps directly to an existing function:

| MCP Tool | Existing Function | Module |
|----------|-------------------|--------|
| `classify` | `classifier::classify_text()` | `src/classifier.rs` |
| `list_sets` | direct reference set iteration | `src/reference_set.rs` |
| `embed` | `EmbeddingEngine::embed_one()` | `src/model.rs` |
| `similarity` | `model::cosine_similarity()` | `src/model.rs` |

Tool results return JSON-formatted text content (MCP standard).

## Tokio Runtime Consideration

The MCP SDK requires tokio for async stdio handling. `fastembed` also uses tokio for model loading. The runtime stays, but the axum/hyper server stack is removed. Net dependency reduction is significant.
