# Feature Specification: Core Binary with CLI and REST Daemon

**Feature Branch**: `001-core-binary-cli`
**Created**: 2026-03-31
**Status**: Draft
**Input**: User description: "Core binary with CLI and REST daemon — single Rust binary (csn) that loads ONNX embedding models via fastembed-rs, classifies text against configurable TOML reference sets (binary and multi-category modes), and exposes functionality via both CLI and REST API. Daemon serves on localhost with warm classification latency under 50ms. Config via TOML file, env vars, CLI flags with standard precedence. This is a backfill spec — prototype code already exists in src/."

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Classify Text via CLI (Priority: P1)

A developer writes a hook script that classifies user prompts to detect corrections. They run a single command, passing the text and reference set name, and receive a match/no-match result with confidence score. The command works whether the daemon is running or not.

**Why this priority**: Classification is the core value proposition. Without it, no downstream integration (hooks, agents) is possible.

**Independent Test**: Run `csn classify "no, use X instead" --set corrections` and verify the output contains a match verdict and confidence score.

**Acceptance Scenarios**:

1. **Given** the daemon is running and a reference set named "corrections" is loaded, **When** the user runs `csn classify "no, that's wrong" --set corrections`, **Then** the system returns a match result with confidence above the set threshold.
2. **Given** the daemon is not running, **When** the user runs `csn classify "hello" --set corrections`, **Then** the system exits with a non-zero exit code, logs a warning that the daemon is not reachable, and suggests starting it with `csn serve` or using `--standalone`.
5. **Given** the daemon is not running, **When** the user runs `csn classify "hello" --set corrections --standalone`, **Then** the system loads the model in-process, classifies the text, and returns a result (slower cold start, no server required).
3. **Given** the user specifies `--json`, **When** classification completes, **Then** the output is valid JSON containing `match`, `confidence`, `top_phrase`, and `scores` fields.
4. **Given** the user requests a non-existent reference set, **When** the command runs, **Then** the system returns an error listing available sets.

---

### User Story 2 — Serve REST API for Hooks (Priority: P1)

A hook script sends an HTTP POST to the daemon's `/classify` endpoint with a JSON body. The daemon returns a JSON classification result within 50ms. Other endpoints (`/embed`, `/similarity`, `/health`, `/sets`) provide supporting functionality.

**Why this priority**: Hooks run on every prompt submission. The REST API is the primary integration surface for automated classification.

**Independent Test**: Start the daemon with `csn serve`, POST to `http://localhost:9847/classify` with a JSON body, and verify the response contains classification results.

**Acceptance Scenarios**:

1. **Given** the daemon is running with reference sets loaded, **When** a client POSTs `{"text": "fix the bug", "reference_set": "commit-types"}` to `/classify`, **Then** the response contains a category, confidence, and top matching phrase.
2. **Given** the daemon is running, **When** a client GETs `/health`, **Then** the response contains status, model name, loaded set count, and uptime.
3. **Given** the daemon is running, **When** a client GETs `/sets`, **Then** the response lists all loaded reference sets with name, mode, and phrase count.
4. **Given** the daemon is running, **When** a client POSTs `{"text": "hello world"}` to `/embed`, **Then** the response contains the embedding vector and its dimensions.
5. **Given** the daemon is running, **When** a client POSTs `{"a": "fix bug", "b": "resolve issue"}` to `/similarity`, **Then** the response contains a similarity score between 0 and 1.

---

### User Story 3 — Manage Reference Sets (Priority: P2)

A developer creates a new TOML reference set file to classify prompts into custom categories (e.g., "rule-topics" for routing prompts to relevant CLAUDE.md rules). They place the file in the reference sets directory, and the running daemon automatically detects the change and loads the new set within seconds — no restart needed.

**Why this priority**: Extensibility through custom reference sets is what makes this a general-purpose tool rather than a single-purpose correction detector.

**Independent Test**: Create a new `.toml` file in the reference sets directory with valid metadata and phrases, then run `csn sets list` and verify the new set appears.

**Acceptance Scenarios**:

1. **Given** a valid binary-mode TOML file is placed in the reference sets directory, **When** the user runs `csn sets list`, **Then** the new set appears with its name, mode, and phrase count.
2. **Given** a valid multi-category TOML file exists, **When** the user classifies text against it, **Then** the result includes the best-matching category and scores for all categories.
3. **Given** a TOML file with invalid structure is placed in the directory, **When** the system loads sets, **Then** the invalid file is skipped with a warning and other sets load normally.

---

### User Story 4 — Compute Embeddings and Similarity (Priority: P3)

A developer uses the CLI to generate embedding vectors or compute similarity scores between two texts. This supports debugging reference sets and building ad-hoc similarity checks.

**Why this priority**: Supporting functionality for reference set development and debugging. Not required for core classification workflow.

**Independent Test**: Run `csn embed "hello world"` and verify it returns a vector; run `csn similarity "fix bug" "resolve issue"` and verify it returns a score.

**Acceptance Scenarios**:

1. **Given** a model is available, **When** the user runs `csn embed "some text"`, **Then** the output contains the embedding vector, its dimensions, and the model name.
2. **Given** a model is available, **When** the user runs `csn similarity "text a" "text b"`, **Then** the output is a decimal similarity score between -1 and 1.
3. **Given** the user runs `csn models`, **Then** the output lists all supported models with their names and embedding dimensions.

---

### Edge Cases

- What happens when the configured model has not been downloaded yet? The system MUST download it on first use and cache it locally.
- What happens when the daemon port is already in use? The system MUST report the conflict clearly and suggest an alternative port.
- What happens when a reference set has zero phrases? The system MUST reject it during loading with a descriptive error.
- What happens when the embedding model produces vectors of different dimensions than cached embeddings? The system MUST detect the mismatch and re-embed the reference set.
- What happens when the config file is missing? The system MUST fall back to built-in defaults.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST load ONNX embedding models via fastembed-rs and cache them locally for reuse across sessions.
- **FR-002**: System MUST parse TOML reference sets in two modes: binary (positive/negative phrases with threshold) and multi-category (named categories with phrases, best-match wins).
- **FR-003**: System MUST classify input text by computing cosine similarity between the input embedding and all reference set phrase embeddings.
- **FR-004**: System MUST expose classification via CLI (`csn classify`) accepting text, reference set name, model selection, and output format (human-readable or JSON).
- **FR-005**: System MUST expose classification via REST endpoint (`POST /classify`) accepting JSON with text and reference set name, returning JSON with match result, confidence, and top phrase.
- **FR-006**: System MUST expose embedding generation via CLI (`csn embed`) and REST (`POST /embed`).
- **FR-007**: System MUST expose pairwise similarity via CLI (`csn similarity`) and REST (`POST /similarity`).
- **FR-008**: System MUST expose health information via REST (`GET /health`) including status, model name, set count, and uptime.
- **FR-009**: System MUST expose loaded reference set metadata via CLI (`csn sets list`) and REST (`GET /sets`).
- **FR-010**: System MUST list all supported embedding models via CLI (`csn models`) with name and dimensions.
- **FR-011**: System MUST run a daemon (`csn serve`) binding to `127.0.0.1` on a configurable port (default 9847).
- **FR-012**: System MUST resolve configuration with precedence: CLI flags > environment variables (`CSN_*`) > config file (`~/.config/computer-says-no/config.toml`) > built-in defaults.
- **FR-013**: System MUST support model selection via `--model` flag or config, defaulting to a quantized model suitable for fast inference on commodity hardware.
- **FR-014**: System MUST hash reference set content and cache precomputed embeddings, re-embedding only when content changes.
- **FR-015**: System MUST ship default reference sets (corrections, commit-types) that are usable without user configuration.
- **FR-016**: System MUST watch the reference sets directory for file changes and automatically re-embed and reload sets without daemon restart. Changed or new sets MUST be available for classification within 1 second of the file change.

### Key Entities

- **Embedding Model**: A named ONNX model with specific embedding dimensions. Loaded once at startup, used for all embedding operations.
- **Reference Set**: A TOML file defining a classification target. Contains metadata (name, mode, threshold) and phrases organized by mode (binary: positive/negative; multi-category: named groups).
- **Classification Result**: The output of comparing input text against a reference set. For binary: match/no-match with confidence and scores. For multi-category: best category with confidence and ranked scores.
- **Configuration**: Merged settings from file, environment, and CLI flags governing port, model, log level, and directory paths.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can classify text against any loaded reference set in a single command or HTTP request and receive a result within 50ms when the daemon is warm.
- **SC-002**: Users can add a new classification use case by creating a single TOML file — no code changes, no restarts required.
- **SC-003**: The system correctly distinguishes corrections from non-corrections with at least 85% accuracy on a labeled test set of 50+ prompts.
- **SC-004**: The system correctly identifies conventional commit types (feat, fix, refactor, docs, chore, test) with at least 80% accuracy on a labeled test set.
- **SC-005**: The daemon starts and is ready to serve requests within 3 seconds on commodity hardware.
- **SC-006**: The system runs as a single binary with no runtime dependencies — download and execute is the complete installation.

## Clarifications

### Session 2026-03-31

- Q: What should the CLI do when the daemon is not running? → A: Fail with non-zero exit code and a warning suggesting `csn serve`. No standalone mode — hooks need fast responses, and cold-start model loading (1-2s) would block every prompt.
- Q: SC-002 says "no restarts required" but file watching was out of scope — contradiction? → A: Pull file watching into this spec. The daemon watches the reference sets directory and auto-reloads on change. Remote set fetching remains out of scope (separate spec).

## Assumptions

- Users run the tool on macOS (Apple Silicon) or Linux x86_64 — these are the primary targets. Windows is a future goal.
- The tool runs on the same machine as Claude Code — localhost-only binding is sufficient.
- Embedding models are downloaded from HuggingFace on first use — network access is required for initial setup but not for subsequent runs.
- Reference sets contain 5-100 phrases per category — the system is not optimized for sets with thousands of phrases.
- The daemon is long-running (installed as a service or started manually) — cold-start CLI is a fallback, not the primary usage pattern.
- MCP/SSE protocol support is out of scope for this spec — it will be covered in a separate spec.
- File watching covers local reference set changes only. Remote set fetching (URL-based auto-update) is out of scope — covered in a separate spec.
- Service management (install/uninstall as system service) is out of scope — covered in a separate spec.
