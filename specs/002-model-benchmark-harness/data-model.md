# Data Model: Model Benchmark Harness

## Entities

### LabeledPrompt

A single test entry within a dataset.

| Attribute | Type | Description |
|-----------|------|-------------|
| text | string | The prompt to classify |
| expected_label | string | Ground truth: "match"/"no-match" (binary) or category name (multi-category) |
| tier | enum | Difficulty: clear, moderate, edge |
| polarity | enum | positive (should match) or negative (should not match) |

### LabeledDataset

A collection of labeled prompts for benchmarking against a reference set.

| Attribute | Type | Description |
|-----------|------|-------------|
| name | string | Dataset name (matches reference set name) |
| reference_set | string | Target reference set to classify against |
| mode | enum | binary, multi-category |
| prompts | list<LabeledPrompt> | Test entries (~500) |
| generated | ISO datetime | When the dataset was created |

Stored as `datasets/{name}.json`.

### BenchmarkConfig

Configuration for a benchmark run.

| Attribute | Type | Default | Description |
|-----------|------|---------|-------------|
| models | list<string> | all 12 | Models to test |
| datasets | list<string> | all available | Datasets to test |
| warmup_iterations | integer | 5 | Warm-up runs before measuring |
| measured_iterations | integer | 20 | Measured runs per prompt |

### ModelResult

Results for a single model across all tested datasets.

| Attribute | Type | Description |
|-----------|------|-------------|
| model | string | Model name |
| dimensions | integer | Embedding dimensions |
| cold_startup_ms | float | Time from load to first embedding |
| datasets | list<DatasetResult> | Per-dataset results |

### DatasetResult

Results for a single model-dataset combination.

| Attribute | Type | Description |
|-----------|------|-------------|
| dataset | string | Dataset name |
| accuracy | float | Overall accuracy (0.0-1.0) |
| accuracy_by_tier | map<string, float> | Per-tier accuracy (clear_pos, moderate_pos, edge_pos, clear_neg, moderate_neg, edge_neg) |
| precision | float | True positives / (true positives + false positives) |
| recall | float | True positives / (true positives + false negatives) |
| latency_p50_ms | float | 50th percentile warm latency |
| latency_p95_ms | float | 95th percentile warm latency |
| latency_p99_ms | float | 99th percentile warm latency |
| latency_cv | float | Coefficient of variation (stability check) |
| total_prompts | integer | Number of prompts tested |
| correct | integer | Number correctly classified |

### BenchmarkRun

A complete benchmark execution.

| Attribute | Type | Description |
|-----------|------|-------------|
| timestamp | ISO datetime | When the run started |
| config | BenchmarkConfig | Run configuration |
| results | list<ModelResult> | Per-model results |
| system_info | string | Platform info for reproducibility |

Stored as JSON when `--output` is specified.

## Relationships

```
BenchmarkConfig  ──configures→  BenchmarkRun
BenchmarkRun  ──contains→  ModelResult[]
ModelResult  ──contains→  DatasetResult[]
LabeledDataset  ──tested-by→  DatasetResult
```

## State Transitions

### Benchmark lifecycle
```
Start → Parse Config → For Each Model:
  Load Model → Measure Cold Startup → Warm Up (5 iters) →
  For Each Dataset:
    Load Dataset → Classify All Prompts (20 iters each) →
    Compute Accuracy + Latency Percentiles → Store DatasetResult
  → Store ModelResult
→ Output Comparison Matrix → Done
```
