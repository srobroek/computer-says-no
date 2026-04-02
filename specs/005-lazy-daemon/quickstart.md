# Quickstart: Lazy Daemon (005)

## What Changed

CLI commands (`classify`, `embed`, `similarity`) now auto-start a background daemon on first use. The daemon keeps the embedding model warm in memory, making subsequent calls ~50x faster (~5ms vs ~254ms).

## For Users

Nothing changes. Same commands, same output. The daemon is invisible — it starts automatically, serves requests, and exits after 5 minutes of idle.

```fish
# First call: ~254ms (starts daemon)
csn classify "test" --set corrections --json

# Second call: ~5ms (daemon warm)
csn classify "another test" --set corrections --json

# Manual restart (e.g., after editing reference sets)
csn stop
```

## For Developers

### New files
- `src/daemon.rs` — daemon server (unix socket listener, idle timeout, PID management)
- `src/client.rs` — daemon client (connect, send request, receive response, spawn if needed)

### New subcommands
- `csn daemon` (hidden) — internal subcommand used to start the daemon process
- `csn stop` — manually stop the running daemon

### Config
```toml
# ~/.config/computer-says-no/config.toml
[daemon]
idle_timeout = 300  # seconds (default: 5 minutes)
```
