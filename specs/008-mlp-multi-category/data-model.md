# Data Model: MLP Multi-Category Classification

## New Types (in `mlp.rs`)

### MultiCatMlpClassifier<B: Backend>

Multi-category MLP classifier. Same architecture as `MlpClassifier` but output layer size is N (number of categories) instead of 1, and forward pass uses softmax instead of sigmoid.

| Field | Type | Description |
|-------|------|-------------|
| linear1 | `Linear<B>` | Input → hidden1 (input_dim → 256) |
| linear2 | `Linear<B>` | hidden1 → hidden2 (256 → 128) |
| output | `Linear<B>` | hidden2 → N categories (128 → N) |
| activation | `Relu` | ReLU between linear layers |

**Forward pass**: `linear1 → relu → linear2 → relu → output → softmax(dim=1)`

### MultiCatMlpConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| input_dim | `usize` | (computed) | E + N*3 + 256 |
| hidden1 | `usize` | 256 | First hidden layer |
| hidden2 | `usize` | 128 | Second hidden layer |
| num_classes | `usize` | (required) | Number of output categories |

### TrainedMultiCatModel

Analogous to `TrainedModel` for binary. Holds the trained classifier and per-category metadata.

| Field | Type | Description |
|-------|------|-------------|
| reference_set_name | `String` | Name of the multi-category reference set |
| content_hash | `String` | blake3 hash (v3-multicat prefix) for cache key |
| classifier | `MultiCatMlpClassifier<NdArray<f32>>` | Trained classifier on inference backend |
| category_embeddings | `Vec<(String, Vec<Embedding>)>` | Per-category embeddings, alphabetically sorted |
| category_phrases | `Vec<(String, Vec<String>)>` | Per-category phrases, alphabetically sorted |
| category_names | `Vec<String>` | Alphabetically sorted category names (indices match softmax output) |

## Modified Types

### `train_models_at_startup` return type

Currently returns `Vec<TrainedModel>`. Extended to return both:

**Option A** (preferred): Return `(Vec<TrainedModel>, Vec<TrainedMultiCatModel>)` tuple.
**Option B**: Create an enum `TrainedModelKind { Binary(TrainedModel), MultiCategory(TrainedMultiCatModel) }` and return `Vec<TrainedModelKind>`.

Prefer Option A — simpler, follows existing pattern where binary and multi-cat are parallel paths.

### `McpHandler` fields

Add `trained_multi_models: Mutex<Vec<TrainedMultiCatModel>>` alongside existing `trained_models`.

## Existing Types (reused, no changes)

- `MultiCategoryEmbeddings` / `CategoryEmbeddings` in `reference_set.rs` — provides per-category embeddings
- `MultiCategoryResult` / `CategoryScore` in `classifier.rs` — output format for multi-cat classification
- `ClassifyResult::MultiCategory` variant — already exists in the enum

## Per-Category Cosine Features

New function `compute_multi_cosine_features`:

**Input**: text embedding, category embeddings (sorted alphabetically)
**Output**: `Vec<f32>` of length N*3

For each category (alphabetically):
1. `max_sim` = max cosine similarity between text and category's embeddings
2. `mean_sim` = mean cosine similarity between text and category's embeddings
3. `margin` = max_sim - max similarity to any other category's best phrase

## Content Hash

New function `multi_content_hash`:

**Input**: sorted category names, sorted phrases per category
**Output**: blake3 hex string

Format: `blake3(["v3-multicat", cat1_name, cat1_phrase1, ..., cat2_name, cat2_phrase1, ...].join("\n"))`

## corrections.toml Structure (after restructuring)

```toml
[metadata]
name = "corrections"
description = "Detect developer correction, frustration, and neutral signals"
mode = "multi-category"
threshold = 0.5

[categories.correction]
phrases = [
    "wrong file",
    "revert that",
    "not what I asked",
    # ... ~400 directive/instructional phrases
]

[categories.frustration]
phrases = [
    "for the love of god stop",
    "I'm losing patience",
    # ... ~600 emotional/exasperated phrases
]

[categories.neutral]
phrases = [
    "add error handling to the parse function",
    "perfect thank you",
    "how does this work?",
    # ... ~600 current negative phrases
]
```
