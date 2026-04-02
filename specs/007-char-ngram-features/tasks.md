# Tasks: Character N-gram Features

**Input**: Design documents from `/specs/007-char-ngram-features/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup

**Purpose**: Add dependency, prepare module structure

- [ ] T001 Add `unicode-normalization = "0.1"` to `Cargo.toml`. Run `cargo check`

---

## Phase 2: Foundational — Character Feature Extraction

**Purpose**: Core n-gram feature computation used by all user stories

- [ ] T002 Implement `char_ngram_features(text: &str) -> Vec<f32>` in `src/mlp.rs`. Steps: NFC-normalize, lowercase, pad with `^`/`$`, extract character bigrams + trigrams via `chars().collect::<Vec<_>>().windows(2)` and `.windows(3)`, hash each n-gram to bucket 0..255 via `std::hash::DefaultHasher` modulo 256, count occurrences, L1-normalize (divide each bucket by total count). Returns 256-dim `Vec<f32>`
- [ ] T003 Update `content_hash()` in `src/mlp.rs`: prepend `"v2-char256\n"` to the hash input before the sorted phrases. This invalidates all existing cached MLP weights, forcing retrain with the new input dimension
- [ ] T004 Unit test in `src/mlp.rs`: verify `char_ngram_features("hello")` produces a 256-dim vector, sums to ~1.0 (L1 normalized), and known bigrams (`^h`, `he`, `el`, `ll`, `lo`, `o$`) map to non-zero buckets. Also verify `char_ngram_features("a")` and `char_ngram_features("a very long sentence with many words")` both produce exactly 256-dim vectors (FR-003 length independence)
- [ ] T005 Unit test in `src/mlp.rs`: verify `char_ngram_features("wtf")` and `char_ngram_features("wwtf")` share at least 50% of their non-zero buckets (typo robustness property)

**Checkpoint**: Character feature extraction works standalone with unit tests.

---

## Phase 3: User Story 1 — Typo-Robust Classification (Priority: P1)

**Goal**: MLP uses character features alongside embeddings. Typos no longer cause misclassification.

**Independent Test**: Classify "wwtf" — should match corrections set with confidence > 50%.

- [ ] T006 [US1] Update `train_single_model()` in `src/mlp.rs`: change `feature_dim` from `embed_dim + 3` to `embed_dim + 3 + 256`. For each training sample, compute `char_ngram_features(phrase)` and append to the feature vector alongside the embedding + cosine features. Update `MlpConfig::new().with_input_dim(feature_dim)` accordingly
- [ ] T007 [US1] Update `classify_with_mlp()` in `src/classifier.rs`: compute `char_ngram_features(text)` for the input text and append to the 387-dim input vector before creating the Burn tensor. Input tensor shape changes from `(1, 387)` to `(1, 643)`
- [ ] T008 [US1] Verify `classify_text()` in `src/classifier.rs` passes the original text string to `classify_with_mlp()`. If the text parameter is missing from the call chain, add it so character features can be computed from the original text, not just the embedding
- [ ] T009 [US1] Unit test in `src/classifier.rs`: verify `classify_with_mlp` accepts 643-dim input (synthetic embedding + cosine + char features) and produces valid output (confidence in 0..1)

**Checkpoint**: `csn classify "wwtf" --set corrections --json` matches with confidence > 50%.

---

## Phase 4: User Story 2 — No Regression on Clean Input (Priority: P1)

**Goal**: Accuracy on correctly-spelled benchmark inputs is maintained or improved.

**Independent Test**: Run benchmark and compare accuracy to baseline.

- [ ] T010 [US2] Update existing MLP unit tests in `src/mlp.rs` and `src/classifier.rs` to account for 643-dim input. Fix any hardcoded 387 references in test helpers (e.g., `synthetic_embedding` size, `MlpConfig::new()` default)
- [ ] T011 [US2] [MANUAL] Run `csn benchmark run --model bge-small-en-v1.5-Q` and compare accuracy against pre-feature baseline. Verify no regression per SC-002

**Checkpoint**: All existing tests pass, benchmark accuracy maintained.

---

## Phase 5: User Story 3 — Transparent Integration (Priority: P2)

**Goal**: MCP, daemon, CLI all benefit transparently. No API changes.

**Independent Test**: Verify MCP classify tool and daemon classify both return correct results for typos.

- [ ] T012 [US3] Verify MCP handler (`src/mcp.rs`) works unchanged — it calls `classifier::classify_text()` which now includes char features. No code changes expected, just verification
- [ ] T013 [US3] Verify daemon handler (`src/daemon.rs`) works unchanged — same classification pipeline. No code changes expected, just verification
- [ ] T014 [US3] [MANUAL] Verify cached weights are invalidated on first run: delete `~/.cache/computer-says-no/mlp/` contents, run `csn classify`, confirm MLP retrains (stderr shows "Training MLP"), then subsequent runs use cache

**Checkpoint**: All access paths (CLI, MCP, daemon) produce typo-robust results.

---

## Phase 6: Polish & Cross-Cutting

**Purpose**: Cleanup, docs, verification

- [ ] T015 [P] Run `just check` (clippy + fmt + test + build). Fix any issues
- [ ] T016 [P] Update CLAUDE.md: note character n-gram features in architecture/recent changes
- [ ] T017 [P] Update `docs/spec-dependency-graph.md`: add spec 007, mark dependencies (007 → 003)
- [ ] T018 [MANUAL] Benchmark classification latency with `hyperfine`: verify < 5ms overhead vs baseline (SC-003)

---

## Task Dependencies

<!-- Machine-readable. Generated by /speckit.tasks, updated by /speckit.iterate.apply -->
<!-- Do not edit manually unless you also update GitHub issue dependencies -->

```toml
[graph]
# Phase 1: Setup
[graph.T001]
blocked_by = []

# Phase 2: Foundational
[graph.T002]
blocked_by = ["T001"]

[graph.T003]
blocked_by = []

[graph.T004]
blocked_by = ["T002"]

[graph.T005]
blocked_by = ["T002"]

# Phase 3: US1 — Typo-Robust Classification
[graph.T006]
blocked_by = ["T002", "T003"]

[graph.T007]
blocked_by = ["T006"]

[graph.T008]
blocked_by = ["T007"]

[graph.T009]
blocked_by = ["T007"]

# Phase 4: US2 — No Regression
[graph.T010]
blocked_by = ["T006"]

[graph.T011]
blocked_by = ["T010"]

# Phase 5: US3 — Transparent Integration
[graph.T012]
blocked_by = ["T007"]

[graph.T013]
blocked_by = ["T007"]

[graph.T014]
blocked_by = ["T007"]

# Phase 6: Polish
[graph.T015]
blocked_by = ["T008", "T010", "T012", "T013"]

[graph.T016]
blocked_by = ["T015"]

[graph.T017]
blocked_by = ["T015"]

[graph.T018]
blocked_by = ["T007"]
```
