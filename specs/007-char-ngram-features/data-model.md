# Data Model: Character N-gram Features

## Entities

### CharNgramFeatures

A fixed-size (256-dim) feature vector derived from character bigrams and trigrams of input text.

| Attribute | Type | Description |
|-----------|------|-------------|
| vector | [f32; 256] | L1-normalized frequency counts of hashed character n-grams |

### Computation Pipeline

```
Input text
  → lowercase + whitespace normalize
  → pad with ^ and $
  → extract bigrams + trigrams
  → hash each to bucket (0..255) via SipHash % 256
  → count occurrences per bucket
  → L1 normalize (divide by total count)
  → 256-dim f32 vector
```

### Hybrid MLP Input Vector

| Segment | Dimensions | Source |
|---------|-----------|--------|
| Embedding | 384 | fastembed ONNX model |
| Cosine features | 3 | max_pos, max_neg, margin |
| Character n-grams | 256 | Feature hashing of bigrams + trigrams |
| **Total** | **643** | Concatenated, fed to MLP |

### Cache Key Change

The MLP weight cache key changes from:

```
blake3(sorted_phrases)
```

to:

```
blake3("v2-char256" + sorted_phrases)
```

This invalidates all existing cached weights, forcing retrain with the new input dimension.
