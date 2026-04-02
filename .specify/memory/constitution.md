<!--
Sync Impact Report
- Version change: 2.0.0 → 2.1.0 (MINOR — lazy daemon addition)
- Modified principles:
  - II. "MCP over Stdio" — added lazy unix socket daemon for CLI acceleration
  - IV. "Fast-Start Performance" — added warm-path targets for daemon
- Technical Constraints changes: added nix 0.29
- Templates requiring updates: none
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

### II. MCP over Stdio + Lazy Daemon for CLI

`csn mcp` MUST serve MCP tools over stdio transport (JSON-RPC via stdin/stdout).
MCP clients (Claude Code, Cursor) spawn `csn mcp` as a subprocess and manage
its lifecycle.

CLI commands (`classify`, `embed`, `similarity`) MUST transparently route through
a lazy background daemon when available, falling back to in-process when not.

- MCP: stdio only — no daemon, no ports, no sockets
- CLI: unix socket daemon auto-starts on first invocation, self-exits on idle
- MUST NOT expose network ports — unix socket is local-only
- MUST NOT require manual daemon management — start/stop is transparent
- Daemon and MCP server are independent processes with separate lifecycles

**Rationale**: MCP follows the stdio convention (uvx/npx pattern). CLI commands
need sub-30ms latency for hooks that run on every user prompt — cold-starting
the model (~254ms) on every invocation is too slow. The lazy daemon keeps the
model warm without requiring manual lifecycle management.

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

- Model loading happens once at process startup (MCP or daemon)
- Reference set embeddings are precomputed and cached (blake3 content hash)
- MLP weights are cached to disk, loaded on startup if hash matches
- First-ever startup may take longer (model download + MLP training)
- Warm-path CLI (daemon running): under 30ms end-to-end
- Cold-path CLI (no daemon): under 500ms including daemon startup
- Daemon self-exits after configurable idle timeout (default: 5 minutes)

**Rationale**: MCP clients spawn the server on first tool call. CLI hooks run on
every user prompt — warm-path latency is critical for interactive workflows.
The lazy daemon amortizes model loading across many CLI invocations.

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
- **Unix process management**: nix 0.29 (signal, process)
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

**Version**: 2.1.0 | **Ratified**: 2026-03-31 | **Last Amended**: 2026-04-02
