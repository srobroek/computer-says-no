# CLI Contract

**Binary**: `csn`

## Subcommands

### csn classify

```
csn classify <TEXT> --set <NAME> [--json] [--model <MODEL>] [--sets-dir <PATH>] [--standalone]
```

- Connects to running daemon (reads port from config)
- If daemon unreachable: exit code 1, warning to stderr suggesting `csn serve` or `--standalone`
- `--standalone`: load model in-process, classify without daemon (cold start ~1-2s, no server needed)
- Default output: human-readable (`MATCH (confidence: 0.88)`)
- `--json`: machine-readable JSON (same schema as REST /classify)
- Exit code 0 on success, 1 on error

### csn embed

```
csn embed <TEXT> [--model <MODEL>]
```

- Connects to daemon for embedding
- Output: JSON with embedding vector, dimensions, model name
- If daemon unreachable: exit code 1

### csn similarity

```
csn similarity <A> <B> [--model <MODEL>]
```

- Connects to daemon
- Output: decimal similarity score (e.g., `0.7515`)
- If daemon unreachable: exit code 1

### csn serve

```
csn serve [--port <PORT>] [--model <MODEL>] [--sets-dir <PATH>]
```

- Starts daemon on 127.0.0.1:{port}
- Loads model, embeds reference sets, starts file watcher, binds port
- Logs to stderr via tracing (level controlled by `CSN_LOG_LEVEL` or `--log-level`)
- Graceful shutdown on SIGTERM/SIGINT

### csn models

```
csn models
```

- Lists supported models with name and dimensions
- Does not require daemon

### csn sets list

```
csn sets list [--sets-dir <PATH>]
```

- Lists reference sets from directory
- Requires loading a model to parse/validate sets
- Does not require daemon

## Configuration Precedence

1. CLI flags (highest)
2. Environment variables (`CSN_PORT`, `CSN_MODEL`, `CSN_LOG_LEVEL`, `CSN_SETS_DIR`, `CSN_CACHE_DIR`)
3. Config file (`~/.config/computer-says-no/config.toml`)
4. Built-in defaults (lowest)

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Runtime error (daemon unreachable, invalid set, model load failure) |
| 2 | Usage error (invalid args — handled by clap) |
