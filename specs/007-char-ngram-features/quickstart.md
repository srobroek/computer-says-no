# Quickstart: Character N-gram Features (007)

## What Changed

The MLP classifier now uses character-level n-gram features alongside embeddings. This makes classification robust to typos — "wwtf" now matches as well as "wtf".

## For Users

Nothing changes. Same commands, same output. Typo-prone inputs just classify more accurately now.

The first run after upgrading will take a few extra seconds as MLP weights retrain with the new feature dimension. Subsequent runs use cached weights.

## For Developers

### Modified files
- `src/mlp.rs` — character n-gram feature extraction, updated training loop, cache version bump
- `src/classifier.rs` — pass character features through the classification pipeline

### How it works
1. Input text is lowercased, padded with `^`/`$` markers
2. Character bigrams and trigrams are extracted
3. Each n-gram is hashed to one of 256 buckets (SipHash % 256)
4. Bucket counts are L1-normalized to a frequency vector
5. The 256-dim vector is concatenated with embedding (384) + cosine (3) features
6. MLP input dimension: 643 (was 387)

### Verify
```fish
csn classify "wwtf" --set corrections --json
# Should match with confidence > 50% (previously failed)

csn classify "thsi is brokne" --set corrections --json
# Should match (previously failed on typos)

csn classify "this is broken" --set corrections --json
# Should still match with same or better confidence (no regression)
```
