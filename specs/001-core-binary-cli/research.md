# Research: Core Binary with CLI and REST Daemon

**Date**: 2026-03-31
**Sources**: fastembed 5.13 source, axum 0.8 docs, notify 7 docs, astro-up repo patterns

## Key Decisions

### 1. Embedding Engine (fastembed-rs 5.13)

**Decision**: Use fastembed-rs with `TextEmbedding` and `InitOptions`/`InitOptionsWithLength`.

**API Summary** (from source inspection):
- `TextInitOptions` = `InitOptionsWithLength<EmbeddingModel>` (re-exported as `InitOptions`)
- `TextEmbedding::try_new(options: InitOptions) -> Result<Self>`
- `embed(&mut self, texts: impl AsRef<[S]>, batch_size: Option<usize>) -> Result<Vec<Embedding>>`
- `Embedding = Vec<f32>`
- `EmbeddingModel` enum: `BGESmallENV15`, `BGESmallENV15Q`, `NomicEmbedTextV15`, `NomicEmbedTextV15Q`, `AllMiniLML6V2`, `AllMiniLML6V2Q`, `GTELargeENV15`, `GTELargeENV15Q`, `SnowflakeArcticEmbedS`, `SnowflakeArcticEmbedSQ`, etc.
- Cache dir via `options.with_cache_dir(path)`
- Model download on first use with progress bar

**Note**: `embed()` takes `&mut self` — the engine is not `Sync`. Requires `Mutex` for shared access in async context.

**Alternatives considered**: None — fastembed-rs is the only Rust crate wrapping ONNX embedding models with a reasonable API. Direct ort usage would require manual tokenization.

### 2. HTTP Server (axum 0.8)

**Decision**: axum with `Router`, `State(Arc<AppState>)`, JSON extractors.

**Patterns**:
- Shared state: `Arc<AppState>` with `Mutex<EmbeddingEngine>` for the non-Sync engine
- Routes: `Router::new().route("/classify", post(handle)).with_state(state)`
- JSON: `Json<T>` for both extraction and response
- Error handling: return `(StatusCode, String)` for simple errors
- Graceful shutdown: `axum::serve(listener, app).with_graceful_shutdown(signal)` for SIGTERM/SIGINT

**Alternatives considered**: actix-web (heavier, actor model unnecessary), warp (less maintained), hyper directly (too low-level).

### 3. File Watching (notify 7)

**Decision**: `RecommendedWatcher` watching the reference sets directory, debounced.

**Patterns**:
- `notify::recommended_watcher(callback)` — creates platform-optimal watcher
- `watcher.watch(path, RecursiveMode::NonRecursive)` — watch one directory
- Events: `EventKind::Create`, `Modify`, `Remove` — filter for `.toml` extension
- Debounce: use `notify_debouncer_mini` or manual debounce (coalesce events within 500ms window)
- Callback triggers re-scan of entire directory + re-embed changed sets
- Atomic swap of reference sets via `Arc::swap` or `RwLock`

**Alternatives considered**: polling-based (simpler but wastes CPU), inotify directly (Linux-only).

### 4. CLI Structure (clap 4 derive)

**Decision**: clap derive with subcommand enum, following astro-up patterns.

**Patterns from astro-up**:
- Thin CLI shell: `main.rs` calls `lib.rs::run()`, all logic in separate modules
- `#[derive(Parser)]` with `#[command(name, version, about)]`
- Subcommands via `#[derive(Subcommand)]` enum
- `FromStr` for custom value types (model names)
- `default_value_t` for optional args with defaults

### 5. Error Handling

**Decision**: `anyhow` for application errors, `thiserror` for library-level error types if we add a library crate later. Single binary for now — `anyhow` everywhere is sufficient.

**Pattern from astro-up**: `thiserror` enum for core, `anyhow` for CLI. We're simpler — no workspace, single binary — so `anyhow` alone covers our needs. Can split later if warranted.

### 6. Configuration

**Decision**: 3-layer precedence (CLI flags > env vars > TOML file > defaults), following astro-up's pattern but without SQLite.

**Implementation**:
- Parse TOML config file from `~/.config/computer-says-no/config.toml`
- Override with `CSN_*` env vars
- Override with CLI flags
- Config struct with serde Deserialize + Default
- No validation crate needed — config is simple (port, model name, log level, paths)

### 7. Module Organization

**Decision**: Flat module structure in `src/` (no workspace). Follows constitution Principle V (Simplicity).

```
src/
├── main.rs           # CLI entry point (clap), dispatches to commands
├── config.rs         # Config loading (TOML + env + defaults)
├── model.rs          # EmbeddingEngine wrapper around fastembed
├── reference_set.rs  # TOML parsing, content hashing, embedding cache
├── classifier.rs     # Classification logic (binary + multi-category)
├── server.rs         # axum REST API
└── watcher.rs        # File watcher for reference set hot-reload
```

**Rationale**: astro-up uses a workspace (core + cli + gui) because it has 3 frontends. We have one binary — a workspace would be premature. If MCP/SSE or a GUI is added later, we can extract a core lib.

### 8. State Architecture

**Decision**: `Arc<AppState>` shared between server routes and file watcher.

```
AppState {
    engine: Mutex<EmbeddingEngine>,     // fastembed is !Sync
    sets: RwLock<Vec<ReferenceSet>>,    // swapped atomically on reload
    config: AppConfig,                   // immutable after startup
    start_time: Instant,
}
```

**Key change from prototype**: Use `RwLock<Vec<ReferenceSet>>` instead of plain `Vec` so the file watcher can swap in new sets without blocking concurrent reads. The `Mutex<EmbeddingEngine>` remains — fastembed requires `&mut self` for `embed()`.
