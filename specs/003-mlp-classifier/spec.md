# Feature Specification: MLP Pushback Classifier

**Feature Branch**: `003-mlp-classifier`
**Created**: 2026-04-01
**Status**: Draft
**Input**: User description: "Binary pushback classifier using Burn framework. Train a 2-layer MLP at daemon startup from reference set phrases. Expose as mlp strategy alongside existing cosine strategies. Cache trained weights. Benchmark integration."

## User Scenarios & Testing

### User Story 1 — Classify with MLP Strategy (Priority: P1)

A user sends text to the `/classify` endpoint specifying the `mlp` strategy. The system runs the text through the pre-trained MLP classifier and returns a binary pushback/not-pushback result with a confidence score.

**Why this priority**: Core value — the MLP classifier is the reason this spec exists. Without it, accuracy stays at the ~80% cosine ceiling.

**Independent Test**: Send a POST to `/classify` with `strategy: "mlp"` and a known pushback phrase. Verify the response contains `is_match: true` with confidence > 0.85.

**Acceptance Scenarios**:

1. **Given** the daemon is running with a trained MLP model, **When** a user sends a pushback phrase with `strategy: "mlp"`, **Then** the response contains `is_match: true` and a confidence score between 0.0 and 1.0.
2. **Given** the daemon is running with a trained MLP model, **When** a user sends a neutral/non-pushback phrase with `strategy: "mlp"`, **Then** the response contains `is_match: false`.
3. **Given** the daemon is running, **When** a user sends a request with `strategy: "mlp"` for a reference set that has no negative phrases, **Then** the system falls back to cosine classification and indicates the fallback in the response.

---

### User Story 2 — Automatic Training at Startup (Priority: P1)

When the daemon starts, the MLP classifier trains automatically from the reference set's positive and negative phrases. The user does not need to run any manual training step.

**Why this priority**: Training must happen without user intervention. This is foundational — the MLP strategy cannot function without a trained model.

**Independent Test**: Start the daemon and verify logs show MLP training completed within the startup sequence. Then issue a `/classify` request with `strategy: "mlp"` and confirm it succeeds.

**Acceptance Scenarios**:

1. **Given** a reference set with both positive and negative phrases exists, **When** the daemon starts, **Then** the MLP model trains from those phrases and is ready for classification requests.
2. **Given** the daemon is starting, **When** MLP training completes, **Then** the total startup time increases by no more than 2 seconds compared to the baseline without MLP.
3. **Given** a reference set with only positive phrases (no negatives), **When** the daemon starts, **Then** MLP training is skipped for that set and a log message explains why.

---

### User Story 3 — Cached Weights for Fast Restart (Priority: P2)

After the first training, the system caches the trained MLP weights to disk. On subsequent restarts, if the reference set has not changed, the system loads cached weights instead of retraining — reducing startup time.

**Why this priority**: Improves restart experience but not required for correctness. The system works without caching (just trains every time).

**Independent Test**: Start the daemon (first run — trains and caches). Stop and restart. Verify the second startup skips training and loads from cache. Modify a reference set phrase, restart, and verify retraining occurs.

**Acceptance Scenarios**:

1. **Given** the MLP has been trained and weights are cached, **When** the daemon restarts with an unchanged reference set, **Then** cached weights are loaded and training is skipped.
2. **Given** cached weights exist, **When** the reference set content changes (phrases added, removed, or modified), **Then** the cache is invalidated and the MLP retrains from scratch.
3. **Given** cached weights exist but the cache file is corrupted or unreadable, **When** the daemon starts, **Then** the system logs a warning and retrains from scratch.

---

### User Story 4 — Benchmark MLP vs Cosine (Priority: P2)

The existing benchmark harness (`csn benchmark run`) can evaluate the MLP strategy alongside cosine, enabling direct accuracy comparison on the same datasets.

**Why this priority**: Validates the accuracy improvement claim. Important for the project's research goals but not for runtime classification.

**Independent Test**: Run `csn benchmark compare-strategies` and verify "mlp" appears as a strategy in the output table alongside threshold/margin/adaptive.

**Acceptance Scenarios**:

1. **Given** the benchmark harness is configured with the `mlp` strategy, **When** a benchmark run executes, **Then** MLP accuracy results appear alongside cosine-based strategies.
2. **Given** benchmark results for both cosine and MLP exist, **When** `compare-strategies` runs, **Then** the comparison table includes MLP and shows per-dataset accuracy differences.

---

### User Story 5 — Hot-Reload Retrains MLP (Priority: P3)

When the file watcher detects a reference set change at runtime, the MLP model retrains automatically without requiring a daemon restart.

**Why this priority**: Nice-to-have for development workflow. Users can iterate on reference set phrases and see MLP results without restarting.

**Independent Test**: With the daemon running, modify a reference set TOML file. Verify logs show MLP retraining triggered, then issue a `/classify` request and confirm the model reflects the updated phrases.

**Acceptance Scenarios**:

1. **Given** the daemon is running with a trained MLP, **When** the reference set file is modified, **Then** the MLP retrains within 5 seconds of the file change.
2. **Given** a retrain is in progress, **When** a `/classify` request arrives with `strategy: "mlp"`, **Then** the request uses the previous model (not blocked by retraining).

---

### Edge Cases

- What happens when a reference set has too few phrases (< 4 total) to train meaningfully?
- How does the system handle a reference set where all phrases are very similar (low embedding diversity)?
- What happens if training fails to converge (loss does not decrease)?
- What happens when MLP is requested for a multi-category reference set (not binary)?

## Requirements

### Functional Requirements

- **FR-001**: System MUST support an `mlp` classification strategy for binary reference sets, returning the same response shape as existing strategies (`is_match`, `confidence`, `top_phrase`, `scores`).
- **FR-002**: System MUST train the MLP classifier automatically at daemon startup from reference set positive and negative phrase embeddings.
- **FR-003**: System MUST use a 2-layer architecture: input (embedding dimensions) → hidden layer 1 (256 units, ReLU) → hidden layer 2 (128 units, ReLU) → output (1 unit, sigmoid).
- **FR-004**: System MUST cache trained model weights to disk, keyed by a content hash of the reference set phrases.
- **FR-005**: System MUST invalidate the weight cache when reference set content changes (phrases added, removed, or modified).
- **FR-006**: System MUST skip MLP training for reference sets that lack negative phrases, falling back to cosine classification with a log message.
- **FR-007**: System MUST skip MLP training when the total phrase count (positive + negative) is below a minimum viable threshold of 4 phrases.
- **FR-008**: System MUST return an error or fallback when `strategy: "mlp"` is requested for a multi-category reference set.
- **FR-009**: System MUST integrate the MLP strategy into the existing benchmark harness for accuracy comparison.
- **FR-010**: System MUST retrain the MLP when the file watcher detects reference set changes at runtime, without blocking ongoing classification requests.
- **FR-011**: System MUST use Adam optimization with L2 regularization and early stopping during training.

### Key Entities

- **MLP Model**: Trained neural network weights (layer matrices + bias vectors) for a specific reference set. Identified by reference set name + content hash.
- **Weight Cache**: Persisted model weights on disk, invalidated when reference set content changes. Stored alongside existing embedding cache.
- **Training Config**: Hyperparameters (learning rate, regularization strength, max epochs, early stopping patience) with sensible defaults.

## Success Criteria

### Measurable Outcomes

- **SC-001**: MLP strategy achieves ≥ 89% accuracy on the pushback dataset, measured by the benchmark harness.
- **SC-002**: MLP training completes within 2 seconds at daemon startup for reference sets up to 200 phrases.
- **SC-003**: MLP inference adds no more than 1 millisecond of latency per classification request compared to embedding time.
- **SC-004**: Cached weight loading completes within 50 milliseconds (vs full training time).
- **SC-005**: The benchmark harness can compare MLP and cosine strategies side-by-side on all existing datasets.

## Assumptions

- The embedding model (bge-small, 384 dimensions) from spec 001 is the input source. MLP input dimension matches the embedding model's output.
- Reference sets with both positive and negative phrases are the expected input. Binary classification is the primary use case.
- Training hyperparameters (learning rate, epochs, regularization) will use sensible defaults that work for the ~20-200 phrase range typical of reference sets. Tuning is out of scope.
- The Burn framework with ndarray backend (+ Accelerate on macOS) provides the ML primitives. This is a build-time dependency, not a runtime download.
- Multi-category MLP classification is out of scope for this spec. MLP applies only to binary reference sets.
- The existing embedding cache (blake3-hashed) continues to handle embedding storage. MLP weight cache is a separate concern.
