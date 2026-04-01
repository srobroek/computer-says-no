# Tasks: Model Benchmark Harness

**Input**: Design documents from `/specs/002-model-benchmark-harness/`
**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup

**Purpose**: Add benchmark dependencies and dataset directory structure

- [X] T001 Add comfy-table and indicatif dependencies to Cargo.toml, add datasets_dir to AppConfig in src/config.rs
- [X] T002 [P] Create src/dataset.rs: define LabeledPrompt, LabeledDataset, Tier, Polarity structs with serde Serialize/Deserialize, add load_dataset and load_all_datasets functions
- [X] T003 [P] Create src/benchmark.rs: define BenchmarkConfig, ModelResult, DatasetResult, BenchmarkRun structs with serde Serialize/Deserialize

**Checkpoint**: Types compile, dataset loading works from JSON files

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Benchmark measurement core and dataset generation scaffold — all user stories depend on these

**CRITICAL**: No user story work can begin until this phase is complete

- [X] T004 Implement benchmark measurement loop in src/benchmark.rs: load model → measure cold startup → warm-up iterations → measured iterations → compute percentiles (inline sort+index for p50/p95/p99) and accuracy per dataset
- [X] T005 Implement accuracy calculation in src/benchmark.rs: compare ClassifyResult against LabeledPrompt expected_label, compute overall accuracy, per-tier accuracy breakdown, precision, recall
- [X] T006 [P] Implement dataset generation scaffold in src/dataset.rs: read reference sets, output JSON template with tier/polarity structure for LLM filling via generate_dataset_scaffold function
- [X] T007 Add benchmark and generate-datasets subcommands to CLI in src/main.rs with clap: benchmark flags (--model, --dataset, --iterations, --warmup, --json, --output, --compare), generate-datasets flags (--sets-dir, --output-dir)

**Checkpoint**: Benchmark loop runs against a single model + test dataset, generate-datasets outputs scaffold JSON

---

## Phase 3: User Story 1 — Run Model Comparison Benchmark (Priority: P1)

**Goal**: Compare all 12 models across all datasets with warm-only latency and accuracy

**Independent Test**: Run `csn benchmark --dataset corrections` and verify comparison matrix output

### Implementation for User Story 1

- [X] T008 [US1] Implement full benchmark orchestration in src/benchmark.rs: iterate all models (or filtered by --model), all datasets (or filtered by --dataset), collect ModelResult vec, display progress via indicatif ProgressBar
- [X] T009 [US1] Implement human-readable table output in src/benchmark.rs using comfy-table: model × dataset matrix showing accuracy, edge accuracy, p50, p95, cold startup, with "Best" recommendation per dataset
- [X] T010 [US1] Implement --json output in src/benchmark.rs: serialize BenchmarkRun to JSON via serde_json, support --output <path> to write to file
- [X] T011 [US1] Wire benchmark orchestration into src/main.rs: load config, resolve datasets_dir, call run_benchmark, handle output format

**Checkpoint**: `csn benchmark` runs all models against test dataset, outputs comparison table. US1 independently testable.

---

## Phase 4: User Story 2 — Generate Labeled Datasets (Priority: P1)

**Goal**: Create 500-prompt labeled datasets for each reference set via LLM generation

**Independent Test**: Run `csn benchmark generate-datasets` and verify it outputs scaffold JSON, then fill with LLM-generated prompts

### Implementation for User Story 2

- [X] T012 [US2] Implement generate_dataset_scaffold in src/dataset.rs: read reference set metadata (name, mode, categories/phrases), output JSON with tier/polarity structure and example prompts as seeds
- [X] T013 [US2] Generate corrections dataset (500 prompts) in datasets/corrections.json: use LLM subagent to create 83 prompts per tier (clear_pos, moderate_pos, edge_pos, clear_neg, moderate_neg, edge_neg) seeded from corrections.toml phrases
- [X] T014 [P] [US2] Generate commit-types dataset (500 prompts) in datasets/commit-types.json: use LLM subagent to create 83 prompts per tier seeded from commit-types.toml categories
- [X] T015 [US2] Wire generate-datasets into src/main.rs: read reference sets from sets_dir, call generate_dataset_scaffold, write to output_dir

**Checkpoint**: datasets/ contains corrections.json and commit-types.json with 500 labeled prompts each. US2 independently testable.

---

## Phase 5: User Story 3 — Benchmark Individual Model or Dataset (Priority: P2)

**Goal**: Filter benchmark to specific model and/or dataset combinations

**Independent Test**: Run `csn benchmark --model bge-small-en-v1.5-Q --dataset corrections` and verify only that combination runs

### Implementation for User Story 3

- [X] T016 [US3] Implement --model filter in src/benchmark.rs: parse model name, validate against ModelChoice::all(), run only matching model
- [X] T017 [US3] Implement --dataset filter in src/benchmark.rs: validate dataset name against available files in datasets_dir, run only matching dataset
- [X] T018 [US3] Handle filter edge cases: invalid model name → error listing available models, invalid dataset → error listing available datasets, no datasets found → suggest generate-datasets

**Checkpoint**: Filtered benchmarks work. `csn benchmark --model X --dataset Y` runs single combination. US3 independently testable.

---

## Phase 6: User Story 4 — Save and Compare Benchmark Results (Priority: P3)

**Goal**: Save results to file and compare against previous runs to detect regressions

**Independent Test**: Run benchmark twice with --output, then use --compare to diff

### Implementation for User Story 4

- [X] T019 [US4] Implement --compare in src/benchmark.rs: load previous BenchmarkRun from JSON, diff accuracy and latency per model-dataset combination, report regressions (accuracy drop >1%, latency increase >20%)
- [X] T020 [US4] Implement regression display in src/benchmark.rs: format comparison output with arrows (▲/▼), highlight regressions with warning markers

**Checkpoint**: `csn benchmark --output a.json` then `csn benchmark --compare a.json` shows diff. US4 independently testable.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Quality, validation, and documentation

- [X] T021 Add unit tests for percentile calculation and accuracy computation in src/benchmark.rs
- [X] T022 [P] Add unit tests for dataset loading and scaffold generation in src/dataset.rs
- [X] T023 Add integration test: run benchmark with test dataset (small, 6 prompts) against one model, verify output structure in tests/benchmark_test.rs
- [X] T024 Run cargo clippy -- -D warnings, cargo fmt --check, cargo test — fix any issues
- [ ] T025 Validate SC-003 (85% corrections accuracy) and SC-004 (80% commit-types accuracy) by running full benchmark against generated datasets (deferred: requires model loading outside sandbox/CI)
- [X] T026 Update justfile: add `just bench` target that runs `cargo run -- benchmark`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on T001-T003 (types and deps)
- **US1 (Phase 3)**: Depends on Phase 2 (measurement loop, accuracy calc)
- **US2 (Phase 4)**: Depends on T006 (scaffold generation). Can run in parallel with US1.
- **US3 (Phase 5)**: Depends on Phase 3 (benchmark orchestration exists to filter)
- **US4 (Phase 6)**: Depends on Phase 3 (benchmark output exists to compare)
- **Polish (Phase 7)**: Depends on all user stories

### Parallel Opportunities

- T002 + T003: dataset.rs and benchmark.rs types touch different files
- T006 + T004: scaffold generation and measurement loop are independent
- T013 + T014: corrections and commit-types datasets can be generated in parallel (different subagents)
- US1 and US2 can be worked in parallel after Phase 2
- T021 + T022: test files are independent

### Within Each User Story

- Types and structs before logic
- Core implementation before output formatting
- Story complete before moving to next priority

---

## Implementation Strategy

### MVP First (User Story 1 + 2)

1. Complete Phase 1: Setup (deps, types)
2. Complete Phase 2: Foundational (measurement loop, accuracy, scaffold)
3. Complete Phase 4: US2 (generate datasets — needed before US1 can produce meaningful results)
4. Complete Phase 3: US1 (run benchmark against generated datasets)
5. **STOP and VALIDATE**: Run full benchmark, verify SC-003/SC-004 accuracy targets
6. Deploy/demo if ready

### Incremental Delivery

1. Setup + Foundational → Types and measurement core ready
2. Add US2 (datasets) → 500-prompt datasets exist → Can test accuracy
3. Add US1 (benchmark) → Full comparison matrix → MVP complete
4. Add US3 (filtering) → Quick single-model runs
5. Add US4 (comparison) → Regression detection
6. Polish → Tests, clippy, justfile → Release ready
