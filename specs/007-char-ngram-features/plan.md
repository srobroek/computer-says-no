# Implementation Plan: Character N-gram Features

**Branch**: `007-char-ngram-features` | **Date**: 2026-04-02 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/007-char-ngram-features/spec.md`

## Summary

Add 256-dimensional character n-gram features (bigrams + trigrams via feature hashing) to the MLP classifier input, alongside the existing 387-dim embedding + cosine features. MLP input grows from 387 to 643 dimensions. This provides typo robustness without changing the model architecture, API surface, or reference set format.

## Technical Context

**Language/Version**: Rust 2024 edition (1.92)
**Primary Dependencies**: burn 0.20 (existing), std::hash, unicode-normalization 0.1 (new, NFC normalization)
**Storage**: MLP weight cache in `~/.cache/computer-says-no/mlp/` (cache key versioned)
**Testing**: cargo test (unit + benchmark)
**Target Platform**: macOS (primary), Linux (secondary)
**Project Type**: CLI tool — internal ML pipeline change
**Performance Goals**: <5ms latency increase per classification, no accuracy regression
**Constraints**: No new dependencies, no API changes, no reference set format changes
**Scale/Scope**: Single module change (mlp.rs) + classifier integration

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Single Binary | PASS | No new dependencies — uses std::hash |
| II. MCP + Lazy Daemon | PASS | No transport changes — internal ML improvement |
| III. Configurable Classification | PASS | Reference set format unchanged |
| IV. Fast-Start Performance | PASS | <5ms overhead, cache retrain only on first run |
| V. Simplicity | PASS | ~100 lines of new code, no new abstractions |

No violations.

## Project Structure

### Documentation (this feature)

```text
specs/007-char-ngram-features/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
└── tasks.md
```

### Source Code (changes only)

```text
src/
├── mlp.rs           # Add char_ngram_features(), update training loop, bump cache version
├── classifier.rs    # Pass character features through classify pipeline
└── (all other files unchanged)
```

**Structure Decision**: No new files. Character feature extraction lives in `mlp.rs` alongside the existing feature computation (cosine features). The function is ~50 lines.
