# Computer Says No (`csn`)

Local embedding service for text classification using ONNX models via fastembed-rs.

## Architecture

Single Rust binary with CLI, MCP server, and lazy background daemon:
- **MCP server** — `csn mcp` runs over stdio, exposes classify/list_sets/embed/similarity tools
- **CLI** — classify, embed, similarity auto-route through background daemon when warm (~5ms), fall back to in-process (~254ms)
- **Daemon** — auto-starts on first CLI use via unix socket, self-exits after idle timeout (default 5min)

## Directory Structure

```
src/              Rust source (main.rs, config.rs, mcp.rs, daemon.rs, client.rs, classifier.rs, model.rs, reference_set.rs, embedding_cache.rs, mlp.rs)
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
| nix | 0.29 | Unix process/signal management (daemon PID checks) |
| blake3 | 1 | Content hashing for embedding cache |

## Conventions

- Binary name: `csn`
- Config: `~/.config/computer-says-no/config.toml` (env var `CSN_*` overrides)
- Reference sets: `~/.config/computer-says-no/reference-sets/`
- Cache: `~/.cache/computer-says-no/{model-name}/`
- Daemon socket: `~/.cache/computer-says-no/csn.sock`
- Daemon PID: `~/.cache/computer-says-no/csn.pid`
- Conventional commits enforced via cocogitto

## Testing

- Unit tests: `cargo test --bin csn` (68 tests: config, model, classifier, reference_set, embedding_cache, benchmark, dataset, mlp, mcp, daemon, client)
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
- nix 0.29 (signal, process), unix socket daemon, JSON lines protocol (005-lazy-daemon)
- Daemon files in `~/.cache/computer-says-no/` (csn.sock, csn.pid, csn.lock) (005-lazy-daemon)
- unicode-normalization 0.1, std::hash for character n-gram feature hashing (007-char-ngram-features)
- MLP input: 643-dim (384 embedding + 3 cosine + 256 char n-grams), cache key versioned v2-char256 (007-char-ngram-features)
- Multi-category MLP: softmax/cross-entropy, per-category cosine features (N*3), v3-multicat cache prefix (008-mlp-multi-category)
- corrections.toml: multi-category (correction/frustration/neutral), hook fires per-category prompts (008-mlp-multi-category)

## Recent Changes
- 008-mlp-multi-category: Multi-category MLP (softmax/CE) — corrections.toml restructured to correction/frustration/neutral, per-category hook prompts, 68 tests
- 007-char-ngram-features: Character n-gram features for typo robustness — 256-dim feature hashing, MLP input 643-dim, 55 tests
- 005-lazy-daemon: Lazy auto-starting background daemon — unix socket, idle timeout, auto-spawn, `csn stop`
- 004-mcp-sse: MCP stdio server — 4 tools (classify, list_sets, embed, similarity), removed REST daemon/watcher/standalone
- 003-mlp-classifier: MLP binary classifier — Burn framework, combined pipeline (embedding + cosine features → MLP), 94.4% accuracy, weight caching
- 002-model-benchmark-harness: Benchmark harness — 12-model comparison, 6 datasets (500 prompts each), strategy comparison, table/JSON output
- 001-core-binary-cli: Core binary — config, CLI, embedding cache (blake3), reference sets
