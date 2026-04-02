# Feature Specification: MCP Server Integration

**Feature Branch**: `004-mcp-sse`
**Created**: 2026-04-02
**Status**: Draft
**Input**: User description: "Expose csn as an MCP server over SSE transport for Claude Code, Cursor, and other MCP-compatible clients. Tools: classify, embed, similarity, list sets."

## User Scenarios & Testing

### User Story 1 — Classify Text via MCP Tool (Priority: P1)

A developer adds `csn` as an MCP server in their Claude Code or Cursor configuration. When their AI agent needs to classify user input (e.g., detect pushback), it calls the `classify` tool with text and a reference set name. The tool returns the classification result — match/no-match, confidence, top phrase, and scores — in the same shape as the REST API.

**Why this priority**: Core functionality — classification is the primary use case for `csn`. Without this, MCP integration has no value.

**Independent Test**: Configure `csn` as an MCP server in Claude Code settings, ask the agent to classify a sentence, verify the tool call returns a valid result.

**Acceptance Scenarios**:

1. **Given** the daemon is running with MCP enabled, **When** an MCP client calls the `classify` tool with text and reference set name, **Then** the tool returns a classification result with match status, confidence, top phrase, and scores.
2. **Given** the client specifies a non-existent reference set, **When** the `classify` tool is called, **Then** the tool returns an error listing available reference sets.

---

### User Story 2 — List Available Reference Sets (Priority: P1)

A developer's AI agent needs to discover which reference sets are available before classifying text. The `list_sets` tool returns the names, modes, and phrase counts of all loaded reference sets.

**Why this priority**: Discovery is essential — agents need to know which reference sets exist to call `classify` correctly.

**Independent Test**: Call the `list_sets` tool via MCP, verify it returns the loaded reference sets with metadata.

**Acceptance Scenarios**:

1. **Given** the daemon has reference sets loaded, **When** an MCP client calls the `list_sets` tool, **Then** the tool returns a list of set names, modes (binary/multi-category), and phrase counts.
2. **Given** no reference sets are loaded, **When** the `list_sets` tool is called, **Then** the tool returns an empty list.

---

### User Story 3 — Embed Text via MCP Tool (Priority: P2)

A developer's AI agent needs to generate embedding vectors for text — for similarity search, clustering, or custom analysis. The `embed` tool takes text and returns the embedding vector with metadata.

**Why this priority**: Useful for advanced use cases but not required for the primary classification workflow.

**Independent Test**: Call the `embed` tool with sample text, verify it returns a vector of the expected dimension.

**Acceptance Scenarios**:

1. **Given** the daemon is running with MCP enabled, **When** an MCP client calls the `embed` tool with text, **Then** the tool returns an embedding vector with dimension metadata.

---

### User Story 4 — Compute Similarity via MCP Tool (Priority: P2)

A developer's AI agent needs to compare two texts semantically. The `similarity` tool takes two texts and returns their cosine similarity score.

**Why this priority**: Complementary to classification — useful for custom similarity workflows but not core.

**Independent Test**: Call the `similarity` tool with two texts, verify it returns a cosine similarity score between -1 and 1.

**Acceptance Scenarios**:

1. **Given** the daemon is running with MCP enabled, **When** an MCP client calls the `similarity` tool with two texts, **Then** the tool returns a cosine similarity score.

---

### User Story 5 — MCP Client Configuration (Priority: P1)

A developer adds `csn` to their MCP client configuration (Claude Code, Cursor, etc.) by pointing to the daemon's SSE endpoint. The connection establishes automatically and the tools become available.

**Why this priority**: Usability — if configuration is difficult, adoption fails regardless of tool quality.

**Independent Test**: Add the SSE endpoint URL to Claude Code's MCP settings, verify tools appear in the tool list.

**Acceptance Scenarios**:

1. **Given** the daemon is running with MCP enabled, **When** a developer adds the SSE endpoint URL to their MCP client, **Then** the client connects and discovers all available tools.
2. **Given** the daemon restarts, **When** the MCP client reconnects, **Then** tools are rediscovered without manual reconfiguration.

---

### Edge Cases

- MCP client connects before reference sets finish loading: tools should be available but classify may return an error until sets are ready.
- Multiple MCP clients connect simultaneously: all should receive tool results independently.
- MCP client disconnects and reconnects: session state should not leak between connections.
- Large embedding vectors in tool responses: serialization must handle 384+ dimension vectors without truncation.
- MCP endpoint and REST API running simultaneously on same daemon: both must function without interference.

## Requirements

### Functional Requirements

- **FR-001**: System MUST expose an MCP-compatible server endpoint on the daemon using SSE transport, accessible at a configurable path.
- **FR-002**: System MUST expose a `classify` tool that accepts text and reference set name parameters and returns the classification result (match status, confidence, top phrase, positive/negative scores).
- **FR-003**: System MUST expose a `list_sets` tool that returns all loaded reference sets with their name, mode, and phrase count.
- **FR-004**: System MUST expose an `embed` tool that accepts text and returns the embedding vector with dimension count and model name.
- **FR-005**: System MUST expose a `similarity` tool that accepts two texts and returns their cosine similarity score.
- **FR-006**: System MUST support multiple concurrent MCP client connections without interference.
- **FR-007**: System MUST coexist with the existing REST API on the same daemon process — both transports share the same AppState (reference sets, trained models, embedding engine).
- **FR-008**: System MUST return structured MCP errors for invalid inputs (missing parameters, unknown reference sets) with actionable error messages.
- **FR-009**: System MUST support MCP tool discovery — clients connecting via SSE must be able to list all available tools and their parameter schemas.
- **FR-010**: MCP tools MUST be configurable — the user can enable or disable the MCP endpoint via configuration (default: enabled when daemon runs).

### Key Entities

- **MCP Tool**: A callable function exposed to MCP clients with a name, description, and JSON Schema for parameters.
- **SSE Endpoint**: Server-Sent Events connection point for MCP protocol communication.
- **Tool Result**: Structured response from a tool call, containing text content or error information.

## Success Criteria

### Measurable Outcomes

- **SC-001**: All four MCP tools (classify, embed, similarity, list_sets) are discoverable and callable from Claude Code within 30 seconds of configuration.
- **SC-002**: MCP classify tool returns identical results to the REST `/classify` endpoint for the same input.
- **SC-003**: The daemon supports at least 5 concurrent MCP client connections without degradation.
- **SC-004**: MCP endpoint adds no more than 2 seconds to daemon startup time.
- **SC-005**: Tool parameter schemas are valid JSON Schema and render correctly in MCP client tool inspectors.

## Assumptions

- The MCP protocol version 2025-11-25 (latest stable) is the target. Older protocol versions are out of scope.
- SSE transport is the primary transport. Streamable HTTP is enabled for forward compatibility but not the primary focus.
- The existing daemon architecture (axum-based) can host the MCP endpoint alongside REST routes.
- MCP clients (Claude Code, Cursor) follow the standard MCP protocol for tool discovery and invocation.
- Authentication is out of scope — the daemon runs locally (127.0.0.1 by default) and trusts all connections, consistent with spec 001's security model.
- Stdio transport (for non-networked MCP) is out of scope — the daemon is a long-running HTTP process.
