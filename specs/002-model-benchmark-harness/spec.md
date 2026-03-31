# Feature Specification: Model Benchmark Harness

**Feature Branch**: `002-model-benchmark-harness`
**Created**: 2026-03-31
**Status**: Draft
**Input**: User description: "Benchmark harness for csn — compare all 12 embedding models across multiple generated datasets measuring warm-only classification latency and accuracy. Generate labeled datasets for different use cases (correction detection, commit type classification, and additional categories). Benchmark must exclude cold startup time by warming the model first. Output a comparison matrix of model × dataset with latency p50/p95/p99 and accuracy metrics. Dataset generation can be parallelized across subagents."

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Run Model Comparison Benchmark (Priority: P1)

A developer wants to choose the best embedding model for their use case. They run a single benchmark command that tests all 12 supported models against all available labeled datasets, measuring both accuracy and warm classification latency. The output is a comparison matrix showing which model performs best for each dataset.

**Why this priority**: Model selection is the primary decision this feature supports. Without it, users guess which model to use.

**Independent Test**: Run `csn benchmark` and verify it produces a comparison table with accuracy and latency for each model × dataset combination.

**Acceptance Scenarios**:

1. **Given** labeled datasets exist in the datasets directory, **When** the user runs `csn benchmark`, **Then** the system tests each model against each dataset and outputs a comparison matrix with accuracy and latency metrics.
2. **Given** the benchmark is running, **When** a model is first loaded, **Then** the system performs warm-up iterations before measuring, ensuring cold startup is excluded from latency metrics.
3. **Given** the benchmark completes, **When** the user views the results, **Then** each cell in the matrix shows accuracy percentage, warm latency at p50/p95/p99, and cold startup time.
4. **Given** the user wants machine-readable output, **When** they pass `--json`, **Then** the results are output as structured JSON.
5. **Given** a model is being benchmarked, **When** the system measures cold startup, **Then** it records the time from model load initiation to first successful embedding, separately from warm latency.

---

### User Story 2 — Generate Labeled Datasets (Priority: P1)

A developer needs labeled test datasets to validate classification accuracy. They run a dataset generation command that creates labeled prompt collections for different use cases: correction detection, commit type classification, and additional categories. Each dataset contains prompts with their expected classification result (ground truth).

**Why this priority**: Datasets are a prerequisite for the benchmark. Without labeled data, accuracy cannot be measured.

**Independent Test**: Run `csn benchmark generate-datasets` and verify it creates JSON files with labeled prompts for each reference set.

**Acceptance Scenarios**:

1. **Given** reference sets exist (corrections, commit-types), **When** the user runs dataset generation, **Then** the system creates labeled datasets with at least 50 prompts per reference set.
2. **Given** a binary reference set (corrections), **When** a dataset is generated, **Then** each entry contains a text prompt and the expected match/no-match label.
3. **Given** a multi-category reference set (commit-types), **When** a dataset is generated, **Then** each entry contains a text prompt and the expected category label.
4. **Given** multiple reference sets, **When** datasets are generated, **Then** generation for independent sets can happen in parallel.

---

### User Story 3 — Benchmark Individual Model or Dataset (Priority: P2)

A developer wants to benchmark a specific model or test against a specific dataset without running the full matrix. They pass flags to narrow the scope.

**Why this priority**: Useful for quick iteration when tuning a specific reference set or evaluating a single model candidate.

**Independent Test**: Run `csn benchmark --model bge-small-en-v1.5-Q --dataset corrections` and verify it runs only that combination.

**Acceptance Scenarios**:

1. **Given** the user specifies `--model`, **When** the benchmark runs, **Then** only the specified model is tested (against all datasets unless `--dataset` is also specified).
2. **Given** the user specifies `--dataset`, **When** the benchmark runs, **Then** only the specified dataset is tested (against all models unless `--model` is also specified).
3. **Given** the user specifies both `--model` and `--dataset`, **When** the benchmark runs, **Then** only that single combination is tested.

---

### User Story 4 — Save and Compare Benchmark Results (Priority: P3)

A developer wants to track benchmark results over time to detect regressions. They save results to a file and compare against previous runs.

**Why this priority**: Regression detection is valuable but not needed for the initial benchmark workflow.

**Independent Test**: Run `csn benchmark --output results.json`, modify a reference set, re-run, and compare.

**Acceptance Scenarios**:

1. **Given** the user passes `--output <path>`, **When** the benchmark completes, **Then** results are saved to the specified file in JSON format.
2. **Given** a previous results file exists, **When** the user passes `--compare <path>`, **Then** the output highlights accuracy or latency regressions.

---

### Edge Cases

- What happens when a model has not been downloaded yet? The system MUST download it during benchmark setup (before timing starts), not during measured iterations.
- What happens when a dataset file is malformed? The system MUST skip it with a warning and continue with remaining datasets.
- What happens when a model produces embeddings of different dimensions than a dataset was generated with? The system MUST re-embed the dataset for each model (datasets store text + labels, not precomputed embeddings).
- What happens when the benchmark is interrupted? Partial results for completed model-dataset combinations MUST still be available.
- What happens when no datasets exist? The system MUST suggest running dataset generation first.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide a `csn benchmark` subcommand that runs classification accuracy and latency tests across models and datasets.
- **FR-002**: System MUST test all 12 supported models unless filtered by `--model`.
- **FR-003**: System MUST test all available labeled datasets unless filtered by `--dataset`.
- **FR-004**: System MUST warm each model before measurement by running a configurable number of warm-up iterations (default: 5) to exclude cold startup from latency.
- **FR-005**: System MUST measure warm classification latency across 20 measured iterations per prompt (configurable via `--iterations`) and report p50, p95, and p99 percentiles.
- **FR-005a**: System MUST measure cold startup latency per model (time from model load to first successful embedding) and include it in results.
- **FR-006**: System MUST compute accuracy as the percentage of correctly classified prompts per model-dataset combination, with per-tier breakdown (clear/moderate/edge) in detailed output.
- **FR-007**: System MUST output results as a human-readable comparison matrix to stdout by default.
- **FR-008**: System MUST support `--json` for machine-readable output containing all metrics.
- **FR-009**: System MUST support `--output <path>` to save results to a file.
- **FR-010**: System MUST provide a `csn benchmark generate-datasets` subcommand that creates labeled test datasets.
- **FR-011**: System MUST generate at least 500 labeled prompts per reference set using LLM-based generation from reference set phrases as seeds, distributed across three difficulty tiers for both positive and negative examples (6 buckets, ~83 each).
- **FR-011a**: Clear positive (~83): unambiguous matches where the correct label is obvious (baseline recall).
- **FR-011b**: Moderate positive (~83): clearly the right label but with some semantic overlap (tests recall under ambiguity).
- **FR-011c**: Edge positive (~83): borderline matches close to the decision boundary — soft corrections vs suggestions, commit messages straddling categories (differentiates model recall).
- **FR-011d**: Clear negative (~83): completely unrelated text — off-topic questions, greetings, random statements (baseline precision).
- **FR-011e**: Moderate negative (~83): topically related but not a match — asking about the topic rather than being an instance of it (tests precision under relevance).
- **FR-011f**: Edge negative (~83): semantically close to a match but should not trigger — false-positive traps that test model discrimination (differentiates model precision).
- **FR-012**: System MUST store datasets as JSON files in the datasets directory with text, expected label, difficulty tier, and reference set name.
- **FR-013**: System MUST support `--compare <path>` to diff results against a previous benchmark run, highlighting regressions.
- **FR-014**: System MUST display a progress indicator during benchmark execution showing current model, dataset, and iteration count.

### Key Entities

- **Benchmark Run**: A complete execution covering one or more model-dataset combinations, with configuration (warm-up count, iterations) and timestamp.
- **Labeled Dataset**: A JSON file containing an array of test entries, each with input text, expected classification result, and metadata about the reference set.
- **Benchmark Result**: Per model-dataset combination: accuracy percentage, warm latency percentiles (p50/p95/p99), cold startup time, iteration count, and model metadata (name, dimensions).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can compare all 12 models across all datasets in a single command and receive a ranked comparison within 30 minutes. Single-model runs complete within 3 minutes.
- **SC-002**: Latency measurements exclude cold startup — the coefficient of variation for warm latency across iterations MUST be below 30% (stable measurement).
- **SC-003**: The corrections dataset achieves at least 85% accuracy with the best-performing model.
- **SC-004**: The commit-types dataset achieves at least 80% accuracy with the best-performing model.
- **SC-005**: Each generated dataset contains at least 500 labeled prompts across 6 difficulty tiers (3 positive, 3 negative), with roughly equal distribution per tier (~83 each). Per-tier accuracy confidence interval MUST be under ±8% at 95% confidence.
- **SC-006**: Benchmark results are reproducible — running the same benchmark twice on the same machine produces accuracy within 1% and latency within 20%.

## Clarifications

### Session 2026-03-31

- Q: How should labeled datasets be generated? → A: LLM-generated from reference set phrases as seeds, with labels derived from generation context. Parallelizable per reference set via subagents.
- Q: How many measured iterations per prompt for latency percentiles? → A: 20 iterations per prompt (default), configurable via `--iterations` flag.
- Q: Should datasets include negative examples? → A: Yes. 3 difficulty tiers for both positives and negatives (6 buckets total: clear/moderate/edge × positive/negative). Tests both recall and precision, with edge tiers differentiating model quality.

## Assumptions

- The benchmark runs on the same machine as the development environment — no remote execution or CI integration in this spec.
- All 12 models can be downloaded and cached locally. Benchmark setup (model download) is not time-bounded — only measured iterations are.
- Labeled datasets are static JSON files generated once and stored in the repository. They can be regenerated but don't change between benchmark runs unless explicitly regenerated.
- Dataset generation uses an LLM to generate diverse, realistic test prompts from reference set phrases as seeds. Labels are derived from the generation context (e.g., prompts generated as "correction examples" are labeled as matches). Each reference set can be generated independently in parallel via subagents.
- The benchmark uses the standalone (in-process) path, not the daemon, to avoid network overhead in measurements. Each model is loaded once, warmed up, then all datasets are run against it before moving to the next model.
- Dataset generation is a one-time setup step — it does not need to be fast. Accuracy and coverage matter more than generation speed.
