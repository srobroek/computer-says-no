# Spec Dependency Graph

```mermaid
graph LR
    001[001: Core Binary + CLI]:::done
    002[002: Benchmark Harness]:::done
    003[003: MLP Classifier]:::done
    004[004: MCP Server]:::done
    005[005: Lazy Daemon]:::done
    007[007: Char N-gram Features]:::done
    008[008: MLP Multi-Category]:::ready

    001 --> 002
    001 --> 003
    001 --> 004
    002 --> 003
    004 --> 005
    003 --> 007
    007 --> 008
    003 --> 008

    classDef done fill:#2da44e,color:#fff
    classDef ready fill:#bf8700,color:#fff
    classDef future fill:#656d76,color:#fff
```

## Status

| Spec | Status | Description |
|------|--------|-------------|
| 001 | done | Core binary with CLI, REST daemon, config, reference sets, embedding cache |
| 002 | done | Benchmark harness — 12-model comparison, datasets, accuracy/latency measurement |
| 003 | done | MLP classifier — 2-layer neural network on embeddings + cosine features |
| 004 | done | MCP stdio server — 4 tools (classify, list_sets, embed, similarity) |
| 005 | done | Lazy auto-starting background daemon — unix socket, idle timeout, fast CLI |
| 007 | done | Character n-gram features — 256-dim feature hashing for typo robustness |
| 008 | ready | MLP multi-category — softmax output, corrections.toml restructure, per-category scoring |

## Ready Now

- **008-mlp-multi-category**: All dependencies met (003 MLP + 007 char n-grams both done)

## Critical Path

003 → 007 → 008 (MLP classifier → char features → multi-category)

## Dependency Details

| From → To | Why | Blocker |
|-----------|-----|---------|
| 002 → 001 | Benchmark uses EmbeddingEngine, ModelChoice, classify_text from core | — |
| 003 → 001 | MLP classifier extends the classification pipeline | — |
| 003 → 002 | Benchmark validates MLP accuracy gains (96.2% target) | — |
| 004 → 001 | MCP server wraps the core classification/embedding engine | — |
| 005 → 004 | Daemon adds unix socket transport alongside MCP stdio | — |
| 007 → 003 | Character features extend MLP input layer | — |
| 008 → 003 | Multi-category extends MLP architecture (sigmoid→softmax) | — |
| 008 → 007 | Multi-category MLP reuses char n-gram feature input | — |
