# Research: Character N-gram Features

## Feature Hashing in Rust

**Decision**: Use `std::hash::DefaultHasher` (SipHash) with modulo 256 for bucket assignment. No external crate needed.

**Rationale**: Feature hashing is trivial — hash each n-gram string, take modulo N to get a bucket index, increment the count. SipHash is already in std and is fast enough for <100 n-grams per input. No need for `fxhash` or `ahash` — the hash quality matters more than speed here (collision distribution), and SipHash is well-distributed.

**Alternatives considered**:
- `fxhash`: Faster but non-cryptographic, worse collision distribution. Not worth the dependency for <100 hashes.
- `ahash`: Similar to fxhash. Overkill for this use case.
- MurmurHash: Classic for feature hashing, but no std implementation. SipHash is equivalent for our purposes.

## Character N-gram Extraction

**Decision**: Extract character bigrams and trigrams from lowercased, whitespace-normalized input. Pad start/end with `^` and `$` markers.

**Rationale**: Start/end markers help distinguish "cat" from "concatenate" (different boundary trigrams). Lowercasing normalizes case variants. Whitespace normalization collapses multiple spaces.

**Process**:
1. Lowercase the input
2. Collapse whitespace to single spaces
3. Pad: `^text$`
4. Extract all character bigrams (`^t`, `te`, `ex`, `xt`, `t$`)
5. Extract all character trigrams (`^te`, `tex`, `ext`, `xt$`)
6. Hash each n-gram to a bucket (0..255)
7. Normalize: divide by count to get frequency distribution (L1 norm)

**Why L1 normalize**: Longer texts produce more n-grams. Without normalization, the feature vector magnitude correlates with text length, biasing the MLP toward longer inputs. L1 normalization makes the features length-independent.

## MLP Input Dimension Change

**Decision**: Change `feature_dim = embed_dim + 3 + 256` (was `embed_dim + 3`). MlpConfig already supports configurable `input_dim`.

**Rationale**: The MLP's `input_dim` is already runtime-configurable via `MlpConfig::new().with_input_dim(feature_dim)`. The only changes needed:
1. Compute character features for each input text
2. Concatenate them with the existing embedding + cosine features
3. Update `feature_dim` calculation in training loop
4. Include the feature dim in the cache hash to invalidate old weights

## Cache Invalidation

**Decision**: Include a model version marker ("v2-char256") in the content hash input. This automatically invalidates all cached weights from the pre-character-features model.

**Rationale**: The cached weight files are keyed by content hash. Adding a version marker to the hash input means the hash changes even for the same phrases, forcing a retrain. This is simpler than checking file format versions.

**Alternative**: Check `input_dim` mismatch on load and retrain. More complex, error-prone if the load succeeds with wrong dimensions.

## Unicode Handling

**Decision**: Apply NFC normalization via `unicode-normalization` crate before n-gram extraction.

**Rationale**: Without normalization, "é" (U+00E9) and "e" + combining accent (U+0065 + U+0301) hash to different buckets despite being visually identical. NFC (canonical composition) normalizes these to a single form. The crate is on blessed.rs and lightweight. NFC (not NFKC) preserves semantic distinctions.

**New dependency**: `unicode-normalization = "0.1"`

## Burn Compatibility

**Decision**: No Burn API changes needed. `Linear::new(643, 256)` works the same as `Linear::new(387, 256)` — Burn's linear layers accept any input dimension at config time.

**Rationale**: Verified from the existing codebase: `MlpConfig` has `input_dim: usize` and `LinearConfig::new(self.input_dim, self.hidden1)` uses it at initialization. The change is purely in the config value passed.
