# Research: MLP Multi-Category Classification

## Burn Framework: Multi-Category Support

### Decision: Use `CrossEntropyLossConfig` + `activation::softmax` from burn 0.20

**Rationale**: Both APIs exist in the burn 0.20.1 crate already in use. No new dependencies needed.

**Alternatives considered**:
- Manual softmax + NLL loss: More error-prone, no advantage over burn's built-in
- One-vs-rest binary classifiers: Simpler per-model but N models instead of 1, slower training/inference

### API Details (verified from burn-nn-0.20.1 and burn-tensor-0.20.1 source)

**CrossEntropyLoss** (`burn::nn::loss::CrossEntropyLossConfig`):
- `forward(logits: Tensor<B, 2>, targets: Tensor<B, 1, Int>) -> Tensor<B, 1>`
- Logits shape: `[batch_size, num_classes]` — raw output, NOT softmax'd (defaults to `logits: true`, applies `log_softmax` internally)
- Targets shape: `[batch_size]` — class indices as `i64`, NOT one-hot
- Supports optional class weights (`with_weights(Some(vec![...]))`) for imbalanced classes
- Supports label smoothing (`with_smoothing(Some(alpha))`)

**Softmax activation** (`burn::tensor::activation::softmax`):
- `softmax<const D: usize, B: Backend>(tensor: Tensor<B, D>, dim: usize) -> Tensor<B, D>`
- For inference: apply softmax on dim=1 to get probability distribution over classes
- For training: NOT needed — CrossEntropyLoss applies log_softmax internally

### Key Differences from Binary MLP Training

| Aspect | Binary (current) | Multi-category (new) |
|--------|-----------------|---------------------|
| Output layer | `LinearConfig::new(hidden2, 1)` | `LinearConfig::new(hidden2, N)` |
| Activation | `activation::sigmoid(output)` | `activation::softmax(output, 1)` |
| Loss | `BinaryCrossEntropyLossConfig` | `CrossEntropyLossConfig` |
| Loss input | Squeezed `(batch,)` sigmoid probs | Raw logits `(batch, N)` — CE applies log_softmax |
| Labels | `Tensor<B, 1, Int>` with 0/1 | `Tensor<B, 1, Int>` with class indices 0..N-1 |
| Inference output | Single f32 probability | N-dim probability vector |

### Training Loop Changes

1. **Forward pass**: Output is `(batch, N)` not `(batch, 1)`. No squeeze needed.
2. **Loss computation**: `CrossEntropyLossConfig::new().init(&device)` — pass raw logits (not softmax'd) + integer targets.
3. **Labels**: Map category names to indices (alphabetically sorted). Each phrase gets its category's index.
4. **Class weights**: Optional. Use `with_weights(Some(weights))` where weights are inversely proportional to class frequency to handle imbalance. Consider this if macro-F1 is below 80%.
5. **Inference**: Apply `activation::softmax(output, 1)` to get probability distribution. The highest probability determines the winning category.

## Per-Category Cosine Features

### Decision: Compute [max, mean, margin] per category

**Rationale**: This gives the MLP 3 informative features per category: how close the input is to the best phrase in each category (max), the average distance to the category (mean), and how much better this category is vs the next-best (margin). This is analogous to the binary `[max_pos, max_neg, margin]` but generalized.

**Alternatives considered**:
- Max-only per category (N features): Less information, loses distributional signal
- Shared 3 features (max across all, min across all, spread): Loses per-category detail
- Pairwise margins (N*(N-1)/2 features): Quadratic growth, diminishing returns

### Input Dimension Formula

For N categories with embedding dimension E:
- Embedding: E (384 for bge-small)
- Per-category cosine: N * 3
- Char n-grams: 256
- **Total**: E + N*3 + 256

For 3 categories: 384 + 9 + 256 = **649**
For 4 categories (if sarcasm retained): 384 + 12 + 256 = **652**

## Corrections.toml Restructuring

### Decision: 3 guaranteed categories (correction, frustration, neutral), sarcasm reviewed per-phrase

**Curation approach**:
1. Current positive phrases (~1000) are split into correction vs frustration based on intent:
   - **Correction** (directive): "wrong file", "revert that", "not what I asked", "the logic is wrong"
   - **Frustration** (emotional): "for the love of god stop", "I'm losing patience", "wtf", "kill me"
   - Phrases with sarcastic tone: evaluate individually — "thanks for nothing" → frustration, "wrong file" → correction
2. Current negative phrases (~600) all become **neutral**: praise, instructions, questions, confirmations
3. Expect approximate distribution: correction ~400, frustration ~600, neutral ~600

### Content Hash Versioning

**Decision**: Use "v3-multicat" prefix for multi-category MLP weight hashes

Hash format: `blake3(["v3-multicat", sorted_category_names..., sorted_phrases_per_category...].join("\n"))`

This ensures:
- No collision with binary "v2-char256" hashes
- Cache invalidation when categories change
- Cache invalidation when phrases within a category change
- Deterministic ordering via sorting
