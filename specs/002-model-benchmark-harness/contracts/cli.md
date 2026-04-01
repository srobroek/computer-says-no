# CLI Contract: Benchmark

## csn benchmark

```
csn benchmark [--model <MODEL>] [--dataset <NAME>] [--iterations <N>] [--warmup <N>] [--json] [--output <PATH>] [--compare <PATH>]
```

- Runs classification benchmark across models and datasets
- Default: all models × all datasets in `datasets/`
- `--model`: test only specified model
- `--dataset`: test only specified dataset
- `--iterations`: measured iterations per prompt (default: 20)
- `--warmup`: warm-up iterations before measuring (default: 5)
- `--json`: output as structured JSON
- `--output <path>`: save results to file
- `--compare <path>`: diff against previous run, highlight regressions
- Exit code 0 on success, 1 on error

**Default output** (human-readable comparison matrix):

```
Model Benchmark Results (2 datasets, 12 models)
═══════════════════════════════════════════════════

corrections (binary, 500 prompts)
┌────────────────────────────┬──────────┬──────────┬──────────┬──────────┬──────────┐
│ Model                      │ Accuracy │ Edge Acc │ p50 (ms) │ p95 (ms) │ Cold (s) │
├────────────────────────────┼──────────┼──────────┼──────────┼──────────┼──────────┤
│ bge-small-en-v1.5-Q        │   87.2%  │   71.4%  │     3.2  │     5.1  │     0.8  │
│ nomic-embed-text-v1.5-Q    │   85.4%  │   68.9%  │     4.7  │     7.3  │     1.2  │
│ ...                        │          │          │          │          │          │
└────────────────────────────┴──────────┴──────────┴──────────┴──────────┴──────────┘

Best: bge-small-en-v1.5-Q (87.2% accuracy, 3.2ms p50)
```

**JSON output** (`--json`):

```json
{
  "timestamp": "2026-03-31T12:00:00Z",
  "config": {
    "models": ["all"],
    "datasets": ["corrections", "commit-types"],
    "warmup_iterations": 5,
    "measured_iterations": 20
  },
  "results": [
    {
      "model": "bge-small-en-v1.5-Q",
      "dimensions": 384,
      "cold_startup_ms": 823.4,
      "datasets": [
        {
          "dataset": "corrections",
          "accuracy": 0.872,
          "accuracy_by_tier": {
            "clear_pos": 0.976,
            "moderate_pos": 0.892,
            "edge_pos": 0.714,
            "clear_neg": 0.964,
            "moderate_neg": 0.880,
            "edge_neg": 0.698
          },
          "precision": 0.89,
          "recall": 0.86,
          "latency_p50_ms": 3.2,
          "latency_p95_ms": 5.1,
          "latency_p99_ms": 8.4,
          "latency_cv": 0.18,
          "total_prompts": 500,
          "correct": 436
        }
      ]
    }
  ]
}
```

**Comparison output** (`--compare previous.json`):

```
Regression Report (vs previous.json from 2026-03-30)

corrections:
  bge-small-en-v1.5-Q: accuracy 87.2% → 85.0% (▼ 2.2%)  ⚠️
  nomic-embed-text-v1.5-Q: accuracy 85.4% → 86.1% (▲ 0.7%)

commit-types:
  All models within tolerance (±1% accuracy, ±20% latency)
```

## csn benchmark generate-datasets

```
csn benchmark generate-datasets [--sets-dir <PATH>] [--output-dir <PATH>]
```

- Reads reference sets and outputs dataset scaffold templates
- `--sets-dir`: reference sets directory (default: from config)
- `--output-dir`: where to write datasets (default: `datasets/`)
- Generates one JSON file per reference set with structure for 500 prompts across 6 tiers
- Exit code 0 on success, 1 on error
