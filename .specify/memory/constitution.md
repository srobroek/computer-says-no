<!--
Sync Impact Report
- Version change: 0.0.0 → 1.0.0 (initial ratification)
- Added principles: I. Single Binary, II. Dual Protocol, III. Configurable Classification, IV. Warm-First Performance, V. Simplicity
- Added sections: Technical Constraints, Development Workflow
- Templates requiring updates:
  - .specify/templates/plan-template.md ✅ no changes needed (Constitution Check section is dynamic)
  - .specify/templates/spec-template.md ✅ no changes needed (generic template)
  - .specify/templates/tasks-template.md ✅ no changes needed (generic template)
- Follow-up TODOs: none
-->

# Computer Says No Constitution

## Core Principles

### I. Single Binary Distribution

`csn` MUST be a single, statically-linked Rust binary with zero runtime dependencies.
Users MUST be able to download and run the binary without installing runtimes, interpreters,
or companion services. ONNX models are downloaded on first use and cached locally.

**Rationale**: The primary consumers are Claude Code hooks and CLI scripts. Any friction
in setup (Python venvs, Docker, sidecar processes) directly reduces adoption.

### II. Dual Protocol, One Port

The daemon MUST serve both REST and MCP/SSE on a single configurable port.
REST endpoints (`/classify`, `/embed`, `/similarity`, `/health`, `/sets`) serve hooks
and CLI clients. The `/sse` endpoint serves Claude Code agents via MCP.

- MUST bind `127.0.0.1` only — never expose to network
- MUST NOT require separate MCP client or wrapper process
- CLI commands MUST act as thin HTTP clients to the daemon (with `--standalone` fallback)

**Rationale**: Two protocols exist because two consumers exist (hooks call REST, agents
call MCP). Merging them onto one port eliminates coordination and port conflicts.

### III. Configurable Classification via Reference Sets

All classification behavior MUST be defined by TOML reference sets, not hardcoded logic.
The binary ships default sets (corrections, commit-types) but users MUST be able to add,
modify, or remove sets without rebuilding.

- Binary mode: positive + negative phrases, threshold
- Multi-category mode: named categories with phrases, best-match wins
- Reference sets MUST be hot-reloaded on file change (no daemon restart)
- Remote sets (URL source) MUST be periodically refreshed

**Rationale**: The tool is a general-purpose embedding classifier. Correction detection
is the first use case, not the only one. Hardcoding classification logic would limit
adoption to a single workflow.

### IV. Warm-First Performance

Classification latency MUST be under 50ms p95 when the daemon is running (warm path).
Cold-start CLI is acceptable at 1-2s but MUST NOT be the primary path.

- Model loading happens once at daemon startup
- Reference set embeddings are precomputed and cached (blake3 content hash)
- File watcher re-embeds changed sets in background (~150ms for 25 phrases)
- Lazy daemon start: if CLI detects daemon is down, start it in background, skip this call

**Rationale**: Hooks run on every prompt submission. Latency above 50ms is perceptible
and degrades the interactive coding experience.

### V. Simplicity

Prefer fewer abstractions, fewer features, and fewer dependencies.

- No feature flags or plugin systems — ship what works
- No backwards-compatibility shims — breaking changes get a major version bump
- No speculative abstractions — three similar lines of code beat a premature helper
- Integration tests over mocks — the ONNX runtime is the system under test

**Rationale**: This is a focused tool, not a framework. Complexity erodes the
single-binary advantage and makes the codebase harder to contribute to.

## Technical Constraints

- **Language**: Rust (2024 edition)
- **Embedding runtime**: fastembed-rs (ONNX via ort)
- **HTTP server**: axum 0.8
- **CLI**: clap 4 (derive)
- **Config**: TOML (`~/.config/computer-says-no/`), env vars (`CSN_*`), CLI flags
- **Cache**: `~/.cache/computer-says-no/{model-name}/` with blake3 content hashing
- **Service management**: service-manager crate (launchd/systemd/Task Scheduler)
- **File watching**: notify 7
- **License**: Apache-2.0

## Development Workflow

- Conventional commits enforced via cocogitto (`cog.toml`)
- Pre-commit hooks: typos, gitleaks, trailing whitespace, end-of-file
- `just check` (clippy + fmt + test + build) MUST pass before merge
- Benchmarks (`csn benchmark`) MUST be run when changing models, reference sets,
  or classification logic — regressions in accuracy or latency block merge
- Feature branches merge to main via `--no-ff` to preserve history

## Governance

This constitution governs all specification and implementation decisions for `csn`.
Amendments require:

1. A speckit iterate cycle documenting the change and rationale
2. Version bump following semver (MAJOR: principle removal/redefinition,
   MINOR: new principle or material expansion, PATCH: clarification/wording)
3. Update to this file and propagation to dependent templates

All specs and plans MUST include a Constitution Check verifying compliance with
these principles. Violations MUST be justified in the Complexity Tracking section.

**Version**: 1.0.0 | **Ratified**: 2026-03-31 | **Last Amended**: 2026-03-31
