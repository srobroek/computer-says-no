# Research: Model Benchmark Harness

## Progress Indicators

**Decision**: Use `indicatif` directly
**Rationale**: Already a transitive dependency via `hf-hub` → `fastembed`. Zero additional binary size. Blessed.rs listed, battle-tested for progress bars and spinners.
**Alternatives**: None needed — already in dependency tree.

## Percentile Calculation

**Decision**: Inline implementation (~5 lines)
```rust
let mut sorted = latencies.clone();
sorted.sort();
let p50 = sorted[sorted.len() / 2];
let p95 = sorted[sorted.len() * 95 / 100];
let p99 = sorted[sorted.len() * 99 / 100];
```
**Rationale**: Principle V (Simplicity). Three lines of math don't justify a dependency.
**Alternatives rejected**:
- `hdrhistogram`: Overkill for 20 data points, 50KB+ binary impact
- `quantiles`: Dependency for trivial operation

## Table Formatting

**Decision**: Use `comfy-table`
**Rationale**: 1.7M downloads, actively maintained, blessed.rs listed. Clean API with auto-width and UTF-8 box drawing. Single new dependency.
**Alternatives rejected**:
- Manual `println!`: Error-prone alignment for wide matrices
- `tabled`: Heavier API, more complex than needed
- `prettytable-rs`: Archived/unmaintained

## Dataset Generation Approach

**Decision**: LLM-generated via subagents during implementation, stored as static JSON
**Rationale**: Reference set phrases are seeds. LLM generates diverse, realistic prompts per tier (clear/moderate/edge × positive/negative). Labels come from generation context. The `generate-datasets` command in the binary loads reference sets and outputs a scaffold template; actual prompt generation happens via LLM during speckit implementation.
**Alternatives rejected**:
- Hand-crafted: 500+ prompts per dataset is impractical to write manually
- Template-based: Produces repetitive, unrealistic prompts

## Benchmark Architecture

**Decision**: Sequential model iteration, in-process (standalone) path
**Rationale**: Each model loads once → warm-up → run all datasets → report → next model. Avoids model reload per dataset. Standalone path eliminates network overhead from measurements. Models tested sequentially because GPU/CPU resources are shared.
**Alternatives rejected**:
- Daemon path: Adds HTTP overhead to latency measurements
- Parallel models: Memory-constrained (large models ~500MB each), misleading latency numbers

## Summary

| Need | Solution | New deps |
|------|----------|----------|
| Progress bars | `indicatif` (existing transitive) | 0 |
| Percentiles | Inline sort+index | 0 |
| Table output | `comfy-table` | 1 |
| Dataset gen | LLM via subagents | 0 |

**Total new dependencies**: 1 (`comfy-table`)
