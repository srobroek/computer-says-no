# Computer Says No (`csn`)

Classify text in under 5ms — locally, with no LLM calls. A single binary that runs on every user message.

## Why csn

| Problem | csn's approach |
|---------|---------------|
| LLM-based classification costs $0.01+ per call and adds 500ms+ latency | Local ONNX embedding + MLP: **~5ms**, zero API cost |
| Cloud classifiers need network access and API keys | Single binary, runs offline, no accounts |
| Regex/keyword matching can't handle typos, sarcasm, or paraphrasing | Embedding similarity + neural network: handles "wtf", "what the fuck", and "what in the actual fuck" identically |
| Setting up ML pipelines requires Python, pip, model servers | `cargo install computer-says-no` — one command, one binary, no runtime deps |

**Built for AI coding agent hooks**: Classify every user message for frustration, corrections, or any custom pattern. Works with any agent that supports hooks — Claude Code, Cursor, Windsurf, Codex, or your own tooling.

## What it does

`csn` embeds text using ONNX models (via fastembed) and classifies it against reference sets of example phrases. An MLP neural network with character n-gram features provides typo-robust classification with per-category confidence scores.

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

## Hook setup

The hook classifies every user message and provides category-tailored feedback to the agent. Works with any coding agent that supports shell hooks.

### 1. Install csn

```bash
curl -fsSL https://raw.githubusercontent.com/srobroek/computer-says-no/main/install.sh | bash
```

### 2. Integration

The core pattern: pipe the user message through `csn classify --json`, check the category, and inject feedback into the agent's context.

<details>
<summary><strong>Claude Code</strong></summary>

Download the hook and register it:

```bash
mkdir -p .claude/hooks
curl -fsSL -o .claude/hooks/user-frustration-check.sh \
  https://raw.githubusercontent.com/srobroek/computer-says-no/main/.claude/hooks/user-frustration-check.sh
chmod +x .claude/hooks/user-frustration-check.sh
```

Add to `.claude/settings.json`:

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
</details>

<details>
<summary><strong>Cursor</strong></summary>

Add a rule in `.cursorrules` that invokes csn:

```
Before responding to user messages, classify the input:
Run: csn classify "<user_message>" --set corrections --json
If category is "correction": acknowledge the mistake and adjust.
If category is "frustration": reflect on what went wrong, de-escalate.
```

Or use csn as an MCP server in Cursor's MCP config for tool-based classification.
</details>

<details>
<summary><strong>Any agent with shell hooks</strong></summary>

The basic pattern for any hook system:

```bash
#!/usr/bin/env bash
USER_MESSAGE="$1"
RESULT=$(csn classify "$USER_MESSAGE" --set corrections --json 2>/dev/null)
CATEGORY=$(echo "$RESULT" | jq -r '.category // empty')
CONFIDENCE=$(echo "$RESULT" | jq -r '.confidence')

case "$CATEGORY" in
  correction) echo "User is correcting you. Acknowledge and fix." ;;
  frustration) echo "User is frustrated. Reflect on what went wrong." ;;
  *) ;; # neutral — no action
esac
```

Adapt the output format to your agent's hook protocol.
</details>

### 3. Configure threshold (optional)

Default: 80% confidence. Adjust via `CSN_FRUSTRATION_THRESHOLD`:

```bash
export CSN_FRUSTRATION_THRESHOLD=0.60  # more sensitive (recommended for multi-category)
export CSN_FRUSTRATION_THRESHOLD=0.90  # less sensitive
```

### What the hook detects

| Category | Examples | Suggested behavior |
|----------|----------|--------------------|
| **Correction** | "wrong file", "revert that", "not what I asked" | Acknowledge mistake, confirm understanding, adjust |
| **Frustration** | "wtf", "are you kidding me", "I give up" | Reflect on what went wrong, de-escalate, save lesson |
| **Neutral** | "sounds good", "add error handling", "how does this work?" | No action needed |

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

### Dataset format

Datasets are JSON files in `datasets/` with labeled prompts:

```json
{
  "name": "corrections",
  "reference_set": "corrections",
  "mode": "multi-category",
  "prompts": [
    {"text": "wrong file, use auth.rs", "expected_label": "correction", "tier": "clear", "polarity": "positive"},
    {"text": "are you kidding me", "expected_label": "frustration", "tier": "clear", "polarity": "positive"},
    {"text": "looks good ship it", "expected_label": "neutral", "tier": "clear", "polarity": "positive"},
    {"text": "hmm not quite", "expected_label": "correction", "tier": "edge", "polarity": "positive"}
  ]
}
```

For multi-category sets, `expected_label` is the category name. Tiers: `clear` (obvious), `moderate` (requires context), `edge` (ambiguous).

### Generate scaffolds

```fish
csn benchmark generate-datasets
# Creates JSON scaffolds from your reference sets
```

### Run benchmarks

```fish
csn benchmark run                                          # all models + datasets
csn benchmark run --model bge-small-en-v1.5-Q              # specific model
csn benchmark run --json --output results.json             # save results
csn benchmark run --compare old-results.json               # compare runs
csn benchmark compare-strategies --dataset corrections     # strategy comparison
```

### Current results

```
corrections (62 prompts, 3 categories)
+---------------------+----------+----------+----------+
| Model               | Accuracy | Edge Acc | p50 (ms) |
+=======================================================+
| bge-small-en-v1.5-Q | 82.3%    | 50.0%    | 14.8     |
+---------------------+----------+----------+----------+
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

Add to your agent's MCP config (Claude Code, Cursor, or any MCP-compatible client):

```json
{
  "mcpServers": {
    "csn": {
      "command": "csn",
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
