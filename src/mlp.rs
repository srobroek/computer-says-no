// TODO: remove allow(dead_code) once mlp module is wired into the classifier
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use burn::backend::{Autodiff, NdArray};
use burn::module::AutodiffModule;
use burn::nn::loss::BinaryCrossEntropyLossConfig;
use burn::nn::{Linear, LinearConfig, Relu};
use burn::optim::{AdamConfig, GradientsParams, Optimizer};
use burn::prelude::*;
use burn::record::{FullPrecisionSettings, NamedMpkFileRecorder};
use burn::tensor::activation;

use crate::model::{Embedding, cosine_similarity};

/// MLP binary classifier for embedding-based text classification.
///
/// Three-layer perceptron: input (387) -> hidden1 (256) -> hidden2 (128) -> output (1).
/// Input dimension is 384 (embedding) + 3 (cosine similarity features).
#[derive(Module, Debug)]
pub struct MlpClassifier<B: Backend> {
    linear1: Linear<B>,
    linear2: Linear<B>,
    output: Linear<B>,
    activation: Relu,
}

impl<B: Backend> MlpClassifier<B> {
    /// Run the forward pass: linear1 -> relu -> linear2 -> relu -> output -> sigmoid.
    ///
    /// Accepts a batch of feature vectors with shape `(batch, 387)` and returns
    /// sigmoid probabilities with shape `(batch, 1)`.
    pub fn forward(&self, input: Tensor<B, 2>) -> Tensor<B, 2> {
        let x = self.activation.forward(self.linear1.forward(input));
        let x = self.activation.forward(self.linear2.forward(x));
        activation::sigmoid(self.output.forward(x))
    }
}

/// Configuration for creating an [`MlpClassifier`].
#[derive(Config, Debug)]
pub struct MlpConfig {
    /// Input feature dimension (384 embedding + 3 cosine features).
    #[config(default = 387)]
    input_dim: usize,
    /// First hidden layer size.
    #[config(default = 256)]
    hidden1: usize,
    /// Second hidden layer size.
    #[config(default = 128)]
    hidden2: usize,
}

impl MlpConfig {
    /// Initialize an [`MlpClassifier`] from this configuration.
    pub fn init<B: Backend>(&self, device: &B::Device) -> MlpClassifier<B> {
        MlpClassifier {
            linear1: LinearConfig::new(self.input_dim, self.hidden1).init(device),
            linear2: LinearConfig::new(self.hidden1, self.hidden2).init(device),
            output: LinearConfig::new(self.hidden2, 1).init(device),
            activation: Relu::new(),
        }
    }
}

/// Compute cosine similarity features for MLP input.
///
/// Given a text embedding and sets of positive/negative reference embeddings,
/// returns `[max_pos, max_neg, margin]` where:
/// - `max_pos` is the maximum cosine similarity to any positive embedding,
/// - `max_neg` is the maximum cosine similarity to any negative embedding,
/// - `margin` is `max_pos - max_neg`.
///
/// If either embedding set is empty, its max defaults to `0.0`.
pub fn compute_cosine_features(
    text_emb: &Embedding,
    pos_embeddings: &[Embedding],
    neg_embeddings: &[Embedding],
) -> [f32; 3] {
    let max_pos = pos_embeddings
        .iter()
        .map(|e| cosine_similarity(text_emb, e))
        .fold(f32::NEG_INFINITY, f32::max);
    let max_pos = if max_pos == f32::NEG_INFINITY {
        0.0
    } else {
        max_pos
    };

    let max_neg = neg_embeddings
        .iter()
        .map(|e| cosine_similarity(text_emb, e))
        .fold(f32::NEG_INFINITY, f32::max);
    let max_neg = if max_neg == f32::NEG_INFINITY {
        0.0
    } else {
        max_neg
    };

    [max_pos, max_neg, max_pos - max_neg]
}

/// Train an MLP binary classifier from positive and negative phrase embeddings.
///
/// Builds training data by computing cosine features (max_pos, max_neg, margin)
/// for each sample and concatenating with the raw embedding to form 387-dim input
/// vectors. Uses `Autodiff<NdArray<f32>>` for training with Adam optimizer, BCE
/// loss, and early stopping.
///
/// Returns the trained model on the inference backend (`NdArray<f32>`) via
/// `model.valid()`, which strips the autodiff wrapper.
pub fn train_mlp(
    pos_embeddings: &[Embedding],
    neg_embeddings: &[Embedding],
    learning_rate: f64,
    weight_decay: f64,
    max_epochs: usize,
    patience: usize,
) -> anyhow::Result<MlpClassifier<NdArray<f32>>> {
    type TrainBackend = Autodiff<NdArray<f32>>;

    let device = <TrainBackend as Backend>::Device::default();
    let config = MlpConfig::new();
    let mut model: MlpClassifier<TrainBackend> = config.init(&device);

    // --- Build training data ---
    let total_samples = pos_embeddings.len() + neg_embeddings.len();
    if total_samples == 0 {
        anyhow::bail!("no training samples provided");
    }

    let embed_dim = pos_embeddings
        .first()
        .or(neg_embeddings.first())
        .map(|e| e.len())
        .unwrap_or(384);
    let feature_dim = embed_dim + 3; // embedding + [max_pos, max_neg, margin]

    let mut features: Vec<f32> = Vec::with_capacity(total_samples * feature_dim);
    let mut labels: Vec<i64> = Vec::with_capacity(total_samples);

    // Positive samples: exclude self from positive set to avoid data leakage.
    for (i, emb) in pos_embeddings.iter().enumerate() {
        let others_pos: Vec<Embedding> = pos_embeddings
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, e)| e.clone())
            .collect();
        let cosine = compute_cosine_features(emb, &others_pos, neg_embeddings);
        features.extend_from_slice(emb);
        features.extend_from_slice(&cosine);
        labels.push(1);
    }

    // Negative samples: exclude self from negative set.
    for (i, emb) in neg_embeddings.iter().enumerate() {
        let others_neg: Vec<Embedding> = neg_embeddings
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, e)| e.clone())
            .collect();
        let cosine = compute_cosine_features(emb, pos_embeddings, &others_neg);
        features.extend_from_slice(emb);
        features.extend_from_slice(&cosine);
        labels.push(0);
    }

    // Create Burn tensors.
    let x = Tensor::<TrainBackend, 2>::from_floats(
        TensorData::new(features, [total_samples, feature_dim]),
        &device,
    );
    let y = Tensor::<TrainBackend, 1, Int>::from_ints(
        TensorData::new(labels, [total_samples]),
        &device,
    );

    // --- Configure optimizer and loss ---
    let optim_config = AdamConfig::new().with_weight_decay(Some(
        burn::optim::decay::WeightDecayConfig::new(weight_decay as f32),
    ));
    let mut optim = optim_config.init();
    let bce = BinaryCrossEntropyLossConfig::new().init(&device);

    // --- Training loop with early stopping ---
    let mut best_loss = f64::INFINITY;
    let mut epochs_without_improvement = 0_usize;

    for epoch in 0..max_epochs {
        // Forward pass.
        let output = model.forward(x.clone()); // (batch, 1)
        let output_flat = output.squeeze::<1>(); // (batch,)

        // Compute loss.
        let loss = bce.forward(output_flat.clone(), y.clone());
        let loss_scalar: f64 = loss.clone().into_scalar().elem();

        // Check for NaN.
        if loss_scalar.is_nan() {
            anyhow::bail!(
                "training diverged: loss is NaN at epoch {epoch}"
            );
        }

        // Early stopping check.
        if loss_scalar < best_loss - 1e-6 {
            best_loss = loss_scalar;
            epochs_without_improvement = 0;
        } else {
            epochs_without_improvement += 1;
        }

        if epochs_without_improvement >= patience {
            break;
        }

        // Backward pass and optimizer step.
        let grads = loss.backward();
        let grads = GradientsParams::from_grads(grads, &model);
        model = optim.step(learning_rate, model, grads);
    }

    Ok(model.valid())
}

/// Compute a blake3 content hash from reference set phrases.
///
/// Sorts all positive and negative phrases, concatenates them with newline
/// separators (positives first, then negatives), and returns the blake3 hex digest.
pub fn content_hash(positive_phrases: &[String], negative_phrases: &[String]) -> String {
    let mut positives: Vec<&str> = positive_phrases.iter().map(String::as_str).collect();
    positives.sort();
    let mut negatives: Vec<&str> = negative_phrases.iter().map(String::as_str).collect();
    negatives.sort();

    let combined: String = positives
        .into_iter()
        .chain(negatives)
        .collect::<Vec<&str>>()
        .join("\n");

    blake3::hash(combined.as_bytes()).to_hex().to_string()
}

/// Resolve the `.mpk` cache path for a given content hash.
///
/// Returns `{cache_dir}/mlp/{hash}.mpk`.
pub fn cache_path(cache_dir: &Path, hash: &str) -> PathBuf {
    cache_dir.join("mlp").join(format!("{hash}.mpk"))
}

/// Save MLP model weights to disk using the Burn `NamedMpkFileRecorder`.
///
/// The recorder auto-appends the `.mpk` extension, so `path` should include
/// the full path **with** the `.mpk` suffix already stripped (the recorder
/// adds it). To keep things simple we strip `.mpk` if present before passing
/// to the recorder.
pub fn save_weights<B: Backend>(model: MlpClassifier<B>, path: &Path) -> anyhow::Result<()> {
    let recorder = NamedMpkFileRecorder::<FullPrecisionSettings>::new();
    let base = path.with_extension("");
    model
        .save_file(base, &recorder)
        .map_err(|e| anyhow::anyhow!("failed to save MLP weights: {e}"))?;
    Ok(())
}

/// Load MLP model weights from disk using the Burn `NamedMpkFileRecorder`.
///
/// Initializes a fresh model from `config`, then loads the saved record into it.
/// As with [`save_weights`], the `.mpk` extension is handled by the recorder.
pub fn load_weights<B: Backend>(
    config: &MlpConfig,
    path: &Path,
    device: &B::Device,
) -> anyhow::Result<MlpClassifier<B>> {
    let recorder = NamedMpkFileRecorder::<FullPrecisionSettings>::new();
    let base = path.with_extension("");
    let model = config.init::<B>(device);
    let model = model
        .load_file(base, &recorder, device)
        .map_err(|e| anyhow::anyhow!("failed to load MLP weights: {e}"))?;
    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;
    use burn::backend::NdArray;

    type TestBackend = NdArray;

    #[test]
    fn config_default_values() {
        let config = MlpConfig::new();
        assert_eq!(config.input_dim, 387);
        assert_eq!(config.hidden1, 256);
        assert_eq!(config.hidden2, 128);
    }

    #[test]
    fn init_creates_model() {
        let config = MlpConfig::new();
        let device = <TestBackend as Backend>::Device::default();
        let model = config.init::<TestBackend>(&device);

        // Verify layer dimensions via weight shapes.
        assert_eq!(model.linear1.weight.dims(), [387, 256]);
        assert_eq!(model.linear2.weight.dims(), [256, 128]);
        assert_eq!(model.output.weight.dims(), [128, 1]);
    }
}
