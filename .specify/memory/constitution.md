<!--
Sync Impact Report
- Version change: 1.0.1 → 2.0.0 (MAJOR — architectural redesign)
- Modified principles:
  - II. "Dual Protocol, One Port" → "MCP over Stdio"
  - III. "Configurable Classification via Reference Sets" — removed hot-reload MUST
  - IV. "Warm-First Performance" → "Fast-Start Performance"
- Added sections: none
- Removed sections: none (content updated in place)
- Technical Constraints changes: removed axum 0.8, notify 7, service-manager; added rust-mcp-sdk
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

### II. MCP over Stdio

`csn mcp` MUST serve MCP tools over stdio transport (JSON-RPC via stdin/stdout).
MCP clients (Claude Code, Cursor) spawn `csn mcp` as a subprocess and manage
its lifecycle. No daemon, no ports, no sockets.

- MUST NOT require a separate running daemon or server process
- MUST NOT expose network ports — stdio is local-only by nature
- CLI commands (`classify`, `embed`, `similarity`) MUST run in-process without
  network calls or daemon dependency
- MCP process loads model and reference sets once at startup, serves tools
  for the session duration

**Rationale**: The ecosystem convention is stdio MCP (uvx/npx pattern). Daemon-based
architectures require manual lifecycle management, port configuration, and process
monitoring — unnecessary complexity for a local classification tool.

### III. Configurable Classification via Reference Sets

All classification behavior MUST be defined by TOML reference sets, not hardcoded logic.
The binary ships default sets (corrections) but users MUST be able to add,
modify, or remove sets without rebuilding.

- Binary mode: positive + negative phrases, threshold
- Multi-category mode: named categories with phrases, best-match wins
- Reference set changes take effect on next `csn mcp` process start or next
  CLI invocation — no hot-reload required

**Rationale**: The tool is a general-purpose embedding classifier. Correction detection
is the first use case, not the only one. Hardcoding classification logic would limit
adoption to a single workflow.

### IV. Fast-Start Performance

`csn` MUST start and be ready for tool calls within 5 seconds when models and MLP
weights are cached. Classification latency MUST be under 50ms per tool call once
the process is running.

- Model loading happens once at process startup
- Reference set embeddings are precomputed and cached (blake3 content hash)
- MLP weights are cached to disk, loaded on startup if hash matches
- First-ever startup may take longer (model download + MLP training)

**Rationale**: MCP clients spawn the server on first tool call. Startup time directly
affects perceived responsiveness. Once running, tool calls must be fast enough
for interactive coding workflows.

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
- **MCP server**: rust-mcp-sdk (stdio transport)
- **CLI**: clap 4 (derive)
- **ML framework**: burn 0.20 (ndarray backend)
- **Config**: TOML (`~/.config/computer-says-no/`), env vars (`CSN_*`), CLI flags
- **Cache**: `~/.cache/computer-says-no/{model-name}/` with blake3 content hashing
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

**Version**: 2.0.0 | **Ratified**: 2026-03-31 | **Last Amended**: 2026-04-02
