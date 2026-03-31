# Quickstart: Computer Says No

## Install

Download the `csn` binary for your platform and place it in your PATH.

## First Run

```bash
# Start the daemon (downloads model on first run ~130MB)
csn serve

# In another terminal — classify text
csn classify "no, use X instead" --set corrections --json

# Check similarity between texts
csn similarity "fix the bug" "resolve the issue"

# List available models
csn models

# List loaded reference sets
csn sets list
```

## Create a Custom Reference Set

Create `~/.config/computer-says-no/reference-sets/my-set.toml`:

```toml
[metadata]
name = "my-set"
mode = "binary"
threshold = 0.45

[phrases]
positive = ["match this", "and this"]
negative = ["not this"]
```

The daemon detects the new file and loads it automatically.

## Configuration

Create `~/.config/computer-says-no/config.toml`:

```toml
port = 9847
model = "nomic-embed-text-v1.5-Q"
log_level = "info"
```

Or use environment variables: `CSN_PORT=8080 csn serve`.

## Use in Claude Code Hooks

```bash
#!/usr/bin/env bash
RESULT=$(curl -s -X POST http://localhost:9847/classify \
  -H "Content-Type: application/json" \
  -d "{\"text\": \"$PROMPT\", \"reference_set\": \"corrections\"}")

if echo "$RESULT" | jq -e '.match == true' >/dev/null 2>&1; then
  echo '{"additionalContext": "Correction detected — save to Vestige"}'
fi
```
