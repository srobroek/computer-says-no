# Research: Lazy Auto-Starting Daemon

## Unix Socket Server

**Decision**: Use `tokio::net::UnixListener` for the daemon's socket server.

**Rationale**: tokio is already a dependency (via rust-mcp-sdk). UnixListener provides async accept/read/write over unix domain sockets. No additional HTTP framework needed — JSON lines over raw socket is sufficient for single-user local use.

**Alternatives considered**:
- axum on UDS: Overkill — adds HTTP framing overhead. We removed axum in spec 004.
- hyper on UDS: Same overhead issue. JSON lines is simpler.
- synchronous std::os::unix::net: Would require threading. tokio is already available.

## Process Daemonization

**Decision**: Use `std::process::Command` with `.process_group(0)` and stdin/stdout/stderr redirected to `/dev/null`. Add `nix` crate for `setsid` and PID checks.

**Rationale**: `.process_group(0)` creates a new process group, detaching from parent. The `nix` crate (0.29, blessed.rs listed) provides safe wrappers for `kill(pid, None)` (PID alive check). Single dependency covers both daemonization and lifecycle needs.

**Alternatives considered**:
- `daemonize` crate: Full double-fork, but unmaintained since 2018.
- Raw `libc`: Works but unsafe for multiple call sites.
- `fork` crate: Wraps libc fork. We only need process_group + PID check.

## Stale Socket / PID Detection

**Decision**: Write PID to file on daemon startup. On connect failure, read PID file, check process alive via `nix::sys::signal::kill(pid, None)`. If dead, remove socket + PID file, start fresh.

**Rationale**: `kill(pid, 0)` is the standard Unix mechanism — sends no signal but returns whether the process exists. The `nix` crate provides a safe wrapper. Adding `nix` is justified since we also use it for `setsid` in daemonization — one dependency for both needs.

**Alternatives considered**:
- Raw `libc::kill`: Works but unsafe, less ergonomic.
- `/proc/{pid}`: Linux-only, not portable to macOS.
- Advisory file locks (flock): More complex, doesn't help with socket cleanup.

## Idle Timeout

**Decision**: `Arc<AtomicU64>` storing the last request timestamp (epoch seconds). A background tokio task checks every 30 seconds; if `now - last_request > timeout`, initiate graceful shutdown.

**Rationale**: Lightweight, no external crate. AtomicU64 avoids mutex contention on the hot path (every request updates it). The check interval (30s) means actual shutdown is within 30s of the configured timeout — acceptable for a 5-minute default.

**Alternatives considered**:
- tokio::time::timeout on accept: Would need restructuring the accept loop.
- Signal-based: Overly complex for a timer.

## Wire Protocol

**Decision**: JSON lines — one JSON object per line, newline-terminated. Request and response are each a single line.

**Rationale**: Simplest protocol that works. serde_json already in the project. No framing, no length prefixes, no HTTP overhead. Easy to test with socat.

**Request format**:
```json
{"command":"classify","args":{"text":"hello","set":"corrections"}}
```

**Response format**:
```json
{"ok":true,"result":{...}}
```
or
```json
{"ok":false,"error":"message"}
```

## Race Condition on Daemon Startup

**Decision**: Use file-based lock (flock on a lock file) during daemon spawn. The first CLI process to acquire the lock spawns the daemon. Others wait for the socket to appear with exponential backoff (10ms, 20ms, 40ms... up to 500ms total).

**Rationale**: File locks are the standard Unix coordination mechanism. Advisory locks via flock are released automatically on process exit, avoiding stale lock issues.

## Daemon Internal Subcommand

**Decision**: Add a hidden `csn daemon` subcommand (not shown in help) that the CLI forks. This keeps the daemon startup logic in the same binary — no separate executable.

**Rationale**: Single binary principle. The daemon is just csn running in "serve on socket" mode. The daemon subcommand is an implementation detail, not a user-facing command.
