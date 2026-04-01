# Research: MLP Classifier (003)

## Decision: ML Framework

**Chosen**: Burn 0.20+ with NdArray backend

**Rationale**: Pure Rust, no native library dependencies, supports autodiff training + inference, well-documented Module/Config pattern. NdArray backend is CPU-only but sufficient for ~1000-sample MLP training.

**Alternatives considered**:
- **linfa**: No MLP/neural network support. Classical ML only. Rejected.
- **Candle** (Hugging Face): Inference-focused, training/autodiff is experimental. Rejected for training use case.
- **tch-rs**: Requires libtorch (~1GB). Rejected — violates constitution principle I (single binary).
- **dfdx**: Low adoption (~3K downloads/month). Rejected — maturity risk.
- **Hand-rolled ndarray**: ~200 LOC inference, but ~500+ LOC for backprop/Adam/early-stopping. Burn provides this out of the box with tested correctness.

## Decision: Backend Configuration

**Chosen**: `Autodiff<NdArray<f32>>` for training, `NdArray<f32>` for inference

**Rationale**: NdArray is pure Rust with optional BLAS acceleration. On macOS, linking Accelerate framework gives near-numpy performance. Training uses `Autodiff` wrapper for backprop; inference strips it via `model.valid()`.

**Alternatives considered**:
- **Wgpu backend**: GPU acceleration overkill for ~1000 samples, adds dependency complexity.
- **NdArray without Accelerate**: Works but 2-5x slower training. Acceptable fallback for Linux CI.

## Decision: Weight Serialization

**Chosen**: Burn's `NamedMpkFileRecorder` (MessagePack binary format)

**Rationale**: Native Burn format, efficient binary serialization, supported by `Module::load_record`. Content-hashed filename (blake3 of reference set phrases) for cache invalidation — reuses existing blake3 dependency.

**Alternatives considered**:
- **JSON weights**: Human-readable but 5-10x larger on disk, slower to load.
- **ONNX export**: Would need ort crate for loading, adds ~5MB. Overkill for internal cache.
- **safetensors**: Hugging Face format, not natively supported by Burn's recorder system.

## Decision: Combined Input Architecture

**Chosen**: Embedding (384-dim) + cosine features (3) = 387-dim input

**Rationale**: Python research (spec 002) showed:
- MLP on embeddings only: 89.5% ± 1.2%
- MLP on cosine features only (3-dim): 94.1%
- MLP on combined (387-dim): 96.2%

The cosine features (max positive similarity, max negative similarity, margin) carry strong signal. Combining with raw embeddings gives the best results.

## Decision: Training Hyperparameters

**Chosen**: Adam optimizer, L2 regularization (weight decay 0.001), early stopping (patience 10 epochs), max 500 epochs, batch size = full dataset (single batch for small data)

**Rationale**: Matches the sklearn MLPClassifier defaults that produced 89-96% accuracy in Python prototypes. Full-batch training is appropriate for ~20-200 samples (no mini-batching needed).

## Decision: Training Failure Policy

**Chosen**: Error by default (daemon refuses to start), configurable override to fall back to pure cosine

**Rationale**: Training failure indicates a data quality problem (too few phrases, degenerate embeddings). Failing loudly prevents serving bad classifications. Config override (`mlp_fallback = true`) allows degraded operation when needed.

## Burn API Patterns (from context7)

### Model Definition
```rust
#[derive(Module, Debug)]
pub struct MlpClassifier<B: Backend> {
    linear1: Linear<B>,
    linear2: Linear<B>,
    output: Linear<B>,
    activation: Relu,
}

#[derive(Config, Debug)]
pub struct MlpConfig {
    input_dim: usize,   // 387 (384 embedding + 3 cosine features)
    hidden1: usize,     // 256
    hidden2: usize,     // 128
}
```

### Training Loop
```rust
type TrainBackend = Autodiff<NdArray<f32>>;
// Forward → loss → loss.backward() → GradientsParams::from_grads → optim.step(lr, model, grads)
// model.valid() strips Autodiff for inference
```

### Weight Persistence
```rust
// Save: NamedMpkFileRecorder::default().record(model.into_record(), path)
// Load: recorder.load(path, &device) → model.load_record(record)
```
