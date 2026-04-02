# Data Model: Lazy Auto-Starting Daemon

## Entities

### DaemonState (runtime, not persisted)

The daemon process holds the warm model and serves requests.

| Attribute | Description |
|-----------|-------------|
| engine | Loaded EmbeddingEngine (ONNX model in memory) |
| reference_sets | Loaded and embedded reference sets |
| trained_models | MLP classifiers trained from reference sets |
| model_choice | Which embedding model is active |
| last_request | Epoch timestamp of most recent request (AtomicU64) |
| idle_timeout | Duration after which daemon self-exits |

### Socket Protocol Messages

#### Request

| Field | Type | Description |
|-------|------|-------------|
| command | string | One of: "classify", "embed", "similarity" |
| args | object | Command-specific arguments |

**classify args**: `{ "text": string, "set": string }`
**embed args**: `{ "text": string }`
**similarity args**: `{ "a": string, "b": string }`

#### Response (success)

| Field | Type | Description |
|-------|------|-------------|
| ok | bool | Always true |
| result | object | Command-specific result (same shape as CLI JSON output) |

#### Response (error)

| Field | Type | Description |
|-------|------|-------------|
| ok | bool | Always false |
| error | string | Error message |

### Filesystem Artifacts

| File | Path | Lifecycle |
|------|------|-----------|
| Socket | `~/.cache/computer-says-no/csn.sock` | Created on daemon start, removed on exit |
| PID file | `~/.cache/computer-says-no/csn.pid` | Created on daemon start, removed on exit |
| Lock file | `~/.cache/computer-says-no/csn.lock` | Acquired during spawn, auto-released |

## State Transitions

```
CLI invoked
  → Check socket exists?
    → Yes → Try connect
      → Success → Send request → Return response (WARM PATH)
      → Failure → Check PID alive?
        → Dead → Clean stale files → Go to "No"
        → Alive → Retry connect (backoff)
    → No → Acquire lock file
      → Got lock → Fork daemon → Wait for socket → Send request (COLD PATH)
      → Lock busy → Wait for socket (another process is spawning)
```

```
Daemon lifecycle:
  Started → Listening → Serving requests → Idle timeout → Shutdown → Clean up files
                          ↑                    ↓
                          └── Request resets ──┘
```
