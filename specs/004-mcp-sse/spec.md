# Feature Specification: MCP Server + Architecture Simplification

**Feature Branch**: `004-mcp-sse`
**Created**: 2026-04-02
**Status**: Draft
**Input**: User description: "Expose csn as an MCP server. Drop daemon, REST, and standalone flag. CLI runs in-process, MCP via stdio for agents."

## Clarifications

### Session 2026-04-02

- Q: Same port or separate port for MCP? → A: Neither — drop the daemon entirely. MCP uses stdio transport (spawned by client), not SSE/HTTP.
- Q: Do we still need REST? → A: No. CLI runs in-process (was standalone), agents use MCP over stdio. REST and the daemon are removed.
- Q: Do we still need the --standalone flag? → A: No. With no daemon, every CLI invocation is in-process. The flag is meaningless.

## User Scenarios & Testing

### User Story 1 — Classify Text via MCP Tool (Priority: P1)

A developer adds `csn` as an MCP server in their Claude Code or Cursor configuration: `{"command": "csn", "args": ["mcp"]}`. When their AI agent needs to classify user input (e.g., detect pushback), it calls the `classify` tool. The process loads the model once at startup and stays resident for the session.

**Why this priority**: Core functionality — classification is the primary use case for `csn`.

**Independent Test**: Configure `csn mcp` in Claude Code settings, ask the agent to classify a sentence, verify the tool returns a valid result.

**Acceptance Scenarios**:

1. **Given** `csn mcp` is configured as an MCP server in Claude Code, **When** the agent calls the `classify` tool with text and reference set name, **Then** the tool returns match status, confidence, top phrase, and scores.
2. **Given** the client specifies a non-existent reference set, **When** the `classify` tool is called, **Then** the tool returns an error listing available reference sets.

---

### User Story 2 — List Available Reference Sets (Priority: P1)

A developer's AI agent needs to discover which reference sets are available before classifying text. The `list_sets` tool returns the names, modes, and phrase counts of all loaded reference sets.

**Why this priority**: Discovery is essential — agents need to know which reference sets exist to call `classify` correctly.

**Independent Test**: Call the `list_sets` tool via MCP, verify it returns the loaded reference sets with metadata.

**Acceptance Scenarios**:

1. **Given** `csn mcp` is running, **When** an MCP client calls the `list_sets` tool, **Then** the tool returns a list of set names, modes (binary/multi-category), and phrase counts.
2. **Given** no reference sets are found on disk, **When** the `list_sets` tool is called, **Then** the tool returns an empty list.

---

### User Story 3 — Embed Text via MCP Tool (Priority: P2)

A developer's AI agent generates embedding vectors for text — for similarity search, clustering, or custom analysis. The `embed` tool takes text and returns the embedding vector with metadata.

**Why this priority**: Useful for advanced use cases but not required for the primary classification workflow.

**Independent Test**: Call the `embed` tool with sample text, verify it returns a vector of the expected dimension.

**Acceptance Scenarios**:

1. **Given** `csn mcp` is running, **When** an MCP client calls the `embed` tool with text, **Then** the tool returns an embedding vector with dimension count and model name.

---

### User Story 4 — Compute Similarity via MCP Tool (Priority: P2)

A developer's AI agent compares two texts semantically. The `similarity` tool takes two texts and returns their cosine similarity score.

**Why this priority**: Complementary to classification — useful for custom similarity workflows but not core.

**Independent Test**: Call the `similarity` tool with two texts, verify it returns a cosine similarity score between -1 and 1.

**Acceptance Scenarios**:

1. **Given** `csn mcp` is running, **When** an MCP client calls the `similarity` tool with two texts, **Then** the tool returns a cosine similarity score.

---

### User Story 5 — CLI Direct Invocation (Priority: P1)

A developer uses `csn` directly from the terminal for quick classification, embedding, or similarity checks. The CLI runs everything in-process — no daemon, no network calls. Model and MLP weights load once per invocation.

**Why this priority**: Developer experience — quick terminal usage must remain fast and simple.

**Independent Test**: Run `csn classify "text" --set corrections`, verify result prints to stdout.

**Acceptance Scenarios**:

1. **Given** `csn` is installed, **When** a user runs `csn classify "some text" --set corrections`, **Then** the result prints to stdout with match status, confidence, and scores.
2. **Given** `csn` is installed, **When** a user runs `csn models`, **Then** available embedding models are listed.

---

### User Story 6 — Zero-Config MCP Setup (Priority: P1)

A developer adds `csn` to their MCP client with a single configuration line. No daemon to start, no ports to configure, no URLs to remember. The client spawns `csn mcp` and it just works.

**Why this priority**: Adoption — if setup is harder than `uvx some-server`, people won't use it.

**Independent Test**: Add `{"command": "csn", "args": ["mcp"]}` to Claude Code config, verify tools appear.

**Acceptance Scenarios**:

1. **Given** `csn` is in the user's PATH, **When** they add the MCP config entry, **Then** Claude Code discovers all tools within startup time.
2. **Given** the MCP client restarts, **When** it re-spawns `csn mcp`, **Then** tools are available again without manual intervention.

---

### Edge Cases

- Embedding model not yet downloaded on first MCP startup: `csn mcp` downloads the model (may take 1-2 minutes on first run). Tool calls during download should return a clear "loading" error.
- Reference set directory doesn't exist: tools that need reference sets return an error, but `embed` and `similarity` still work.
- Large embedding vectors in tool responses: serialization must handle 384+ dimension vectors without truncation.
- MCP client sends concurrent tool calls: the process handles them sequentially (single-threaded stdio) — this is standard for stdio MCP servers.
- Config file missing: sensible defaults used (same as spec 001).

## Requirements

### Functional Requirements

- **FR-001**: System MUST provide a `csn mcp` subcommand that runs an MCP server over stdio transport (JSON-RPC over stdin/stdout), compatible with the MCP protocol.
- **FR-002**: System MUST expose a `classify` tool that accepts `text` (string) and `reference_set` (string) parameters and returns match status, confidence, top phrase, and positive/negative scores as structured text content.
- **FR-003**: System MUST expose a `list_sets` tool (no parameters) that returns all loaded reference sets with name, mode, and phrase count.
- **FR-004**: System MUST expose an `embed` tool that accepts `text` (string) and returns the embedding vector, dimension count, and model name.
- **FR-005**: System MUST expose a `similarity` tool that accepts `a` (string) and `b` (string) and returns the cosine similarity score.
- **FR-006**: System MUST return structured MCP errors for invalid inputs (missing parameters, unknown reference sets) with actionable error messages.
- **FR-007**: System MUST support MCP tool discovery — clients must be able to list all available tools with parameter schemas via the standard `tools/list` method.
- **FR-008**: System MUST remove the `serve` subcommand, all REST endpoints (`/classify`, `/embed`, `/similarity`, `/health`, `/sets`), the `--standalone` flag, and the HTTP client dependency (reqwest).
- **FR-009**: System MUST remove the file watcher (hot-reload) — reference sets are loaded once at startup. The MCP process is short-lived per session; clients restart it to pick up changes.
- **FR-010**: CLI subcommands (`classify`, `embed`, `similarity`, `models`, `sets list`, `benchmark`) MUST run in-process without any network calls or daemon dependency.

### Key Entities

- **MCP Tool**: A callable function exposed to MCP clients with a name, description, and JSON Schema for input parameters.
- **Tool Result**: Structured response from a tool call, containing text content (JSON-formatted results) or error information.
- **Stdio Transport**: JSON-RPC communication over stdin/stdout, the standard MCP transport for spawned processes.

## Success Criteria

### Measurable Outcomes

- **SC-001**: All four MCP tools (classify, embed, similarity, list_sets) are discoverable and callable from Claude Code within 30 seconds of adding the configuration.
- **SC-002**: `csn mcp` startup time (model load + MLP train/cache) completes within 5 seconds on subsequent runs (model cached).
- **SC-003**: Tool parameter schemas are valid JSON Schema and render correctly in MCP client tool inspectors.
- **SC-004**: CLI commands produce identical output to pre-refactor behavior (minus `--standalone` flag and `serve` subcommand).
- **SC-005**: Binary size decreases after removing axum, reqwest, notify, and tower dependencies.

## Assumptions

- The MCP protocol (latest stable version) is the target. The Rust MCP SDK (`rust-mcp-sdk` or equivalent) provides stdio transport support.
- Stdio is the only transport — no SSE, no HTTP for MCP. This matches the ecosystem convention (uvx/npx pattern).
- MCP clients (Claude Code, Cursor) manage the `csn mcp` process lifecycle — spawning on connect, killing on disconnect.
- The embedding model download happens on first run. Subsequent runs use the cached model.
- Sequential tool call handling is acceptable for stdio MCP (single client per process).
- The file watcher is removed — reference set changes require restarting the MCP process (client handles this naturally).
- Authentication is out of scope — stdio transport is local-only by nature.
- The `benchmark` subcommand is preserved unchanged (it runs in-process already).
