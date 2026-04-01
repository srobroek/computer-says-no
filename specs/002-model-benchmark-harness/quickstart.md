# Quickstart: Model Benchmark Harness

## Generate Datasets

```fish
# Generate labeled datasets from reference sets (one-time)
csn benchmark generate-datasets --sets-dir ./reference-sets --output-dir ./datasets
# Then use LLM to fill the scaffold with 500 diverse prompts per dataset
```

## Run Full Benchmark

```fish
# Compare all 12 models across all datasets
csn benchmark

# With JSON output saved to file
csn benchmark --json --output results/benchmark-2026-03-31.json
```

## Run Filtered Benchmark

```fish
# Single model
csn benchmark --model bge-small-en-v1.5-Q

# Single dataset
csn benchmark --dataset corrections

# Single combination (fastest)
csn benchmark --model bge-small-en-v1.5-Q --dataset corrections

# Custom iteration count
csn benchmark --iterations 50 --warmup 10
```

## Compare Results

```fish
# Run benchmark and compare against previous run
csn benchmark --output results/current.json --compare results/previous.json
```

## Reading Results

The comparison matrix shows per model:
- **Accuracy**: Overall % of correctly classified prompts
- **Edge Acc**: Accuracy on hardest tier (model differentiator)
- **p50/p95**: Warm classification latency in milliseconds
- **Cold**: Model load time in seconds

The "Best" line at the bottom recommends the model with highest accuracy. Check edge accuracy for nuanced use cases.
