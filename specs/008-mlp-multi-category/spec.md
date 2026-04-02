# Feature Specification: MLP Multi-Category Classification

**Feature Branch**: `008-mlp-multi-category`  
**Created**: 2026-04-02  
**Status**: Draft  
**Input**: User description: "Extend MLP classifier from binary (sigmoid) to multi-category (softmax). Restructure corrections.toml to multi-category. Update CLI/MCP output for per-category results. Hook reads category from classify output."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Multi-Category Classification via CLI (Priority: P1)

A developer uses `csn classify` against a multi-category reference set and receives per-category confidence scores from the MLP, enabling them to distinguish between correction, frustration, sarcasm, and neutral inputs in a single classification call.

**Why this priority**: This is the core capability — without MLP-backed multi-category classification, the feature has no value. The existing cosine-only multi-category path lacks the learned decision boundaries that MLP provides.

**Independent Test**: Can be fully tested by running `csn classify "you broke my code again" corrections` and verifying the output includes per-category MLP scores with a winning category.

**Acceptance Scenarios**:

1. **Given** a multi-category reference set with 3+ categories, **When** the user classifies text via CLI, **Then** the output shows the winning category, its confidence, per-category scores, and the top matching phrase per category.
2. **Given** a multi-category reference set with a trained MLP, **When** the user classifies text, **Then** the MLP softmax output is used instead of cosine-only scoring.
3. **Given** a multi-category reference set with fewer than 4 total phrases, **When** the system starts up, **Then** MLP training is skipped and cosine-only classification is used as fallback.

---

### User Story 2 - Corrections Reference Set Restructured (Priority: P2)

The corrections.toml reference set is restructured from binary (positive/negative) to multi-category (correction, frustration, sarcasm, neutral), enabling the system to distinguish the *type* of signal rather than just "is this pushback?".

**Why this priority**: The restructured reference set is the primary use case that motivates multi-category MLP. Without it, the feature is technically complete but lacks practical value.

**Independent Test**: Can be tested by loading the restructured corrections.toml and verifying it parses as multi-category with the expected categories, and classification produces differentiated scores per category.

**Acceptance Scenarios**:

1. **Given** the restructured corrections.toml with categories (correction, frustration, sarcasm, neutral), **When** the system loads it, **Then** it is recognized as a multi-category reference set with all categories populated.
2. **Given** a clearly frustrated input like "for the love of god stop", **When** classified against the restructured set, **Then** the frustration category scores highest.
3. **Given** a neutral instruction like "add error handling to the parse function", **When** classified, **Then** the neutral category scores highest.

---

### User Story 3 - MCP and Hook Integration (Priority: P3)

The MCP `classify` tool returns per-category results for multi-category sets, and the frustration detection hook reads the category directly from the classify output instead of running a separate check.

**Why this priority**: Downstream integration ensures the multi-category capability is usable by MCP clients and the existing hook system. Without this, the new capability is CLI-only.

**Independent Test**: Can be tested by calling the MCP `classify` tool with a multi-category set and verifying the JSON response contains per-category scores, and by verifying the hook script parses the new output format.

**Acceptance Scenarios**:

1. **Given** a multi-category reference set, **When** the MCP `classify` tool is called, **Then** the response JSON includes `category`, `confidence`, and `all_scores` with per-category breakdowns.
2. **Given** the frustration hook is configured, **When** a user submits a frustrated message, **Then** the hook reads the category and fires a frustration-tailored prompt (empathize, de-escalate).
3. **Given** the hook is configured, **When** a user submits a correction like "no, wrong file", **Then** the hook fires a correction-tailored prompt (acknowledge mistake, adjust approach).
4. **Given** the hook is configured, **When** a user submits a neutral instruction, **Then** the hook does NOT fire.

---

### Edge Cases

- What happens when a multi-category set has only 2 categories? The system trains a multi-category MLP with 2 softmax outputs (not a binary MLP). The mode is determined by the TOML `mode = "multi-category"` field, not the category count.
- What happens when MLP training fails for a multi-category set? The system falls back to cosine-only multi-category classification (existing behavior).
- What happens when a reference set is converted from binary to multi-category? The binary MLP weight cache is unaffected (different hash prefix "v2-char256" vs "v3-multicat"). Old binary weights remain on disk but are no longer loaded. The multi-category MLP trains fresh on first run.
- What happens when all categories score below the threshold? The result reports `is_match: false` with the highest-scoring category still identified.
- What happens when existing binary reference sets are loaded? They continue to use the binary MLP path unchanged. No shared code paths are modified in a way that changes binary behavior.
- What happens when two categories tie for highest softmax score? The alphabetically-first category wins (deterministic, consistent with the alphabetical ordering used for input features and output indices).
- What happens when a category has zero phrases after loading? The reference set loader rejects it (existing validation in `load_reference_set` requires at least one phrase per category). This is an error, not a silent skip.
- What happens when the daemon receives a classify result with the new multi-category MLP shape? The daemon already forwards `ClassifyResult` as-is via JSON lines. The `MultiCategoryResult` variant is already defined and serializable. No daemon protocol changes needed.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST support MLP classification for multi-category reference sets, producing per-category confidence scores via softmax activation.
- **FR-002**: System MUST train a multi-category MLP with softmax output layer (N units, one per category) and cross-entropy loss when a multi-category reference set has sufficient training data. Minimum: at least 2 phrases per category AND at least 4 total phrases across all categories. Categories with fewer than 2 phrases MUST cause training to be skipped (fallback to cosine). Training hyperparameters (learning rate, weight decay, max epochs, patience) reuse the same defaults as the binary MLP path.
- **FR-003**: System MUST preserve the existing binary MLP path (sigmoid, BCE loss, 1 output unit) for binary reference sets — multi-category support is additive. Shared code paths (`train_models_at_startup`, `classify_with_text`) are extended with a branch for multi-category, not modified in-place. Binary behavior MUST remain byte-identical for existing reference sets.
- **FR-004**: System MUST cache trained multi-category MLP weights using a content hash that includes a version prefix (e.g., "v3-multicat"), sorted category names, and sorted phrases per category. Automatic invalidation when the reference set changes. Binary MLP weights (using "v2-char256" prefix) are unaffected — the different prefix ensures no collision between binary and multi-category caches for the same reference set name.
- **FR-005**: System MUST fall back to cosine-only multi-category classification when MLP training fails or is skipped (fewer than 4 phrases).
- **FR-006**: The corrections.toml reference set MUST be restructured from binary to multi-category. Guaranteed categories: correction, frustration, neutral. All current negative phrases (praise, instructions, questions, confirmations) become neutral. Sarcasm is NOT a guaranteed standalone category — during phrase curation, sarcasm phrases MUST be reviewed individually and assigned to either frustration or correction based on intent. If a meaningful cluster remains that fits neither, sarcasm MAY be retained as a fourth category. Curation criteria: frustration = emotional/exasperated (anger, despair, profanity-as-emotion); correction = directive/instructional (wrong file, revert, undo); sarcasm = evaluate per-phrase whether the underlying intent is frustrated or corrective and assign accordingly. The curation is performed during task implementation and reviewed in the PR.
- **FR-007**: CLI `classify` output for multi-category sets MUST show the winning category, its confidence score, and per-category score breakdown.
- **FR-008**: MCP `classify` tool MUST return `MultiCategoryResult` (with `category`, `confidence`, `all_scores`) when classifying against a multi-category set with a trained MLP. The existing `MultiCategoryResult` struct in `classifier.rs` is reused — no new result type is needed. The MLP replaces the cosine-only scoring within the same result shape.
- **FR-009**: The frustration detection hook MUST read the category from the classify output. The hook MUST fire separately for frustration and correction with category-tailored prompts. Frustration prompt guidance: acknowledge the user's frustration, de-escalate, avoid being defensive. Correction prompt guidance: acknowledge the specific mistake, confirm understanding, adjust approach. The hook MUST NOT fire for the neutral category. If sarcasm is retained as a category, sarcasm triggers the same hook path as whichever parent category (frustration or correction) it most closely aligns with.
- **FR-010**: Character n-gram features (256-dim) MUST be included in the multi-category MLP input, consistent with the binary MLP feature set.
- **FR-011**: Multi-category MLP input dimension MUST be: embedding dimension + (N_categories * 3 cosine features) + 256 char n-gram features. Per-category cosine features: for each category, compute [max_similarity_to_category, mean_similarity_to_category, similarity_margin_vs_next_best_category]. This yields 3 features per category. Categories MUST be sorted alphabetically for deterministic ordering of input features and softmax output indices. This ordering is stable across runs and weight cache loads.
- **FR-012**: The `list_sets` MCP tool and CLI MUST report the category names and per-category phrase counts for multi-category reference sets.
- **FR-013**: Any multi-category reference set (not just corrections.toml) MUST be eligible for MLP training if it meets the minimum phrase requirements in FR-002. The multi-category MLP is a general capability, not corrections-specific.

### Key Entities

- **MultiCategoryMlpClassifier**: An MLP with softmax output layer producing N-dimensional probability vectors, one probability per category. Categories are ordered alphabetically.
- **TrainedMultiCategoryModel**: A trained multi-category MLP with per-category reference embeddings, alphabetically-ordered category name mapping, and weight cache metadata. Analogous to the existing `TrainedModel` for binary sets.
- **CategoryScore**: Per-category classification result including category name, confidence score, and top matching phrase. Already exists in `classifier.rs` — reused as-is.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Multi-category MLP classification achieves at least 80% macro-averaged F1 score on the restructured corrections set (measured via leave-one-out cross-validation). Macro-averaged F1 accounts for class imbalance (neutral will have far more phrases than correction/frustration).
- **SC-002**: Multi-category MLP classification latency is within 2x of binary MLP latency for the same text, and under 15ms absolute for a warm classify call (binary baseline is ~5ms warm).
- **SC-003**: All existing binary reference sets continue to work identically — no regression in binary MLP accuracy or behavior.
- **SC-004**: The frustration hook correctly identifies the signal type (correction vs frustration) for at least 80% of a representative sample of curated phrases when tested against their assigned category post-curation.
- **SC-005**: All existing unit tests pass without modification (backward compatibility).

## Clarifications

### Session 2026-04-02

- Q: Should the ~600 current negative phrases be split into sub-categories (praise, instruction, question) or lumped as neutral? → A: All current negatives become a single "neutral" category. 4 categories max: correction, frustration, sarcasm (if warranted), neutral.
- Q: How should the hook behave per category? → A: Frustration and correction fire separate category-tailored prompts. Sarcasm is not a standalone hook trigger — sarcasm phrases are reviewed during curation and assigned to frustration or correction. Hook does not fire for neutral.

## Assumptions

- The existing multi-category data structures in `reference_set.rs` (`MultiCategoryEmbeddings`, `CategoryEmbeddings`) are reused — no structural changes to the reference set loading code. These structures already provide per-category embeddings and phrases, which is exactly what the multi-category MLP needs for per-category cosine feature computation and category-to-index mapping.
- The phrase redistribution from binary (positive/negative) to multi-category (correction/frustration/neutral) is a curation step performed during implementation, reviewed in the PR.
- The daemon protocol (JSON lines over unix socket) does not need changes — it already forwards `ClassifyResult` (which includes the `MultiCategoryResult` variant) as serialized JSON. Validated: the daemon serializes `ClassifyResult` via serde, and `MultiCategoryResult` is already a variant.
- The benchmark harness (spec 002) is not updated for multi-category in this spec. If multi-category benchmark support is needed, it will be tracked as a separate issue (not a spec — it's an enhancement to existing infrastructure).
- The existing `MultiCategoryResult` and `CategoryScore` structs in `classifier.rs` are reused for MLP output. The MLP replaces the cosine-only scoring within the existing multi-category classification path, producing the same result shape with MLP-derived confidence scores.
