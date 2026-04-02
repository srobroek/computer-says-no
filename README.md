# Computer Says No (`csn`)

Local embedding classifier for real-time text classification. Built for AI agent hooks — classify user messages for frustration, corrections, or any custom pattern in under 5ms.

## What it does

`csn` embeds text using ONNX models (via fastembed) and classifies it against reference sets of example phrases. A 2-layer MLP neural network with character n-gram features provides typo-robust classification.

**Primary use case**: A Claude Code `UserPromptSubmit` hook that detects user frustration and corrections on every message, prompting the agent to reflect, acknowledge, and course-correct.

## Quick start

```fish
# Build
cargo build --release

# Classify text
./target/release/csn classify "what the fuck" --set corrections --json
# → {"match": true, "confidence": 0.99, "top_phrase": "what in the actual fuck", ...}

# First call starts a background daemon (~254ms). Subsequent calls: ~5ms.
./target/release/csn classify "looks good ship it" --set corrections --json
# → {"match": false, "confidence": 0.00, ...}
```

## Installation

### Prerequisites

- Rust 1.92+ (2024 edition)
- ~500MB disk for ONNX model cache (downloaded on first run)

### Build

```fish
git clone https://github.com/srobroek/computer-says-no.git
cd computer-says-no
cargo build --release
```

The binary is at `target/release/csn`. No runtime dependencies — single binary, models cached locally.

### Verify

```fish
./target/release/csn classify "test" --set corrections --json
# Should output JSON with match/confidence/top_phrase/scores
```

## Architecture

```
User message → csn classify → daemon (warm) or in-process (cold)
                                  ↓
                          embed text (ONNX)
                                  ↓
                    cosine similarity + char n-grams
                                  ↓
                          MLP classifier
                                  ↓
                      match / no-match + confidence
```

- **CLI**: `csn classify`, `csn embed`, `csn similarity` — auto-route through background daemon
- **MCP server**: `csn mcp` — stdio transport for Claude Code/Cursor agent tools
- **Daemon**: Unix socket at `~/.cache/computer-says-no/csn.sock` — auto-starts on first CLI call, self-exits after 5 min idle

## Hook setup (Claude Code)

The hook classifies every user message and provides feedback to the agent when it detects corrections or frustration.

### 1. Add the hook to your project

Copy the hook script to your project's `.claude/hooks/` directory:

```fish
mkdir -p .claude/hooks
cp path/to/computer-says-no/.claude/hooks/user-frustration-check.sh .claude/hooks/
chmod +x .claude/hooks/user-frustration-check.sh
```

### 2. Edit the hook paths

Open `.claude/hooks/user-frustration-check.sh` and update `REPO_ROOT` to point to where you built csn:

```bash
# Option A: Hardcode the path
CSN="/path/to/computer-says-no/target/release/csn"
SETS_DIR="/path/to/computer-says-no/reference-sets"

# Option B: Use an environment variable
CSN="${CSN_BIN:-/path/to/computer-says-no/target/release/csn}"
SETS_DIR="${CSN_SETS_DIR:-/path/to/computer-says-no/reference-sets}"
```

### 3. Register the hook in project settings

Add to your project's `.claude/settings.json`:

```json
{
  "hooks": {
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": ".claude/hooks/user-frustration-check.sh",
            "timeout": 5
          }
        ]
      }
    ]
  }
}
```

Or add to your global `~/.claude/settings.json` to enable for all projects.

### 4. Configure threshold (optional)

The default threshold is 80% confidence. Set `CSN_FRUSTRATION_THRESHOLD` to adjust:

```fish
# In your shell config or .envrc
set -gx CSN_FRUSTRATION_THRESHOLD 0.75  # more sensitive
set -gx CSN_FRUSTRATION_THRESHOLD 0.90  # less sensitive
```

### What the hook detects

The `corrections` reference set catches two types of signals:

| Signal type | Examples | What it means |
|-------------|----------|---------------|
| **Corrections** | "no revert that", "wrong file", "that's not what I asked" | The agent did the wrong thing — acknowledge and fix |
| **Frustration** | "what the fuck", "are you kidding me", "I give up" | The user is upset — reflect on what went wrong, adjust approach |
| **Sarcasm** | "great, another bug", "thanks for nothing", "chef's kiss of bad code" | Passive-aggressive feedback — take it seriously |

The hook prompt instructs the agent to:
1. **Reflect** on what it did wrong (last 2-3 actions)
2. **Acknowledge** the issue without being defensive
3. **Course-correct** its approach
4. **Save a lesson learned** to memory for future sessions

## Reference sets

Reference sets are TOML files in `reference-sets/` that define classification patterns.

### Format

```toml
[metadata]
name = "corrections"
description = "Detect developer pushback, frustration, and correction"
mode = "binary"
threshold = 0.5

[phrases]
positive = [
    "no",
    "wrong",
    "revert that",
    "what the fuck",
    # ... more patterns
]
negative = [
    "looks good",
    "perfect",
    "great work",
    # ... non-matching patterns
]
```

- **positive**: Phrases that should trigger a match
- **negative**: Phrases that should NOT match (helps the MLP learn the boundary)
- **threshold**: Cosine similarity threshold for the pure-cosine fallback path

### Creating custom reference sets

1. Create a new `.toml` file in `reference-sets/`:

```toml
[metadata]
name = "my-pattern"
description = "Detect my custom pattern"
mode = "binary"
threshold = 0.5

[phrases]
positive = [
    "example positive phrase 1",
    "example positive phrase 2",
]
negative = [
    "example negative phrase 1",
    "example negative phrase 2",
]
```

2. Classify against it:

```fish
csn classify "test input" --set my-pattern --json
```

3. The MLP trains automatically on first use and caches weights.

### Tuning tips

- **More phrases = better accuracy.** The MLP needs diverse examples to generalize. Aim for 50+ positive and 50+ negative phrases.
- **Include near-misses in negatives.** Phrases that look similar but shouldn't match help the model learn the boundary (e.g., "holy shit that's amazing" as a negative for frustration detection).
- **Include typo variants in positives** if you want typo robustness beyond what character n-grams provide (e.g., "srsly" alongside "seriously").
- **Test with `csn classify --json`** to check confidence scores. Adjust your hook threshold based on where false positives/negatives land.

## System learning loop

The hook creates a feedback loop where the agent learns from its mistakes:

```
User sends frustrated message
        ↓
Hook classifies → FRUSTRATION DETECTED (95%)
        ↓
Agent reflects on what it did wrong
        ↓
Agent acknowledges and adjusts approach
        ↓
Agent saves lesson to memory system
        ↓
Future sessions: agent recalls lessons, avoids repeating mistakes
```

### Memory integration

The hook instructs the agent to save lessons to "any available memory system." This works with:

- **Vestige MCP** — `smart_ingest` for spaced-repetition memory
- **File-based memory** — `~/.claude/projects/.../memory/` markdown files
- **Any memory MCP** — the instruction is system-agnostic

### Tuning the learning prompt

Edit the `additionalContext` string in the hook script to change what the agent does when corrections/frustration are detected. The current prompt has 4 required actions (reflect, acknowledge, course-correct, learn). You can:

- Remove the LEARN step if you don't want memory integration
- Add a SUMMARIZE step to log incidents
- Change REFLECT to focus on specific patterns (e.g., "check if you ignored the user's explicit instruction")
- Add severity tiers (>95% = strong frustration, >80% = mild correction)

## Commands

| Command | Description |
|---------|-------------|
| `csn classify <text> --set <name> --json` | Classify text against a reference set |
| `csn embed <text>` | Generate embedding vector |
| `csn similarity <a> <b>` | Cosine similarity between two texts |
| `csn mcp` | Run as MCP server (stdio, for Claude Code) |
| `csn stop` | Stop the background daemon |
| `csn models` | List available embedding models |
| `csn sets list` | List loaded reference sets |
| `csn benchmark run` | Run accuracy benchmark |

## Configuration

Config file: `~/.config/computer-says-no/config.toml`

```toml
model = "bge-small-en-v1.5-Q"    # embedding model
log_level = "warn"                 # trace, debug, info, warn, error

[mlp]
fallback = false                   # fall back to cosine if MLP training fails
learning_rate = 0.001
weight_decay = 0.001
max_epochs = 500
patience = 10

[daemon]
idle_timeout = 300                 # seconds before daemon self-exits (default: 5 min)
```

Environment variable overrides: `CSN_MODEL`, `CSN_LOG_LEVEL`, `CSN_SETS_DIR`, `CSN_CACHE_DIR`, `CSN_IDLE_TIMEOUT`, `CSN_MLP_FALLBACK`.

## Performance

| Metric | Value |
|--------|-------|
| Warm-path classify (daemon running) | ~5ms |
| Cold-start classify (daemon starts) | ~370ms |
| Model load + MLP train (first ever) | ~10s |
| Binary size (release) | ~25MB |
| MLP training (cached weights) | 0ms (loaded from disk) |
| Character feature overhead | <3ms |

## How it works

1. **Embedding**: fastembed (ONNX) embeds text into a 384-dim vector
2. **Cosine features**: Max similarity to positive/negative reference phrases (3 values)
3. **Character n-grams**: 256-dim hashed bigram/trigram features for typo robustness
4. **MLP**: 2-layer neural network (643 → 256 → 128 → 1, sigmoid) classifies the combined features
5. **Daemon**: Background process keeps the model warm via unix socket

## License

Apache-2.0
