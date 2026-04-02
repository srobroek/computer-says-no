# Feature Specification: Lazy Auto-Starting Daemon

**Feature Branch**: `005-lazy-daemon`
**Created**: 2026-04-02
**Status**: Draft
**Input**: User description: "Lazy auto-starting background daemon for fast CLI classification. When `csn classify` is invoked, the binary checks if a persistent daemon is already running (via unix socket). If running: send the classify request over the socket and return the result (~5ms). If not running: fork a detached background daemon process, wait for socket, then send the request. First invocation pays ~254ms cold start, all subsequent ~5ms. Idle timeout self-exit. Transparent to callers."

## Clarifications

- Q: Should all CLI commands route through the daemon, or only classify? → A: All data commands (classify, embed, similarity) benefit. Model-listing and set-listing are instant and don't need the daemon.
- Q: Should the daemon reload reference sets when files change? → A: No. Spec 004 removed hot-reload. Daemon loads once at startup. Users restart via `csn stop` + next invocation to pick up changes.
- Q: Should the MCP server share the daemon? → A: No. MCP runs over stdio with its own lifecycle. The daemon serves CLI commands only.

### Session 2026-04-02

- Q: What wire protocol should the daemon use over the unix socket? → A: JSON lines (newline-delimited JSON request/response).
- Q: How should the daemon handle concurrent requests? → A: Queue them — process one at a time, subsequent requests wait.

## User Scenarios & Testing

### User Story 1 — Transparent Fast Classification (Priority: P1)

A developer has a Claude Code hook that classifies every user prompt for frustration detection. The hook calls `csn classify "user message" --set corrections --json`. The first call in a session takes ~254ms (cold start), but every subsequent call completes in ~5ms because a background daemon is already warm with the model loaded.

**Why this priority**: This is the core value proposition — making per-prompt classification fast enough for interactive hooks without any manual daemon management.

**Independent Test**: Run `csn classify "test" --set corrections --json` twice in sequence. First call starts the daemon. Second call should complete in under 30ms.

**Acceptance Scenarios**:

1. **Given** no daemon is running, **When** `csn classify` is invoked, **Then** a background daemon starts, the classification completes, and the result is identical to a direct in-process classification.
2. **Given** a daemon is already running, **When** `csn classify` is invoked, **Then** the classification completes via the socket in under 30ms.
3. **Given** a daemon is running, **When** a second concurrent `csn classify` is invoked, **Then** both requests are served without error.

---

### User Story 2 — Daemon Self-Exits on Idle (Priority: P2)

After a period of inactivity (no requests for a configurable duration), the daemon process exits cleanly, releasing its resources. The next `csn classify` call transparently starts a new daemon.

**Why this priority**: Prevents resource waste when the developer stops using Claude Code or switches projects. Without this, orphan daemon processes accumulate.

**Independent Test**: Start the daemon via `csn classify`, wait for the idle timeout to expire, verify the process has exited and the socket file is cleaned up.

**Acceptance Scenarios**:

1. **Given** a running daemon with no requests for the idle timeout duration, **When** the timeout expires, **Then** the daemon exits cleanly, removes the socket file, and removes the PID file.
2. **Given** the daemon has self-exited, **When** `csn classify` is invoked, **Then** a new daemon starts transparently.

---

### User Story 3 — Manual Daemon Control (Priority: P3)

A developer can manually stop the daemon via `csn stop` to force a clean restart (e.g., after updating reference sets or changing configuration).

**Why this priority**: Escape hatch for when the daemon needs to pick up changes. Low priority because it's an infrequent operation.

**Independent Test**: Start daemon via `csn classify`, run `csn stop`, verify process exited and socket removed.

**Acceptance Scenarios**:

1. **Given** a running daemon, **When** `csn stop` is invoked, **Then** the daemon exits gracefully, the socket file is removed, and the PID file is removed.
2. **Given** no daemon is running, **When** `csn stop` is invoked, **Then** a message indicates no daemon is running and the command exits cleanly.

---

### User Story 4 — Embed and Similarity via Daemon (Priority: P2)

The `csn embed` and `csn similarity` commands also route through the daemon when available, benefiting from the same warm-model fast path.

**Why this priority**: Same latency benefit as classify. These commands share the same model initialization bottleneck.

**Independent Test**: With daemon running, call `csn embed "test"` and `csn similarity "a" "b"`. Verify results match direct in-process execution and complete in under 30ms.

**Acceptance Scenarios**:

1. **Given** a running daemon, **When** `csn embed "text"` is invoked, **Then** the embedding result is returned via the socket in under 30ms.
2. **Given** a running daemon, **When** `csn similarity "a" "b"` is invoked, **Then** the similarity score is returned via the socket in under 30ms.

---

### Edge Cases

- What happens when the socket file exists but the daemon process is dead (stale socket)? → Detect via PID file, clean up stale socket, start fresh daemon.
- What happens when two `csn classify` calls race to start a daemon? → Only one should win; the other should wait for the socket to appear and use the running daemon.
- What happens when the daemon crashes mid-request? → CLI should detect the connection failure, fall back to in-process classification, and clean up stale files.
- What happens when the config or reference sets change while daemon is running? → Daemon uses its startup config. Changes require `csn stop` + restart.
- What happens when disk is full and socket/PID files can't be created? → Fall back to in-process classification with a warning.

## Requirements

### Functional Requirements

- **FR-001**: When `csn classify`, `csn embed`, or `csn similarity` is invoked, the system MUST check for a running daemon via the unix socket before performing in-process classification.
- **FR-002**: If a daemon is running and reachable, the system MUST send the request as a newline-delimited JSON message over the unix socket and return the daemon's JSON response.
- **FR-003**: If no daemon is running, the system MUST fork a detached background daemon process (own process group, survives parent exit), wait for the socket to become available, and then send the request.
- **FR-004**: The forked daemon MUST load the embedding model, reference sets, and MLP weights once at startup and serve all subsequent requests from memory.
- **FR-005**: The daemon MUST listen on a unix socket at a well-known path in the cache directory (`~/.cache/computer-says-no/csn.sock`).
- **FR-006**: The daemon MUST write its PID to a file alongside the socket for lifecycle management.
- **FR-007**: The daemon MUST self-exit after a configurable idle timeout (default: 5 minutes) with no incoming requests. On exit, it MUST remove the socket file and PID file.
- **FR-008**: The system MUST provide a `csn stop` subcommand that sends a termination signal to the daemon and cleans up socket and PID files.
- **FR-009**: If the socket file exists but the daemon process is not running (stale socket), the system MUST clean up stale files and start a fresh daemon.
- **FR-010**: If two CLI invocations race to start a daemon, only one MUST succeed in spawning; the other MUST detect the newly created socket and connect to it.
- **FR-011**: If the daemon is unreachable (crash, timeout), the system MUST fall back to in-process classification and clean up stale socket/PID files.
- **FR-012**: The `csn mcp` subcommand MUST NOT be affected — it continues to run over stdio with its own lifecycle, independent of the daemon.
- **FR-013**: The daemon MUST use the same classification logic as in-process mode — results MUST be identical for the same input.

### Key Entities

- **Daemon Process**: Background process holding the warm embedding model, reference sets, and MLP weights. Listens on unix socket.
- **Socket File**: Unix domain socket at a well-known path. Presence indicates a daemon may be running.
- **PID File**: Contains the daemon's process ID. Used for stale detection and manual stop.

## Success Criteria

### Measurable Outcomes

- **SC-001**: Warm-path classification (daemon running) completes in under 30ms end-to-end from CLI invocation to result output. All subsequent calls maintain this latency for the daemon's lifetime.
- **SC-002**: Cold-start classification (no daemon) completes in under 500ms, including daemon startup.
- **SC-003**: Daemon idle shutdown releases all resources (memory, socket file, PID file) within 5 seconds of timeout expiry.
- **SC-004**: Classification results from the daemon are byte-identical to in-process classification results for the same input.
- **SC-005**: Existing MCP server (`csn mcp`) continues to function correctly with no behavioral changes.

## Assumptions

- The unix socket transport is sufficient for the CLI-to-daemon protocol. No need for TCP or HTTP overhead.
- The daemon serves a single user on a single machine — no authentication or multi-tenancy needed.
- The daemon's idle timeout (default 5 min) is sufficient for typical Claude Code session gaps. Users who need longer can configure it.
- The daemon processes requests sequentially (one at a time, queued). Concurrent inference is not needed for single-user local use.
- The daemon runs with the same user permissions as the CLI — no privilege escalation needed.
- The MCP server (`csn mcp`) has its own lifecycle and does not interact with the daemon.
