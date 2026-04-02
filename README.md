# Computer Says No (`csn`)

Local embedding classifier for real-time text classification. Built for AI agent hooks — classify user messages for frustration, corrections, or any custom pattern in under 5ms.

## What it does

`csn` embeds text using ONNX models (via fastembed) and classifies it against reference sets of example phrases. An MLP neural network with character n-gram features provides typo-robust classification with per-category confidence scores.

**Primary use case**: A Claude Code `UserPromptSubmit` hook that detects user frustration and corrections on every message, prompting the agent to reflect, acknowledge, and course-correct.

## Quick start

```fish
cargo install computer-says-no

# Classify text (multi-category)
csn classify "what the fuck" --set corrections --json
# → {"category": "frustration", "confidence": 0.99, "all_scores": [...]}

csn classify "wrong file" --set corrections --json
# → {"category": "correction", "confidence": 0.88, "all_scores": [...]}

csn classify "sounds good" --set corrections --json
# → {"category": "neutral", "confidence": 0.95, "all_scores": [...]}

# First call downloads the model + trains MLP (~10s). Subsequent calls: ~5ms.
```

## Installation

### Quick install (recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/srobroek/computer-says-no/main/install.sh | bash
```

This installs the `csn` binary via `cargo install` and downloads the default reference sets to the correct platform directory. Requires Rust 1.92+.

### Manual install

```fish
cargo install computer-says-no
```

Then download reference sets (see [Reference set locations](#reference-set-locations) below).

### From GitHub releases

Download a precompiled binary for your platform from [Releases](https://github.com/srobroek/computer-says-no/releases). No build tools needed — single binary, models cached locally on first run.

### From source

```fish
git clone https://github.com/srobroek/computer-says-no.git
cd computer-says-no
cargo build --release
# Binary: target/release/csn — reference sets in reference-sets/ (auto-detected)
```

### Verify

```fish
csn classify "test" --set corrections --json
# Should output JSON with category/confidence/all_scores
```

## Architecture

```
User message → csn classify → daemon (warm, ~5ms) or in-process (cold, ~370ms)
                                  ↓
                          embed text (ONNX, 384-dim)
                                  ↓
               per-category cosine features + char n-grams (256-dim)
                                  ↓
                    MLP classifier (softmax → per-category scores)
                                  ↓
                    winning category + confidence + all scores
```

- **CLI**: `csn classify`, `csn embed`, `csn similarity` — auto-route through background daemon
- **MCP server**: `csn mcp` — stdio transport for Claude Code/Cursor agent tools
- **Daemon**: Unix socket at `~/.cache/computer-says-no/csn.sock` — auto-starts on first CLI call, self-exits after 5 min idle

## Hook setup (Claude Code)

The hook classifies every user message and provides category-tailored feedback to the agent.

### 1. Install csn

```fish
cargo install computer-says-no
```

### 2. Add the hook to your project

Download the hook script from the repository:

```fish
mkdir -p .claude/hooks
curl -o .claude/hooks/user-frustration-check.sh \
  https://raw.githubusercontent.com/srobroek/computer-says-no/main/.claude/hooks/user-frustration-check.sh
chmod +x .claude/hooks/user-frustration-check.sh
```

### 3. Edit the hook paths

Open `.claude/hooks/user-frustration-check.sh` and update the paths. If `csn` is on your PATH:

```bash
CSN="csn"
SETS_DIR="${CSN_SETS_DIR:-}"  # empty = uses default ~/.config/computer-says-no/reference-sets/
```

Or point to a specific install:

```bash
CSN="${CSN_BIN:-$HOME/.cargo/bin/csn}"
SETS_DIR="${CSN_SETS_DIR:-/path/to/reference-sets}"
```

### 4. Register the hook

Add to `.claude/settings.json` (project or global):

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

### 5. Configure threshold (optional)

Default: 80% confidence. Adjust via `CSN_FRUSTRATION_THRESHOLD`:

```fish
set -gx CSN_FRUSTRATION_THRESHOLD 0.75  # more sensitive
set -gx CSN_FRUSTRATION_THRESHOLD 0.90  # less sensitive
```

### What the hook detects

| Category | Examples | Hook behavior |
|----------|----------|---------------|
| **Correction** | "wrong file", "revert that", "not what I asked" | Acknowledge mistake, confirm understanding, adjust |
| **Frustration** | "wtf", "are you kidding me", "I give up" | Reflect on what went wrong, de-escalate, save lesson |
| **Neutral** | "sounds good", "add error handling", "how does this work?" | Hook does not fire |

## Reference sets

Reference sets are TOML files that define classification patterns. `csn` ships with a `corrections` set (1600+ phrases for correction/frustration/neutral detection).

### Reference set locations

`csn` searches for reference sets in this order:

| Priority | Location | When to use |
|----------|----------|-------------|
| 1 | `--sets-dir` CLI flag | One-off testing |
| 2 | `CSN_SETS_DIR` env var | CI, hooks |
| 3 | `sets_dir` in config.toml | Permanent override |
| 4 | Platform config dir (default) | Normal use |
| 5 | Next to the binary | GitHub release downloads |
| 6 | `./reference-sets/` in CWD | Development (source builds) |

**Platform config directories** (via the `directories` crate):

| Platform | Path |
|----------|------|
| macOS | `~/Library/Application Support/computer-says-no/reference-sets/` |
| Linux | `~/.config/computer-says-no/reference-sets/` |
| Windows | `%APPDATA%\computer-says-no\reference-sets\` |

The install script handles this automatically. For manual install:

```fish
# macOS
mkdir -p ~/Library/Application\ Support/computer-says-no/reference-sets
curl -fsSL -o ~/Library/Application\ Support/computer-says-no/reference-sets/corrections.toml \
  https://raw.githubusercontent.com/srobroek/computer-says-no/main/reference-sets/corrections.toml

# Linux
mkdir -p ~/.config/computer-says-no/reference-sets
curl -fsSL -o ~/.config/computer-says-no/reference-sets/corrections.toml \
  https://raw.githubusercontent.com/srobroek/computer-says-no/main/reference-sets/corrections.toml
```

### Creating a reference set

1. Create a `.toml` file in your reference sets directory
2. Classify against it: `csn classify "test" --set my-set --json`
3. The MLP trains automatically on first use and caches weights

### Multi-category format (recommended)

```toml
[metadata]
name = "my-classifier"
description = "Classify text into categories"
mode = "multi-category"
threshold = 0.5

[categories.positive]
phrases = ["example positive 1", "example positive 2"]

[categories.negative]
phrases = ["example negative 1", "example negative 2"]

[categories.neutral]
phrases = ["neutral phrase 1", "neutral phrase 2"]
```

### Binary format (simpler, for two-class problems)

```toml
[metadata]
name = "my-pattern"
mode = "binary"
threshold = 0.5

[phrases]
positive = ["phrases that should match"]
negative = ["phrases that should NOT match"]
```

### Minimum requirements

- Multi-category: 2+ phrases per category, 4+ total
- Binary: 1+ positive phrase (negatives optional but improve accuracy)
- MLP trains automatically on first use and caches weights

## Creating effective reference sets

### Phrase count

Aim for **50+ phrases per category**. The shipped `corrections` set has ~500 per category. More phrases = better accuracy.

### Near-miss negatives

Include phrases that look similar but belong to a different category. This teaches the MLP where the boundary is:

```toml
# Frustration category: emotional outbursts
[categories.frustration]
phrases = ["what the fuck", "are you kidding me"]

# Neutral category: profanity used positively (near-miss!)
[categories.neutral]
phrases = ["holy shit that's amazing", "fuck yeah it works"]
```

### Vocabulary coverage

Cover formal, informal, profane, and abbreviated variants:

```toml
phrases = [
    "that is incorrect",       # formal
    "nah that's off",          # informal
    "wtf",                     # profane abbreviation
    "no",                      # minimal
    "wrong file wrong line",   # compound
]
```

### Category boundaries

Use **intent** as the guide:

| If the phrase is... | Category |
|---------------------|----------|
| Directing a specific change | Correction |
| Expressing emotion/anger/despair | Frustration |
| Sarcastic + frustrated undertone | Frustration |
| Sarcastic + corrective undertone | Correction |
| Praise, agreement, questions, instructions | Neutral |

## Datasets and benchmarking

### Generate scaffold datasets

```fish
csn benchmark generate-datasets
# Creates JSON scaffolds in ~/.config/computer-says-no/datasets/
```

Fill scaffolds with diverse prompts (aim for 500 per dataset). Include easy, medium, and hard tiers:

```json
{
  "name": "corrections",
  "reference_set": "corrections",
  "prompts": [
    {"text": "that's wrong revert it", "expected_match": true, "tier": "easy"},
    {"text": "hmm not quite", "expected_match": true, "tier": "hard"},
    {"text": "sounds good ship it", "expected_match": false, "tier": "easy"}
  ]
}
```

### Run benchmarks

```fish
csn benchmark run                                          # all models + datasets
csn benchmark run --model bge-small-en-v1.5-Q              # specific model
csn benchmark run --json --output results.json             # save results
csn benchmark run --compare old-results.json               # compare runs
csn benchmark compare-strategies --dataset corrections     # strategy comparison
```

### Dataset recommendations

| Use case | Approach |
|----------|----------|
| AI agent hook | Use shipped `corrections` set (1600+ phrases, 3 categories) |
| Code review classification | Multi-category: `bug`, `style`, `security`, `neutral` |
| Sentiment analysis | Binary: `positive` / `negative` |
| Intent detection | Multi-category: one category per intent |
| Spam detection | Binary: `spam` / `ham` |

## MCP server

`csn mcp` exposes 4 tools over stdio:

| Tool | Description |
|------|-------------|
| `classify` | Classify text against a reference set |
| `list_sets` | List sets with categories and phrase counts |
| `embed` | Generate embedding vector |
| `similarity` | Cosine similarity between two texts |

Add to your MCP config:

```json
{
  "mcpServers": {
    "csn": {
      "command": "/path/to/csn",
      "args": ["mcp"]
    }
  }
}
```

## Commands

| Command | Description |
|---------|-------------|
| `csn classify <text> --set <name> --json` | Classify text |
| `csn embed <text>` | Embedding vector |
| `csn similarity <a> <b>` | Cosine similarity |
| `csn mcp` | MCP server (stdio) |
| `csn stop` | Stop daemon |
| `csn models` | List models |
| `csn sets list` | List reference sets |
| `csn benchmark run` | Accuracy benchmark |
| `csn benchmark compare-strategies` | Strategy comparison |
| `csn benchmark generate-datasets` | Dataset scaffolds |

## Configuration

Config file location matches the platform config directory (macOS: `~/Library/Application Support/computer-says-no/config.toml`, Linux: `~/.config/computer-says-no/config.toml`):

```toml
model = "bge-small-en-v1.5-Q"
log_level = "warn"

[mlp]
fallback = false
learning_rate = 0.001
weight_decay = 0.001
max_epochs = 500
patience = 10

[daemon]
idle_timeout = 300
```

Environment overrides: `CSN_MODEL`, `CSN_LOG_LEVEL`, `CSN_SETS_DIR`, `CSN_CACHE_DIR`, `CSN_IDLE_TIMEOUT`, `CSN_MLP_FALLBACK`.

## Performance

| Metric | Value |
|--------|-------|
| Warm classify (daemon) | ~5ms |
| Cold classify (daemon starts) | ~370ms |
| First run (model download + train) | ~10s |
| Binary size (stripped) | ~25MB |

## License

Apache-2.0
