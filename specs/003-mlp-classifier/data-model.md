# Data Model: MLP Classifier (003)

## Entities

### MlpClassifier

Trained 2-layer MLP neural network for binary classification.

| Field | Type | Description |
|-------|------|-------------|
| linear1 | Linear layer | 387 → 256 (input → hidden 1) |
| linear2 | Linear layer | 256 → 128 (hidden 1 → hidden 2) |
| output | Linear layer | 128 → 1 (hidden 2 → sigmoid output) |
| activation | Relu | Shared activation function |

**Identity**: One MlpClassifier per binary reference set.
**Lifecycle**: Untrained → Training → Trained → (on set change) → Retraining.

### MlpConfig

Hyperparameters for training. Stored as Burn `Config` (serializable).

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| input_dim | usize | 387 | Embedding dim + 3 cosine features |
| hidden1 | usize | 256 | First hidden layer size |
| hidden2 | usize | 128 | Second hidden layer size |
| learning_rate | f64 | 0.001 | Adam learning rate |
| weight_decay | f64 | 0.001 | L2 regularization strength |
| max_epochs | usize | 500 | Maximum training epochs |
| patience | usize | 10 | Early stopping patience |

### TrainedModel

Runtime container associating a trained MLP with its reference set.

| Field | Type | Description |
|-------|------|-------------|
| reference_set_name | String | Name of the binary reference set |
| content_hash | String | blake3 hash of reference set phrases (cache key) |
| classifier | MlpClassifier | The trained model (inference mode) |
| pos_embeddings | Vec<Embedding> | Cached positive phrase embeddings (for cosine features) |
| neg_embeddings | Vec<Embedding> | Cached negative phrase embeddings (for cosine features) |

### WeightCache

Disk-persisted model weights for fast restart.

| Aspect | Detail |
|--------|--------|
| Format | Burn NamedMpkFileRecorder (MessagePack binary) |
| Location | `~/.cache/computer-says-no/mlp/{content_hash}.mpk` |
| Key | blake3 hash of sorted reference set phrases |
| Invalidation | Content hash mismatch → retrain |
| Corruption | Log warning, retrain from scratch |

## Relationships

```
AppConfig 1──* ReferenceSet
ReferenceSet(binary) 1──0..1 TrainedModel
TrainedModel 1──1 MlpClassifier
TrainedModel 1──1 WeightCache (on disk)
```

## State Transitions

```
Startup:
  ReferenceSet loaded
    → has negatives AND ≥4 phrases?
      → YES: check WeightCache
        → cache hit (hash match): load weights → Trained
        → cache miss: train from phrases → save weights → Trained
        → training fails to converge:
          → mlp_fallback=false (default): ERROR, daemon refuses to start
          → mlp_fallback=true: log warning → Pure Cosine fallback
      → NO: skip MLP → Pure Cosine only

Hot-reload:
  File watcher detects set change
    → re-embed phrases, recompute content hash
    → retrain MLP in background
    → swap model atomically (old model serves until new one ready)
```

## Classification Flow (Combined Pipeline)

```
Input text
  → embed(text) → 384-dim embedding
  → compute cosine features:
    max_pos = max(cosine(text_emb, pos_emb_i))
    max_neg = max(cosine(text_emb, neg_emb_i))
    margin = max_pos - max_neg
  → concatenate: [embedding(384), max_pos, max_neg, margin] = 387-dim
  → MLP forward: 387 → 256 (ReLU) → 128 (ReLU) → 1 (sigmoid)
  → confidence = sigmoid output
  → is_match = confidence > 0.5
  → response: { is_match, confidence (MLP), top_phrase, scores: { positive: max_pos, negative: max_neg } }
```
