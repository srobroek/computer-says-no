# Contract: Configuration (MLP additions)

## Config File (`~/.config/computer-says-no/config.toml`)

New optional section:

```toml
[mlp]
# Fall back to pure cosine if MLP training fails (default: false = error on failure)
fallback = false

# Training hyperparameters (defaults are tuned for 20-200 phrase reference sets)
# learning_rate = 0.001
# weight_decay = 0.001
# max_epochs = 500
# patience = 10
```

## Server Configuration (added in spec 003)

```toml
# Bind address for the daemon (default: 127.0.0.1)
host = "127.0.0.1"
```

## Environment Variable Overrides

| Variable | Default | Description |
|----------|---------|-------------|
| `CSN_HOST` | `127.0.0.1` | Server bind address |
| `CSN_MLP_FALLBACK` | `false` | Fall back to cosine on training failure |

## Cache Layout

```
~/.cache/computer-says-no/
├── {model-name}/          # existing embedding cache
└── mlp/
    └── {content_hash}.mpk  # trained MLP weights (MessagePack)
```

`content_hash` = blake3 hash of sorted, concatenated reference set phrases (positive + negative).
