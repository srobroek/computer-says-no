# Data Model: MCP Server + Architecture Simplification

## Entities

### McpHandler

Holds all shared state for the MCP server. Created once at startup, passed to the `ServerHandler` implementation.

| Field | Type | Description |
|-------|------|-------------|
| engine | EmbeddingEngine | Fastembed engine for embedding text |
| reference_sets | Vec\<ReferenceSet\> | Loaded reference sets |
| trained_models | Vec\<TrainedModel\> | MLP models trained at startup |
| config | AppConfig | Application configuration |

### MCP Tools

| Tool Name | Parameters | Returns |
|-----------|-----------|---------|
| classify | text: string, reference_set: string | JSON: {match, confidence, top_phrase, scores: {positive, negative}} |
| list_sets | (none) | JSON: [{name, mode, phrase_count}] |
| embed | text: string | JSON: {embedding: [f32], dimensions: usize, model: string} |
| similarity | a: string, b: string | JSON: {similarity: f32, model: string} |

### Removed Entities

| Entity | Was in | Reason |
|--------|--------|--------|
| AppState | src/server.rs | REST daemon state — replaced by McpHandler |
| ClassifyRequest/Response | src/server.rs | REST request/response types — replaced by MCP tool params |
| EmbedRequest/Response | src/server.rs | REST types — replaced by MCP tool params |
| SimilarityRequest/Response | src/server.rs | REST types — replaced by MCP tool params |
| ErrorResponse | src/server.rs | REST error format — replaced by MCP CallToolError |

## Config Changes

### Removed fields
- `host: String` (no server to bind)
- `port: u16` (no server to bind)

### Preserved fields
- `model: ModelChoice`
- `log_level: String`
- `sets_dir: PathBuf`
- `cache_dir: PathBuf`
- `datasets_dir: PathBuf`
- `mlp_*` fields (training config)

## CLI Changes

### Before (spec 001-003)
```
csn classify <text> --set <name> [--standalone] [--json]
csn embed <text> [--standalone]
csn similarity <a> <b> [--standalone]
csn serve [--port] [--model]
csn models
csn sets list
csn benchmark {run|compare-strategies|generate-datasets}
```

### After (spec 004)
```
csn classify <text> --set <name> [--json]
csn embed <text>
csn similarity <a> <b>
csn mcp                          # NEW: stdio MCP server
csn models
csn sets list
csn benchmark {run|compare-strategies|generate-datasets}
```

Removed: `serve`, `--standalone` flag from all subcommands.
Added: `mcp` subcommand.
