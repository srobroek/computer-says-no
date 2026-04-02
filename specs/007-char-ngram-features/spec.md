# Feature Specification: Character N-gram Features for Typo Robustness

**Feature Branch**: `007-char-ngram-features`
**Created**: 2026-04-02
**Status**: Draft
**Input**: User description: "Character-level features alongside embeddings for typo robustness. Add character n-gram features (trigrams/bigrams) that are robust to typos since they operate at sub-word level. Hybrid model concatenating embedding features with character n-gram features before the MLP. Addresses deferred issues #97, #98, #99 from spec 003."

## Clarifications

### Session 2026-04-02

- Q: What should the character n-gram feature hashing dimension be? → A: 256 dimensions. Keeps character features supplementary to the embedding signal (387-dim), avoids over-reliance on character patterns.

## User Scenarios & Testing

### User Story 1 — Typo-Robust Classification (Priority: P1)

A developer types a frustrated message with typos ("wwtf", "thsi is brokne") into Claude Code. The frustration detection hook classifies it correctly despite the misspellings, because the character-level features capture sub-word patterns that survive typos.

**Why this priority**: This is the core problem — misspelled input currently produces very different embeddings from the intended phrase, causing missed classifications. Character features provide a parallel signal that is inherently robust to character-level noise.

**Independent Test**: Classify "wwtf" and verify it matches with similar confidence to "wtf". Classify "thsi is brokne" and verify it matches with similar confidence to "this is broken".

**Acceptance Scenarios**:

1. **Given** a correctly spelled phrase that matches a reference set, **When** the same phrase is submitted with 1-2 character typos (swap, insertion, deletion), **Then** the classification result (match/no-match) MUST be the same, and confidence MUST be within 20 percentage points of the original.
2. **Given** a non-matching phrase, **When** submitted with typos, **Then** the classification MUST still be non-matching (no false positives from character noise).
3. **Given** the existing benchmark dataset, **When** run with the hybrid model, **Then** accuracy MUST be equal to or better than the current model on correctly-spelled inputs.

---

### User Story 2 — No Regression on Clean Input (Priority: P1)

The hybrid model must maintain or improve classification accuracy on correctly-spelled text. Adding character features must not degrade the existing MLP's performance on the standard benchmark datasets.

**Why this priority**: Equal to US1 — improving typo handling is worthless if it breaks clean-input accuracy.

**Independent Test**: Run the existing benchmark suite (`csn benchmark run`) and compare accuracy against the pre-feature baseline.

**Acceptance Scenarios**:

1. **Given** the existing benchmark datasets (corrections, etc.), **When** run with the hybrid model, **Then** accuracy MUST be >= the current model's accuracy on each dataset.
2. **Given** the existing unit tests for classification, **When** run with the hybrid model, **Then** all existing tests MUST still pass.

---

### User Story 3 — Transparent Integration (Priority: P2)

The character features are computed automatically — no user configuration, no new commands, no changes to reference set format. The existing classification pipeline, MCP tools, daemon, and CLI all benefit transparently.

**Why this priority**: The feature should be invisible to users. It's an internal model improvement, not a new capability.

**Independent Test**: Run `csn classify`, `csn mcp` (classify tool), and daemon-routed classify — all should work identically, just with better typo handling.

**Acceptance Scenarios**:

1. **Given** the reference set TOML format is unchanged, **When** a user loads existing reference sets, **Then** classification works with character features automatically.
2. **Given** the MCP classify tool, **When** called with a misspelled input, **Then** the response format is identical (same JSON fields) and accuracy is improved.
3. **Given** cached MLP weights from before this feature, **When** the system starts, **Then** it MUST detect the model architecture change and retrain (cache invalidation).

---

### Edge Cases

- What happens with very short input (1-2 characters)? → Character n-grams are sparse but still computed. The embedding signal dominates for very short text.
- What happens with non-Latin characters (emoji, CJK)? → Character n-grams work on any Unicode text. Emoji produce unique n-grams. No special handling needed.
- What happens with all-punctuation input ("!!!", "...")? → Produces character features from punctuation n-grams. Classification relies primarily on embedding signal for these.
- How does the MLP input dimension change affect cached weights? → Cache key includes a content hash. Architecture change invalidates the cache, triggering retrain on first use.
- What if character features make the MLP input too large (>1000 dimensions)? → The n-gram feature vector size is bounded by the hashing trick (fixed-size output regardless of input length).

## Requirements

### Functional Requirements

- **FR-001**: The system MUST compute character n-gram features (character-level bigrams and trigrams) from the input text and concatenate them with the existing embedding + cosine feature vector before MLP classification.
- **FR-002**: The system MUST compute the same character n-gram features from reference set phrases during MLP training, so the model learns the relationship between character patterns and classification labels.
- **FR-003**: The character n-gram feature vector MUST be a fixed size regardless of input text length (bounded representation via hashing or similar technique).
- **FR-004**: The MLP architecture MUST remain two hidden layers (256 → 128 → 1) but with an increased input dimension to accommodate the additional character features.
- **FR-005**: The system MUST invalidate cached MLP weights when the feature vector size changes (architecture change detected via cache key).
- **FR-006**: Classification accuracy on correctly-spelled benchmark inputs MUST NOT decrease compared to the pre-feature baseline.
- **FR-007**: Classification of misspelled variants (1-2 character edits) of matching phrases MUST produce the same match/no-match decision as the correctly-spelled version in at least 80% of cases.
- **FR-008**: The character feature computation MUST NOT require any external model, dictionary, or network access — it operates purely on the input string.
- **FR-009**: The reference set TOML format MUST NOT change. Character features are derived from the existing phrase text.
- **FR-010**: The MCP classify tool, CLI classify command, and daemon classify handler MUST all benefit from character features transparently — no API changes.
- **FR-011**: Classification latency MUST NOT increase by more than 5ms per call (character feature computation must be fast).

### Key Entities

- **Character N-gram Feature Vector**: Fixed-size numeric vector derived from character bigrams and trigrams of the input text. Concatenated with the embedding + cosine features to form the MLP input.
- **Hybrid MLP Input**: The combined feature vector: [embedding (384-dim) + cosine features (3-dim) + character n-gram features (256-dim)] = 643-dim, fed into the MLP.

## Success Criteria

### Measurable Outcomes

- **SC-001**: Misspelled variants of matching phrases (1-2 character edits) are correctly classified in at least 80% of cases (vs ~30% baseline without character features).
- **SC-002**: Benchmark accuracy on correctly-spelled inputs is equal to or better than the current model (no regression).
- **SC-003**: Classification latency increase is under 5ms per call (character feature computation overhead).
- **SC-004**: "wwtf" classifies as a match against the corrections set with confidence > 50% (currently fails).
- **SC-005**: No changes to the reference set format, CLI interface, MCP tool interface, or daemon protocol.

## Assumptions

- Character bigrams and trigrams provide sufficient sub-word signal for typo detection. Higher-order n-grams (4-grams, 5-grams) are unlikely to add value for short input text.
- A fixed-size feature vector (via feature hashing) is sufficient — no need for a learned character embedding model.
- The MLP hidden layer sizes (256, 128) remain adequate for the larger input dimension. If not, layer sizes may need scaling, but architecture (2 layers) stays.
- The existing benchmark datasets are sufficient for measuring regression. No new datasets are needed.
- Unicode normalization (NFC) is applied before n-gram extraction to avoid encoding-variant duplicates.
- The feature hashing dimension is 256. This keeps character features supplementary to the 387-dim embedding+cosine signal, avoiding over-reliance on character patterns.
