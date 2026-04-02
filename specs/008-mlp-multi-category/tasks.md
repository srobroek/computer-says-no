# Tasks: MLP Multi-Category Classification

**Input**: Design documents from `/specs/008-mlp-multi-category/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup

**Purpose**: No new project setup needed — extending existing codebase. This phase covers foundational types and functions that all user stories depend on.

---

## Phase 2: Foundational (Multi-Category MLP Core)

**Purpose**: Core multi-category MLP types, training, and inference that MUST be complete before any user story can be implemented.

- [ ] T001 [P] Add `MultiCatMlpClassifier<B: Backend>` struct and `MultiCatMlpConfig` to `src/mlp.rs`. Three-layer perceptron identical to `MlpClassifier` but output layer is `LinearConfig::new(hidden2, num_classes)`. Forward pass returns **raw logits** (no activation) — softmax is applied only at inference time in `classify_with_multi_mlp`, not in the model's forward method. This is required because `CrossEntropyLoss` (logits=true) applies `log_softmax` internally during training. Add `num_classes: usize` field to config. Derive `Module, Debug`.

- [ ] T002 [P] Add `compute_multi_cosine_features` function to `src/mlp.rs`. Takes text embedding + `Vec<(String, Vec<Embedding>)>` (alphabetically sorted category embeddings). Returns `Vec<f32>` of length N*3. Per category: `[max_similarity, mean_similarity, margin_vs_next_best]`. Reuse `cosine_similarity` from `crate::model`.

- [ ] T003 [P] Add `multi_content_hash` function to `src/mlp.rs`. Takes sorted category names and sorted phrases per category. Returns blake3 hex string. Format: `blake3(["v3-multicat", cat1_name, cat1_phrase1, ..., cat2_name, ...].join("\n"))`. Categories and phrases within each category are sorted alphabetically.

- [ ] T004 [P] Add `TrainedMultiCatModel` struct to `src/mlp.rs`. Fields: `reference_set_name: String`, `content_hash: String`, `classifier: MultiCatMlpClassifier<NdArray<f32>>`, `category_embeddings: Vec<(String, Vec<Embedding>)>`, `category_phrases: Vec<(String, Vec<String>)>`, `category_names: Vec<String>`. All category data sorted alphabetically.

- [ ] T005 Add `train_multi_mlp` function to `src/mlp.rs`. Analogous to `train_mlp` but for multi-category. Takes per-category embeddings and phrases (sorted alphabetically), plus hyperparameters. Builds training data: for each sample, compute per-category cosine features + char n-gram features + embedding. Labels are category indices (0..N-1, alphabetical). Uses `CrossEntropyLossConfig::new().init(&device)` with raw logits (no softmax during training — CE applies log_softmax internally). Adam optimizer, early stopping. Returns `MultiCatMlpClassifier<NdArray<f32>>` via `model.valid()`.

- [ ] T006 [P] Add `save_multi_weights` and `load_multi_weights` functions to `src/mlp.rs`. Identical to `save_weights`/`load_weights` but for `MultiCatMlpClassifier`. Use `NamedMpkFileRecorder`.

- [ ] T007 Extend `train_models_at_startup` in `src/mlp.rs` to also train multi-category models. Add a second loop over reference sets for `ReferenceSetKind::MultiCategory`. Check minimum: 2 phrases per category AND 4 total — skip if not met. Handle `Err` from `train_multi_mlp` with same fallback logic as binary: when `fallback=true`, warn and skip; when `false`, return error (FR-005). Return type changes to `(Vec<TrainedModel>, Vec<TrainedMultiCatModel>)`. Cache path uses `multi_content_hash`.

- [ ] T008 Add unit tests for multi-category MLP in `src/mlp.rs`: `multi_config_init`, `multi_forward_output_shape` (batch × N), `multi_forward_logits_range` (raw logits — no softmax constraint, values can be any f32), `compute_multi_cosine_features_known`, `multi_content_hash_deterministic`, `multi_content_hash_no_collision_with_binary`, `save_load_multi_roundtrip`, `multi_forward_tie_breaks_alphabetically` (when two categories have equal logits, alphabetically-first wins after softmax).

**Checkpoint**: Multi-category MLP core is ready — types, training, caching, and unit tests.

---

## Phase 3: User Story 1 — Multi-Category Classification via CLI (Priority: P1) 🎯 MVP

**Goal**: `csn classify` against a multi-category set returns per-category MLP scores.

**Independent Test**: Run `csn classify "you broke my code" corrections --json` and verify output has `category`, `confidence`, `all_scores` with MLP-derived scores.

### Implementation for User Story 1

- [ ] T009 [US1] Add `classify_with_multi_mlp` function to `src/classifier.rs`. Takes text embedding, text string, and `TrainedMultiCatModel`. Computes per-category cosine features via `compute_multi_cosine_features`, char n-gram features, concatenates with embedding. Runs MLP forward pass. Maps softmax output to `MultiCategoryResult` using alphabetically-ordered category names. Top phrase per category via `best_match` against category embeddings.

- [ ] T010 [US1] Extend `classify_with_text` in `src/classifier.rs` to accept `Option<&TrainedMultiCatModel>` parameter. In the `MultiCategory` branch, if a trained multi-cat model is provided, call `classify_with_multi_mlp` instead of cosine-only scoring.

- [ ] T011 [US1] Update `classify_text` signature in `src/classifier.rs` to pass through the multi-cat model. Update all callers: `cmd_classify` in `src/main.rs`, `handle_classify` in `src/mcp.rs`, daemon handler in `src/daemon.rs`.

- [ ] T012 [US1] Update `cmd_classify` in `src/main.rs` to pass `trained_multi_models` to `classify_text`. Update `train_models_at_startup` call to destructure the new return tuple `(binary_models, multi_models)`. Look up multi-cat model by set name.

- [ ] T013 [US1] Update `McpHandler` in `src/mcp.rs`: add `trained_multi_models: Mutex<Vec<TrainedMultiCatModel>>` field. Update `new()` constructor and `handle_classify` to look up and pass multi-cat model. Update `cmd_mcp` in `src/main.rs` to pass multi models to `McpHandler::new`.

- [ ] T014 [US1] Update daemon in `src/daemon.rs` to hold `Vec<TrainedMultiCatModel>` alongside binary models. Pass multi-cat model in classify handler. Update daemon startup in `src/main.rs` `Command::Daemon` to destructure and pass multi models.

- [ ] T015 [US1] Add unit tests in `src/classifier.rs`: `classify_with_multi_mlp_returns_valid_result` (confidence in 0..1, all_scores sum ~1, category is one of the category names), `classify_with_multi_mlp_scores_match_categories`.

**Checkpoint**: Multi-category MLP classification works via CLI, MCP, and daemon. Cosine fallback still works for sets without MLP.

---

## Phase 4: User Story 2 — Corrections Reference Set Restructured (Priority: P2)

**Goal**: corrections.toml is restructured from binary to multi-category with curated categories.

**Independent Test**: Load restructured corrections.toml, verify it parses as multi-category, and classification produces differentiated scores per category.

### Implementation for User Story 2

- [ ] T016 [US2] Restructure `reference-sets/corrections.toml` from binary (`[phrases]` with positive/negative) to multi-category (`[categories.correction]`, `[categories.frustration]`, `[categories.neutral]`). Change `mode = "binary"` to `mode = "multi-category"`. Curate current positive phrases into correction (directive: wrong file, revert, undo ~400) and frustration (emotional: anger, despair, profanity ~600). Move all current negative phrases to neutral (~600). Review sarcasm phrases individually — assign to frustration or correction based on intent per FR-006 criteria.

- [ ] T017 [US2] Update `list_sets` in `src/mcp.rs` to include `categories` field with per-category phrase counts for multi-category sets. Update `cmd_sets_list` in `src/main.rs` to display category names and per-category phrase counts (FR-012).

- [ ] T018 [US2] Add tests: verify corrections.toml parses as multi-category with expected categories, verify each category has ≥2 phrases (FR-002 minimum), verify no phrase appears in multiple categories.

**Checkpoint**: corrections.toml is multi-category, MLP trains on it, classification differentiates signal types.

---

## Phase 5: User Story 3 — MCP and Hook Integration (Priority: P3)

**Goal**: Hook reads category from classify output and fires tailored prompts per signal type.

**Independent Test**: Run hook with frustrated input → frustration prompt; correction input → correction prompt; neutral → no fire.

### Implementation for User Story 3

- [ ] T019 [US3] Update `.claude/hooks/user-frustration-check.sh` to read `category` from JSON output (`.category` field). Add category-tailored `additionalContext` prompts: frustration → "acknowledge frustration, de-escalate, avoid being defensive"; correction → "acknowledge the specific mistake, confirm understanding, adjust approach". Do not fire for neutral category. Handle both multi-category (has `.category`) and binary (has `.match`) output shapes for backward compatibility with binary sets.

- [ ] T020 [US3] Add a test for hook output parsing: create a small test script or unit test that feeds sample JSON (multi-category classify output) through the hook's jq parsing logic and verifies correct category extraction and prompt selection.

**Checkpoint**: Hook fires tailored prompts per category. Full feature complete.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [ ] T021 [P] Run `just check` (clippy + fmt + test + build) and fix any warnings or errors across all changed files.
- [ ] T022 [P] [MANUAL] Run `csn classify` with various inputs against restructured corrections set and verify: accuracy meets SC-001 (≥80% macro F1), latency meets SC-002 (<15ms warm), and hook correctly identifies signal type for ≥80% of a representative sample per SC-004. Requires model download.
- [ ] T023 Verify all existing tests pass unmodified (SC-005 backward compatibility). Run `cargo test --bin csn`.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 2 (Foundational)**: No dependencies — starts immediately
- **Phase 3 (US1)**: Depends on Phase 2 completion
- **Phase 4 (US2)**: Depends on Phase 3 (needs MLP integration to test multi-cat classification)
- **Phase 5 (US3)**: Depends on Phase 4 (needs restructured corrections.toml for category output)
- **Phase 6 (Polish)**: Depends on all prior phases

### Within Phase 2

- T001 (classifier struct) → T005 (training, needs struct) → T007 (startup, needs training)
- T002 (cosine features) → T005 (training, needs features)
- T003 (content hash) → T007 (startup, needs hash)
- T004 (trained model struct) → T007 (startup, needs struct)
- T006 (save/load) → T007 (startup, needs save/load)
- T008 (tests) → after T001-T006

### Within Phase 3

- T009 (classify function) → T010 (wire into classify_with_text) → T011 (update callers)
- T011 → T012 (main.rs), T013 (mcp.rs), T014 (daemon.rs) — these three are parallel
- T015 (tests) → after T009

### Parallel Opportunities

- T001, T002, T003, T004, T006 can run in parallel (different functions, no interdependencies)
- T012, T013, T014 can run in parallel (different files)
- T021, T022 can run in parallel

---

## Implementation Strategy

### MVP First (User Story 1)

1. Complete Phase 2: Multi-category MLP core types and training
2. Complete Phase 3: Wire MLP into classify path
3. **STOP and VALIDATE**: Test with a small multi-category TOML (can use existing test in `reference_set.rs` format)
4. Continue to Phase 4 (corrections.toml restructure) and Phase 5 (hook)

### Incremental Delivery

1. Phase 2 → MLP can train on any multi-category set (general capability)
2. + Phase 3 → CLI/MCP/daemon all support multi-category MLP (MVP!)
3. + Phase 4 → corrections.toml restructured (primary use case)
4. + Phase 5 → Hook fires per-category prompts (full feature)

---

## Task Dependencies

<!-- Machine-readable. Generated by /speckit.tasks, updated by /speckit.iterate.apply -->
<!-- Do not edit manually unless you also update GitHub issue dependencies -->

```toml
[graph]
# Phase 2: Foundational
[graph.T001]
blocked_by = []

[graph.T002]
blocked_by = []

[graph.T003]
blocked_by = []

[graph.T004]
blocked_by = []

[graph.T005]
blocked_by = ["T001", "T002"]

[graph.T006]
blocked_by = []

[graph.T007]
blocked_by = ["T003", "T004", "T005", "T006"]

[graph.T008]
blocked_by = ["T001", "T002", "T003", "T004", "T005", "T006"]

# Phase 3: US1
[graph.T009]
blocked_by = ["T007"]

[graph.T010]
blocked_by = ["T009"]

[graph.T011]
blocked_by = ["T010"]

[graph.T012]
blocked_by = ["T011"]

[graph.T013]
blocked_by = ["T011"]

[graph.T014]
blocked_by = ["T011"]

[graph.T015]
blocked_by = ["T009"]

# Phase 4: US2
[graph.T016]
blocked_by = ["T012"]

[graph.T017]
blocked_by = ["T012"]

[graph.T018]
blocked_by = ["T016"]

# Phase 5: US3
[graph.T019]
blocked_by = ["T016"]

[graph.T020]
blocked_by = ["T019"]

# Phase 6: Polish
[graph.T021]
blocked_by = ["T019"]

[graph.T022]
blocked_by = ["T016"]

[graph.T023]
blocked_by = ["T021"]
```
