# Quickstart: MLP Classifier (003)

## What Changed

The `/classify` endpoint now uses a combined pipeline (embedding + cosine features → MLP neural network) for binary reference sets, improving accuracy from ~80% to 89-96%.

## For Users

Nothing changes. The API request/response shape is identical. The MLP trains automatically at first startup and caches weights for fast restarts.

## For Developers

### New files
- `src/mlp.rs` — MLP model definition, training loop, weight cache
- `src/mlp.rs` integrates with `src/classifier.rs` for the combined pipeline

### New dependency
- `burn` + `burn-ndarray` in `Cargo.toml`

### Build
```fish
just build  # cargo build (burn compiles ~20-30s first time)
just test   # includes MLP unit tests
```

### Verify
```fish
just serve
# In another terminal:
curl -s localhost:9847/classify -d '{"text": "no that is wrong", "reference_set": "corrections"}' | jq
# confidence should now be MLP probability (0.0-1.0)
```

### Config override
To fall back to pure cosine if MLP training fails:
```toml
# ~/.config/computer-says-no/config.toml
[mlp]
fallback = true
```
