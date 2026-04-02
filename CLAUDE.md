# Computer Says No (`csn`)

Local embedding service for text classification using ONNX models via fastembed-rs.

## Architecture

Single Rust binary with CLI and MCP server:
- **MCP server** — `csn mcp` runs over stdio, exposes classify/list_sets/embed/similarity tools
- **CLI** — in-process commands: classify, embed, similarity, models, sets list, benchmark

## Directory Structure

```
src/              Rust source (main.rs, config.rs, mcp.rs, classifier.rs, model.rs, reference_set.rs, embedding_cache.rs, mlp.rs)
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
just mcp          # cargo run -- mcp
just clean        # cargo clean
```

## Technologies

| Tool | Version | Purpose |
|------|---------|---------|
| Rust | 2024 edition | Language |
| fastembed | 5.13 | ONNX embedding models |
| rust-mcp-sdk | 0.9 | MCP server (stdio transport) |
| clap | 4 | CLI argument parsing |
| burn | 0.20 | MLP neural network (ndarray backend) |
| blake3 | 1 | Content hashing for embedding cache |

## Conventions

- Binary name: `csn`
- Config: `~/.config/computer-says-no/config.toml` (env var `CSN_*` overrides)
- Reference sets: `~/.config/computer-says-no/reference-sets/`
- Cache: `~/.cache/computer-says-no/{model-name}/`
- Conventional commits enforced via cocogitto

## Testing

- Unit tests: `cargo test --bin csn` (40 tests: config, model, classifier, reference_set, embedding_cache, benchmark, dataset, mlp, mcp)
- Integration: `cargo test --test integration_test -- --ignored` (spawns `csn mcp`, tests MCP protocol via stdio — requires model download)
- Benchmark tests: `cargo test --test benchmark_test` (validates dataset structure, labels, tier distribution)
- Benchmarks: `just bench` (runs `csn benchmark run` — requires model download, not available in CI/sandbox)

## Active Technologies
- Rust 2024 edition (1.92) + fastembed 5.13, clap 4, tokio 1 (001-core-binary-cli)
- Filesystem — TOML config, TOML reference sets, binary embedding cache (blake3-hashed) (001-core-binary-cli)
- comfy-table 7, indicatif 0.17, serde_json (002-model-benchmark-harness)
- JSON files in `datasets/` directory, JSON output for results (002-model-benchmark-harness)
- burn 0.20+ (burn-ndarray), MLP weight cache in `~/.cache/computer-says-no/mlp/{hash}.mpk` (003-mlp-classifier)
- rust-mcp-sdk 0.9 (stdio transport, tool macros), async-trait (004-mcp-sse)

## Recent Changes
- 004-mcp-sse: MCP stdio server — 4 tools (classify, list_sets, embed, similarity), removed REST daemon/watcher/standalone, 40 unit tests + MCP integration test
- 003-mlp-classifier: MLP binary classifier — Burn framework, combined pipeline (embedding + cosine features → MLP), 94.4% accuracy, weight caching
- 002-model-benchmark-harness: Benchmark harness — 12-model comparison, 6 datasets (500 prompts each), strategy comparison, table/JSON output
- 001-core-binary-cli: Core binary — config, CLI, embedding cache (blake3), reference sets
