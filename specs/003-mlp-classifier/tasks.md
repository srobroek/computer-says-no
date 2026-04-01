# Tasks: MLP Pushback Classifier

**Input**: Design documents from `/specs/003-mlp-classifier/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup

**Purpose**: Add Burn dependency and create the MLP module skeleton

- [ ] T001 Add `burn` and `burn-ndarray` dependencies to Cargo.toml, feature-gate ndarray BLAS/Accelerate backend for macOS
- [ ] T002 Create `src/mlp.rs` module file with `MlpClassifier` struct using `#[derive(Module, Debug)]` (Linear layers: 387→256, 256→128, 128→1, Relu activation) and `MlpConfig` using `#[derive(Config, Debug)]` with fields: input_dim (387), hidden1 (256), hidden2 (128). Implement `MlpConfig::init()` to create the model. Add `mod mlp;` to `src/main.rs`
- [ ] T003 Add `[mlp]` config section to `AppConfig` in `src/config.rs`: `mlp_fallback: bool` (default false), `mlp_learning_rate: f64` (0.001), `mlp_weight_decay: f64` (0.001), `mlp_max_epochs: usize` (500), `mlp_patience: usize` (10). Parse from TOML and `CSN_MLP_FALLBACK` env var

---

## Phase 2: Foundational — MLP Training & Inference

**Purpose**: Core MLP training loop and weight persistence. MUST complete before user stories.

- [ ] T004 Implement `MlpClassifier::forward()` in `src/mlp.rs`: input Tensor<B, 2> (batch, 387) → linear1 → relu → linear2 → relu → output → sigmoid. Return Tensor<B, 2> (batch, 1) with sigmoid probability
- [ ] T005 Implement `train_mlp()` in `src/mlp.rs`: accept positive/negative phrase embeddings (Vec<Embedding>), build training data by computing cosine features (max_pos, max_neg, margin) and concatenating with raw embeddings to form 387-dim input tensors. Labels: 1.0 for positive, 0.0 for negative. Use `Autodiff<NdArray<f32>>` backend, `AdamConfig::new().with_weight_decay(weight_decay)` optimizer, `BinaryCrossEntropyLoss`, full-batch training. Implement early stopping: track validation loss, stop after `patience` epochs of no improvement. Return trained model (inference mode via `model.valid()`) or error if no convergence
- [ ] T006 Implement `compute_cosine_features()` in `src/mlp.rs`: given a text embedding and reference set positive/negative embeddings, compute max positive cosine similarity, max negative cosine similarity, and margin (max_pos - max_neg). Return [f32; 3]. Reuse `cosine_similarity` from `src/model.rs`
- [ ] T007 Implement weight cache in `src/mlp.rs`: `save_weights()` using `NamedMpkFileRecorder` to `~/.cache/computer-says-no/mlp/{content_hash}.mpk`, and `load_weights()` to load from cache. Content hash = blake3 of sorted concatenated positive + negative phrases. `cache_path()` helper to resolve the `.mpk` path from config cache_dir + hash
- [ ] T008 Implement `TrainedModel` struct in `src/mlp.rs`: holds `MlpClassifier<NdArray<f32>>` (inference backend), reference set name, content hash, and cloned positive/negative embeddings for cosine feature computation at inference time

**Checkpoint**: MLP can train, infer, save, and load weights. No integration yet.

---

## Phase 3: User Story 1 — Enhanced Classification via Combined Pipeline (Priority: P1) 🎯 MVP

**Goal**: `/classify` automatically uses combined pipeline (embedding + cosine features → MLP) for binary sets with trained models. Transparent — no API changes.

**Independent Test**: POST to `/classify` with pushback text, verify higher confidence than pure cosine baseline.

- [ ] T009 [US1] Implement `classify_with_mlp()` in `src/classifier.rs`: given text embedding, TrainedModel reference, compute cosine features via `compute_cosine_features()`, concatenate with embedding into 387-dim input, run MLP forward, return `BinaryResult` with confidence = sigmoid output, scores.positive/negative = raw cosine values, top_phrase from best_match
- [ ] T010 [US1] Modify `classify()` in `src/classifier.rs`: accept `Option<&TrainedModel>` parameter. When `Some` and reference set is binary, delegate to `classify_with_mlp()`. When `None`, use existing pure cosine path. Update `classify_text()` signature accordingly
- [ ] T011 [US1] Unit tests in `src/mlp.rs`: test MlpConfig::init creates model with correct layer dimensions, test forward pass produces output in (0, 1) range for random input, test compute_cosine_features returns correct values for known embeddings
- [ ] T012 [US1] Unit test in `src/classifier.rs`: test classify_with_mlp returns BinaryResult with MLP confidence and raw cosine scores

**Checkpoint**: Combined pipeline works in-process. Not yet wired to daemon.

---

## Phase 4: User Story 2 — Automatic Training at Startup (Priority: P1)

**Goal**: Daemon trains MLP at startup from reference set phrases. No manual steps.

**Independent Test**: Start daemon, verify logs show MLP training, then `/classify` uses combined pipeline.

- [ ] T013 [US2] Implement `train_models_at_startup()` in `src/mlp.rs`: iterate loaded reference sets, skip non-binary / no negatives / <4 phrases (FR-006, FR-007, FR-008), attempt to load from cache first (FR-004), train if cache miss, save weights on success. On convergence failure: if mlp_fallback=true log warning and continue, else return error (FR-012). Return `Vec<TrainedModel>`
- [ ] T014 [US2] Add `trained_models: RwLock<Vec<TrainedModel>>` to `AppState` in `src/server.rs`. In `serve()`: call `train_models_at_startup()` after reference sets load, store results in AppState
- [ ] T015 [US2] Wire `handle_classify` in `src/server.rs`: look up TrainedModel by reference set name from AppState, pass to `classify_text()`. If no trained model found, pure cosine is used (existing behavior)
- [ ] T016 [US2] Add startup logging: log MLP training duration per set, cache hit/miss, skip reasons. Use `tracing::info!` for success, `tracing::warn!` for skips and fallbacks

**Checkpoint**: Daemon starts with MLP, classify uses combined pipeline, fallback works.

---

## Phase 5: User Story 3 — Cached Weights for Fast Restart (Priority: P2)

**Goal**: Trained weights cached to disk. Restart loads from cache when set unchanged.

**Independent Test**: Start daemon (trains + caches), restart (loads from cache — faster), modify set, restart (retrains).

- [ ] T017 [US3] Integration test in `tests/integration_test.rs`: start daemon, verify `/classify` returns result (confirms MLP loaded). This is a [MANUAL] test requiring model download — validate structure only in CI
- [ ] T018 [US3] Unit test in `src/mlp.rs`: test save_weights/load_weights round-trip — train a small model, save, load, verify forward pass produces same output. Test cache invalidation: change phrases, verify new hash doesn't match old cache file

**Checkpoint**: Cache works, invalidation works, fast restart confirmed.

---

## Phase 6: User Story 4 — Benchmark Combined vs Cosine (Priority: P2)

**Goal**: Benchmark harness can compare combined pipeline accuracy alongside cosine strategies.

**Independent Test**: Run `csn benchmark compare-strategies` and see "combined" in the output.

- [ ] T019 [US4] Add "combined" strategy to `compare_strategies()` in `src/benchmark.rs`: train MLP for the reference set, then classify each prompt using combined pipeline, compute accuracy alongside existing threshold/margin/adaptive strategies
- [ ] T020 [US4] Wire `csn benchmark compare-strategies` in `src/main.rs` to use the new combined strategy. Ensure strategy table output includes "combined" row with accuracy

**Checkpoint**: Benchmark validates MLP accuracy improvement over cosine.

---

## Phase 7: User Story 5 — Hot-Reload Retrains MLP (Priority: P3)

**Goal**: File watcher retrains MLP on reference set change without blocking requests.

**Independent Test**: With daemon running, modify corrections.toml, verify retrain in logs, classify reflects new model.

- [ ] T021 [US5] Extend `handle_set_change()` in `src/watcher.rs`: after re-embedding reference sets, retrain MLP for changed binary sets. Swap new TrainedModel into `AppState.trained_models` atomically via RwLock write. Old model continues serving until swap completes
- [ ] T022 [US5] Unit test in `src/watcher.rs`: verify that a simulated set change triggers MLP retrain logic (mock or minimal test)

**Checkpoint**: Hot-reload retrains MLP, requests unblocked during retrain.

---

## Phase 8: Polish & Cross-Cutting

**Purpose**: Cleanup, docs, final validation

- [ ] T023 [P] Remove unused multi-category datasets from `datasets/` that reference deleted reference sets (commit-types.json, intent.json, sentiment.json, topic-routing.json)
- [ ] T024 Run `just check` (clippy + fmt + test + build) — fix any issues
- [ ] T025 [MANUAL] Run `csn benchmark compare-strategies` locally to validate SC-001 (≥89% accuracy on pushback dataset). Record results
- [ ] T026 Update `docs/spec-dependency-graph.md`: mark 003 as done, update status of dependent specs

---

## Task Dependencies

<!-- Machine-readable. Generated by /speckit.tasks, updated by /speckit.iterate.apply -->
<!-- Do not edit manually unless you also update GitHub issue dependencies -->

```toml
[graph]
# Phase 1: Setup
[graph.T001]
blocked_by = []

[graph.T002]
blocked_by = ["T001"]

[graph.T003]
blocked_by = ["T001"]

# Phase 2: Foundational
[graph.T004]
blocked_by = ["T002"]

[graph.T005]
blocked_by = ["T003", "T004", "T006"]

[graph.T006]
blocked_by = ["T002"]

[graph.T007]
blocked_by = ["T002"]

[graph.T008]
blocked_by = ["T004"]

# Phase 3: US1 — Combined Pipeline
[graph.T009]
blocked_by = ["T005", "T006", "T008"]

[graph.T010]
blocked_by = ["T009"]

[graph.T011]
blocked_by = ["T004", "T006"]

[graph.T012]
blocked_by = ["T009"]

# Phase 4: US2 — Startup Training
[graph.T013]
blocked_by = ["T003", "T005", "T007"]

[graph.T014]
blocked_by = ["T008", "T013"]

[graph.T015]
blocked_by = ["T010", "T014"]

[graph.T016]
blocked_by = ["T013"]

# Phase 5: US3 — Cache
[graph.T017]
blocked_by = ["T015"]

[graph.T018]
blocked_by = ["T007"]

# Phase 6: US4 — Benchmark
[graph.T019]
blocked_by = ["T005", "T009"]

[graph.T020]
blocked_by = ["T019"]

# Phase 7: US5 — Hot-Reload
[graph.T021]
blocked_by = ["T013", "T014"]

[graph.T022]
blocked_by = ["T021"]

# Phase 8: Polish
[graph.T023]
blocked_by = []

[graph.T024]
blocked_by = ["T010", "T013", "T019", "T021"]

[graph.T025]
blocked_by = ["T020"]

[graph.T026]
blocked_by = ["T024"]
```
