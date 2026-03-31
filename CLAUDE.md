# Computer Says No (`csn`)

Local embedding service for text classification using ONNX models via fastembed-rs.

## Architecture

Single Rust binary serving two protocols on one port:
- **REST API** — hooks and CLI call `/classify`, `/embed`, `/similarity`, `/health`, `/sets`
- **MCP/SSE** — Claude Code agent connects to `/sse` for tool access

## Directory Structure

```
src/              Rust source (main.rs, server.rs, classifier.rs, model.rs, reference_set.rs, watcher.rs, service.rs)
reference-sets/   Default TOML reference sets (shipped with binary)
datasets/         Labeled test prompts for benchmarking (JSON)
hooks/            Example Claude Code hook scripts
tests/            Integration tests and benchmark harness
docs/             Documentation
scripts/          Build tooling, automation
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
| notify | 7 | Cross-platform file watcher |
| service-manager | 0.7 | Cross-platform service install |
| blake3 | 1 | Content hashing for cache |
| reqwest | 0.12 | HTTP client (CLI→daemon, remote sets) |

## Conventions

- Binary name: `csn`
- Config: `~/.config/computer-says-no/config.toml` (env var `CSN_*` overrides)
- Reference sets: `~/.config/computer-says-no/reference-sets/`
- Cache: `~/.cache/computer-says-no/{model-name}/`
- Default port: 9847 (configurable)
- Conventional commits enforced via cocogitto

## Testing

- Unit tests: `cargo test`
- Benchmarks: `csn benchmark` (accuracy + latency across models × datasets)
- Integration: start daemon → run hooks → verify classification output

## Active Technologies
- Rust 2024 edition (1.85+) + fastembed 5.13, axum 0.8, clap 4, notify 7, tokio 1 (001-core-binary-cli)
- Filesystem — TOML config, TOML reference sets, binary embedding cache (blake3-hashed) (001-core-binary-cli)

## Recent Changes
- 001-core-binary-cli: Added Rust 2024 edition (1.85+) + fastembed 5.13, axum 0.8, clap 4, notify 7, tokio 1
