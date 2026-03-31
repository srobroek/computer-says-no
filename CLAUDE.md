# Computer Says No (`csn`)

Local embedding service for text classification using ONNX models via fastembed-rs.

## Architecture

Single Rust binary with CLI and REST daemon:
- **REST API** — hooks and CLI call `/classify`, `/embed`, `/similarity`, `/health`, `/sets`
- **CLI** — thin HTTP clients to daemon, `--standalone` for in-process mode
- **MCP/SSE** — planned for spec 004

## Directory Structure

```
src/              Rust source (main.rs, config.rs, server.rs, classifier.rs, model.rs, reference_set.rs, embedding_cache.rs, watcher.rs)
reference-sets/   Default TOML reference sets (shipped with binary)
tests/            Integration tests
specs/            Feature specifications (speckit)
```

## Commands

```
just build        # cargo build
just release      # cargo build --release
just test         # cargo test
just bench        # cargo run -- benchmark
just clippy       # cargo clippy -- -D warnings
just fmt          # cargo fmt --check
just fmt-fix      # cargo fmt
just lint         # clippy + fmt
just check        # lint + test + build
just serve        # cargo run -- serve
just clean        # cargo clean
```

## Technologies

| Tool | Version | Purpose |
|------|---------|---------|
| Rust | 2024 edition | Language |
| fastembed | 5.13 | ONNX embedding models |
| axum | 0.8 | HTTP server (REST + SSE) |
| clap | 4 | CLI argument parsing |
| notify | 7 | File watcher for hot-reload |
| blake3 | 1 | Content hashing for embedding cache |
| reqwest | 0.12 | HTTP client (CLI→daemon) |

## Conventions

- Binary name: `csn`
- Config: `~/.config/computer-says-no/config.toml` (env var `CSN_*` overrides)
- Reference sets: `~/.config/computer-says-no/reference-sets/`
- Cache: `~/.cache/computer-says-no/{model-name}/`
- Default port: 9847 (configurable)
- Conventional commits enforced via cocogitto

## Testing

- Unit tests: `cargo test --bin csn` (17 tests: config, model, classifier, reference_set, watcher, embedding_cache)
- Integration: `cargo test --test integration_test` (starts daemon subprocess, tests all REST endpoints)
- Benchmarks: planned for spec 002 (labeled datasets + accuracy validation)

## Active Technologies
- Rust 2024 edition (1.85+) + fastembed 5.13, axum 0.8, clap 4, notify 7, tokio 1 (001-core-binary-cli)
- Filesystem — TOML config, TOML reference sets, binary embedding cache (blake3-hashed) (001-core-binary-cli)
- Rust 2024 edition (1.85+) + fastembed 5.13 (existing), clap 4 (existing), serde_json (existing), indicatif (progress — already transitive dep of fastembed), comfy-table (NEW — table formatting) (002-model-benchmark-harness)
- JSON files in `datasets/` directory, JSON output for results (002-model-benchmark-harness)

## Recent Changes
- 001-core-binary-cli: Full implementation — config, server (RwLock + graceful shutdown), CLI (thin HTTP + standalone), file watcher, embedding cache (blake3), 17 unit tests + integration test
