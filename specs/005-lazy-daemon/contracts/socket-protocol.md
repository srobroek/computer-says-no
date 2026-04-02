# Contract: Unix Socket Protocol

## Transport

- Unix domain socket at `~/.cache/computer-says-no/csn.sock`
- JSON lines: one JSON object per line, newline-terminated (`\n`)
- Request-response: client sends one line, daemon responds with one line
- Sequential processing: requests are queued, one at a time

## Request Schema

```json
{"command": "<command>", "args": {<command-specific>}}
```

### classify

```json
{"command": "classify", "args": {"text": "input text", "set": "corrections"}}
```

### embed

```json
{"command": "embed", "args": {"text": "input text"}}
```

### similarity

```json
{"command": "similarity", "args": {"a": "first text", "b": "second text"}}
```

## Response Schema

### Success

```json
{"ok": true, "result": <command-specific-result>}
```

Result shapes match existing CLI JSON output:
- **classify**: `{"match": bool, "confidence": float, "top_phrase": string, "scores": {"positive": float, "negative": float}}`
- **embed**: `{"embedding": [float...], "dimensions": int, "model": string}`
- **similarity**: `{"similarity": float, "model": string}`

### Error

```json
{"ok": false, "error": "human-readable error message"}
```

## Lifecycle Protocol

No handshake. No keepalive. Each connection is one request-response, then closed.

The daemon may close the socket and exit at any time (idle timeout). Clients must handle connection refused gracefully.
