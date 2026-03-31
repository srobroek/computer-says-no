# Data Model: Core Binary

## Entities

### EmbeddingModel (value type)

A supported ONNX model identified by name.

| Attribute | Type | Description |
|-----------|------|-------------|
| name | string | Human-readable identifier (e.g., "bge-small-en-v1.5-Q") |
| dimensions | integer | Output vector size (384, 768, 1024) |

Fixed set of variants — not user-extensible.

### ReferenceSet

A collection of labeled phrases defining a classification target.

| Attribute | Type | Description |
|-----------|------|-------------|
| name | string | Unique identifier (derived from filename) |
| description | string (optional) | Human-readable purpose |
| mode | enum: binary, multi-category | Classification mode |
| threshold | float | Minimum similarity score for a match |
| source | URL (optional) | Remote source for auto-update (future spec) |
| content_hash | string | blake3 hash of file content — used for cache invalidation |
| source_path | path | Filesystem location of the TOML file |

**Binary mode phrases**:
- positive: list of strings (match targets)
- negative: list of strings (anti-targets, optional)

**Multi-category mode**:
- categories: map of category_name → list of phrases

### ClassificationResult

Output of comparing input text against a reference set.

**Binary result**:
| Field | Type | Description |
|-------|------|-------------|
| match | boolean | Whether positive score exceeds threshold and negative |
| confidence | float | Highest positive similarity score |
| top_phrase | string | Best-matching positive phrase |
| scores.positive | float | Best positive score |
| scores.negative | float | Best negative score |

**Multi-category result**:
| Field | Type | Description |
|-------|------|-------------|
| match | boolean | Whether best category score exceeds threshold |
| category | string | Best-matching category name |
| confidence | float | Best category score |
| top_phrase | string | Best-matching phrase in the best category |
| all_scores | list | All categories ranked by score |

### AppConfig

Merged configuration from all sources.

| Field | Type | Default | Env var |
|-------|------|---------|---------|
| port | integer | 9847 | CSN_PORT |
| model | string | nomic-embed-text-v1.5-Q | CSN_MODEL |
| log_level | string | warn | CSN_LOG_LEVEL |
| sets_dir | path | ~/.config/computer-says-no/reference-sets/ | CSN_SETS_DIR |
| config_dir | path | ~/.config/computer-says-no/ | — |
| cache_dir | path | ~/.cache/computer-says-no/ | CSN_CACHE_DIR |

## Relationships

```
EmbeddingModel  ←used-by─  EmbeddingEngine  ←used-by─  Classifier
                                                            │
ReferenceSet  ──embedded-by→  EmbeddingEngine           ←uses─┘
     │
     └──watched-by→  FileWatcher  ──triggers→  reload(ReferenceSet[])

AppConfig  ──configures→  EmbeddingEngine, Server, FileWatcher
```

## State Transitions

### ReferenceSet lifecycle
```
File Created → Detected by Watcher → Parsed → Embedded → Available for Classification
File Modified → Detected by Watcher → Re-parsed → Re-embedded → Atomically Swapped
File Deleted → Detected by Watcher → Removed from Available Sets
File Invalid → Detected by Watcher → Skipped with Warning → Other Sets Unaffected
```

### Daemon lifecycle
```
Start → Load Config → Load Model → Load & Embed Sets → Start Watcher → Bind Port → Ready
Ready → Serve Requests (concurrent reads, serialized embeds via Mutex)
Ready → Watcher Event → Re-embed in Background → Atomic Swap Sets
SIGTERM/SIGINT → Graceful Shutdown → Stop Accepting → Drain Active → Exit
```
