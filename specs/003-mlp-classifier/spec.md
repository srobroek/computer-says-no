# Feature Specification: MLP Pushback Classifier

**Feature Branch**: `003-mlp-classifier`
**Created**: 2026-04-01
**Status**: In Progress
**Project**: Computer Says No
**Project Number**: 2
**Project ID**: PVT_kwDOECmZr84BTbZa
**Input**: User description: "Binary pushback classifier using Burn framework. Train a 2-layer MLP at daemon startup from reference set phrases. Expose as mlp strategy alongside existing cosine strategies. Cache trained weights. Benchmark integration."

## Clarifications

### Session 2026-04-01

- Q: How should the user select the MLP strategy? → A: No strategy field. Combined pipeline (embeddings + cosine features → MLP) is always used when an MLP model is trained. Pure cosine is the automatic fallback when MLP is unavailable.
- Q: Should MLP replace or combine with cosine? → A: Combined always. MLP input = embedding (384-dim) + cosine features (max pos similarity, max neg similarity, margin) = 387-dim input. This is the architecture that achieved 96.2% accuracy in research.
- Q: Train MLP for all binary sets or opt-in? → A: Auto-train for all eligible binary sets. Currently only `corrections` is binary (it *is* the pushback detector). Multi-category sets are excluded by FR-008. No opt-in mechanism needed.
- Q: What happens when MLP training fails to converge? → A: Error by default — daemon refuses to start. User can override via config to fall back to pure cosine instead. Training only occurs on first run or when the reference set changes (weight cache handles this).
- Q: What does `confidence` represent with the combined pipeline? → A: MLP sigmoid output (0.0-1.0 probability). `scores.positive` and `scores.negative` still carry raw cosine similarity values for transparency.

## User Scenarios & Testing

### User Story 1 — Enhanced Classification via Combined Pipeline (Priority: P1)

A user sends text to the `/classify` endpoint. When the reference set has a trained MLP model, the system automatically uses the combined pipeline (embeddings + cosine features → MLP) for higher accuracy. The API contract and response shape are unchanged — the improvement is transparent.

**Why this priority**: Core value — the combined pipeline breaks through the ~80% cosine accuracy ceiling to 89-96% without any API changes for existing clients.

**Independent Test**: Send a POST to `/classify` with a known pushback phrase. Verify the response contains `is_match: true` with higher confidence than pure cosine would produce.

**Acceptance Scenarios**:

1. **Given** the daemon is running with a trained MLP model for a binary reference set, **When** a user sends a pushback phrase, **Then** the response contains `is_match: true` and a confidence score between 0.0 and 1.0 derived from the MLP.
2. **Given** the daemon is running with a trained MLP model, **When** a user sends a neutral/non-pushback phrase, **Then** the response contains `is_match: false`.
3. **Given** the daemon is running, **When** a user sends a request for a reference set that has no trained MLP (no negatives, too few phrases, or multi-category), **Then** the system uses pure cosine classification transparently.

---

### User Story 2 — Automatic Training at Startup (Priority: P1)

When the daemon starts, the MLP classifier trains automatically from the reference set's positive and negative phrases. The user does not need to run any manual training step.

**Why this priority**: Training must happen without user intervention. This is foundational — the MLP strategy cannot function without a trained model.

**Independent Test**: Start the daemon and verify logs show MLP training completed within the startup sequence. Then issue a `/classify` request and confirm the combined pipeline is active.

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

### User Story 4 — Benchmark Combined vs Pure Cosine (Priority: P2)

The existing benchmark harness (`csn benchmark run`) can evaluate the combined pipeline alongside pure cosine, enabling direct accuracy comparison on the same datasets.

**Why this priority**: Validates the accuracy improvement claim. Important for the project's research goals but not for runtime classification.

**Independent Test**: Run `csn benchmark compare-strategies` and verify "combined" appears as a strategy in the output table alongside threshold/margin/adaptive.

**Acceptance Scenarios**:

1. **Given** the benchmark harness includes the combined pipeline as a strategy, **When** a benchmark run executes, **Then** combined pipeline accuracy results appear alongside cosine-based strategies.
2. **Given** benchmark results for both pure cosine and combined pipeline exist, **When** `compare-strategies` runs, **Then** the comparison table shows per-dataset accuracy differences.

---

### User Story 5 — Hot-Reload Retrains MLP (Priority: P3)

When the file watcher detects a reference set change at runtime, the MLP model retrains automatically without requiring a daemon restart.

**Why this priority**: Nice-to-have for development workflow. Users can iterate on reference set phrases and see combined pipeline results without restarting.

**Independent Test**: With the daemon running, modify a reference set TOML file. Verify logs show MLP retraining triggered, then issue a `/classify` request and confirm the model reflects the updated phrases.

**Acceptance Scenarios**:

1. **Given** the daemon is running with a trained MLP, **When** the reference set file is modified, **Then** the MLP retrains within 5 seconds of the file change.
2. **Given** a retrain is in progress, **When** a `/classify` request arrives, **Then** the request uses the previous MLP model (not blocked by retraining).

---

### Edge Cases

- Reference set with too few phrases (< 4 total): MLP training skipped, pure cosine used (FR-007).
- Reference set with very similar phrases (low embedding diversity): training may converge to a trivial model. System proceeds — accuracy will be validated by benchmark.
- Training fails to converge: daemon refuses to start by default; configurable override falls back to pure cosine (FR-012).
- Multi-category reference set: MLP not applicable, pure cosine used (FR-008).

## Requirements

### Functional Requirements

- **FR-001**: System MUST automatically use the combined pipeline (embedding + cosine features → MLP) for binary reference sets that have a trained MLP model, with no API changes. The response shape (`is_match`, `confidence`, `top_phrase`, `scores`) MUST remain unchanged. When MLP is active, `confidence` MUST be the MLP sigmoid probability (0.0-1.0) and `scores.positive`/`scores.negative` MUST remain as raw cosine similarity values.
- **FR-002**: System MUST train the MLP classifier automatically at daemon startup and standalone classify invocation from reference set positive and negative phrase embeddings.
- **FR-003**: System MUST use a 2-layer architecture with combined input: embedding concatenated with cosine features (max positive similarity, max negative similarity, margin) = embedding_dim + 3 input (computed at runtime from the embedding model's output dimension) → hidden layer 1 (256 units, ReLU) → hidden layer 2 (128 units, ReLU) → output (1 unit, sigmoid).
- **FR-004**: System MUST cache trained model weights to disk, keyed by a content hash of the reference set phrases.
- **FR-005**: System MUST invalidate the weight cache when reference set content changes (phrases added, removed, or modified).
- **FR-006**: System MUST skip MLP training for reference sets that lack negative phrases, falling back to pure cosine classification with a log message.
- **FR-007**: System MUST skip MLP training when the total phrase count (positive + negative) is below a minimum viable threshold of 4 phrases.
- **FR-008**: System MUST fall back to pure cosine classification for multi-category reference sets (MLP applies only to binary sets).
- **FR-009**: System MUST integrate the combined pipeline as a strategy in the existing benchmark harness for accuracy comparison against pure cosine.
- **FR-010**: System MUST retrain the MLP when the file watcher detects reference set changes at runtime, without blocking ongoing classification requests.
- **FR-011**: System MUST use Adam optimization with L2 regularization and early stopping during training.
- **FR-012**: System MUST refuse to start if MLP training fails to converge (loss does not decrease after max epochs). A configuration option MUST allow the user to override this to fall back to pure cosine instead.

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
