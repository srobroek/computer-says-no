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
use crate::reference_set::{ReferenceSet, ReferenceSetKind};

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

/// A trained MLP model ready for inference.
///
/// Holds the classifier (pinned to the `NdArray<f32>` backend), the originating
/// reference set name and content hash (used as a cache key), and cloned
/// positive/negative embeddings needed to compute cosine features at inference time.
pub struct TrainedModel {
    /// Name of the binary reference set this model was trained on.
    pub reference_set_name: String,
    /// blake3 hash of the reference set phrases (cache key for weight files).
    pub content_hash: String,
    /// The trained MLP classifier using the NdArray inference backend.
    pub classifier: MlpClassifier<NdArray<f32>>,
    /// Cached positive phrase embeddings for cosine feature computation.
    pub pos_embeddings: Vec<Embedding>,
    /// Cached negative phrase embeddings for cosine feature computation.
    pub neg_embeddings: Vec<Embedding>,
    /// Positive phrase strings for top_phrase reporting during classification.
    pub pos_phrases: Vec<String>,
}

/// Configuration for creating an [`MlpClassifier`].
#[derive(Config, Debug)]
pub struct MlpConfig {
    /// Input feature dimension (embedding dim + 3 cosine features).
    #[config(default = 387)]
    pub input_dim: usize,
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

    // Configure MLP with actual feature dimension (varies by embedding model).
    let config = MlpConfig::new().with_input_dim(feature_dim);
    let mut model: MlpClassifier<TrainBackend> = config.init(&device);

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
            anyhow::bail!("training diverged: loss is NaN at epoch {epoch}");
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

/// Train MLP models for all eligible reference sets at startup.
///
/// Iterates the loaded reference sets and trains (or loads from cache) an MLP
/// binary classifier for each binary set that has negative phrases and at least
/// 4 total phrases. Multi-category sets and binary sets without negatives are
/// silently skipped.
///
/// On convergence failure, behavior depends on `fallback`: when `true`, a
/// warning is logged and the set is skipped; when `false`, an error is returned
/// and the daemon should refuse to start.
#[allow(clippy::too_many_arguments)]
pub fn train_models_at_startup(
    reference_sets: &[ReferenceSet],
    cache_dir: &Path,
    learning_rate: f64,
    weight_decay: f64,
    max_epochs: usize,
    patience: usize,
    fallback: bool,
) -> anyhow::Result<Vec<TrainedModel>> {
    tracing::info!("training MLP models at startup");

    let device = <NdArray<f32> as Backend>::Device::default();
    let mut trained: Vec<TrainedModel> = Vec::new();

    for rs in reference_sets {
        let name = &rs.metadata.name;

        // Skip non-binary reference sets.
        let bin = match &rs.kind {
            ReferenceSetKind::Binary(b) => b,
            ReferenceSetKind::MultiCategory(_) => {
                tracing::info!(set = %name, reason = "multi-category set", "skipping MLP training");
                continue;
            }
        };

        // FR-007: skip if no negative embeddings.
        if bin.negative.is_empty() {
            tracing::info!(set = %name, reason = "no negative phrases", "skipping MLP training");
            continue;
        }

        // FR-008: skip if total phrases < 4.
        let total_phrases = bin.positive_phrases.len() + bin.negative_phrases.len();
        if total_phrases < 4 {
            tracing::info!(set = %name, reason = "fewer than 4 phrases", "skipping MLP training");
            continue;
        }

        // Derive input dimension from actual embedding size.
        let embed_dim = bin
            .positive
            .first()
            .or(bin.negative.first())
            .map(|e| e.len())
            .unwrap_or(384);
        let feature_dim = embed_dim + 3;
        let config = MlpConfig::new().with_input_dim(feature_dim);

        // FR-004: compute content hash and check cache.
        let hash = content_hash(&bin.positive_phrases, &bin.negative_phrases);
        let path = cache_path(cache_dir, &hash);

        // Attempt cache load, falling back to training on miss or load failure.
        let classifier = if path.exists() {
            match load_weights::<NdArray<f32>>(&config, &path, &device) {
                Ok(model) => {
                    tracing::info!(set = %name, hash = %hash, "loaded MLP weights from cache");
                    Some(model)
                }
                Err(e) => {
                    tracing::warn!(set = %name, error = %e, "cache load failed, retraining");
                    do_train(
                        name,
                        bin,
                        &path,
                        learning_rate,
                        weight_decay,
                        max_epochs,
                        patience,
                        fallback,
                    )?
                }
            }
        } else {
            do_train(
                name,
                bin,
                &path,
                learning_rate,
                weight_decay,
                max_epochs,
                patience,
                fallback,
            )?
        };

        if let Some(classifier) = classifier {
            trained.push(TrainedModel {
                reference_set_name: name.clone(),
                content_hash: hash,
                classifier,
                pos_embeddings: bin.positive.clone(),
                neg_embeddings: bin.negative.clone(),
                pos_phrases: bin.positive_phrases.clone(),
            });
        }
    }

    tracing::info!(count = trained.len(), "MLP startup training complete");

    Ok(trained)
}

/// Train a single MLP from a binary set's embeddings and save weights to disk.
///
/// Returns `Ok(Some(model))` on success, `Ok(None)` when training fails with
/// `fallback` enabled (set is skipped), or `Err` when training fails with
/// `fallback` disabled.
#[allow(clippy::too_many_arguments)]
fn do_train(
    name: &str,
    bin: &crate::reference_set::BinaryEmbeddings,
    path: &Path,
    learning_rate: f64,
    weight_decay: f64,
    max_epochs: usize,
    patience: usize,
    fallback: bool,
) -> anyhow::Result<Option<MlpClassifier<NdArray<f32>>>> {
    tracing::info!(set = %name, "training MLP model");
    let start = std::time::Instant::now();
    match train_mlp(
        &bin.positive,
        &bin.negative,
        learning_rate,
        weight_decay,
        max_epochs,
        patience,
    ) {
        Ok(model) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            save_weights(model.clone(), path)?;
            tracing::info!(set = %name, duration_ms = duration_ms, "MLP training complete");
            Ok(Some(model))
        }
        Err(e) => {
            if fallback {
                tracing::warn!(set = %name, error = %e, "MLP training failed, using cosine fallback");
                Ok(None)
            } else {
                Err(anyhow::anyhow!("MLP training failed for set '{name}': {e}"))
            }
        }
    }
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

    #[test]
    fn forward_output_shape_and_range() {
        let config = MlpConfig::new();
        let device = <TestBackend as Backend>::Device::default();
        let model = config.init::<TestBackend>(&device);

        let batch = 4;
        let input = Tensor::<TestBackend, 2>::random(
            [batch, 387],
            burn::tensor::Distribution::Uniform(-1.0, 1.0),
            &device,
        );

        let output = model.forward(input);

        // Output shape must be (batch, 1).
        assert_eq!(output.dims(), [batch, 1]);

        // All values must be in (0, 1) — sigmoid output.
        let data: Vec<f32> = output.into_data().to_vec().unwrap();
        for (i, &val) in data.iter().enumerate() {
            assert!(
                val > 0.0 && val < 1.0,
                "output[{i}] = {val} is not in (0, 1)"
            );
        }
    }

    #[test]
    fn cosine_features_known_embeddings() {
        // Use 3-dimensional embeddings for simplicity.
        let text: Embedding = vec![1.0, 0.0, 0.0];
        let pos: Vec<Embedding> = vec![
            vec![1.0, 0.0, 0.0], // identical to text -> cosine = 1.0
            vec![0.0, 1.0, 0.0], // orthogonal -> cosine = 0.0
        ];
        let neg: Vec<Embedding> = vec![
            vec![-1.0, 0.0, 0.0], // opposite -> cosine = -1.0
            vec![0.0, 0.0, 1.0],  // orthogonal -> cosine = 0.0
        ];

        let [max_pos, max_neg, margin] = compute_cosine_features(&text, &pos, &neg);

        assert!(
            (max_pos - 1.0).abs() < 1e-6,
            "max_pos = {max_pos}, expected 1.0"
        );
        assert!(
            (max_neg - 0.0).abs() < 1e-6,
            "max_neg = {max_neg}, expected 0.0"
        );
        assert!(
            (margin - 1.0).abs() < 1e-6,
            "margin = {margin}, expected 1.0"
        );
    }

    #[test]
    fn cosine_features_empty_sets() {
        let text: Embedding = vec![1.0, 0.0, 0.0];

        // Both empty.
        let [max_pos, max_neg, margin] = compute_cosine_features(&text, &[], &[]);
        assert!(
            (max_pos - 0.0).abs() < 1e-6,
            "max_pos = {max_pos}, expected 0.0"
        );
        assert!(
            (max_neg - 0.0).abs() < 1e-6,
            "max_neg = {max_neg}, expected 0.0"
        );
        assert!(
            (margin - 0.0).abs() < 1e-6,
            "margin = {margin}, expected 0.0"
        );

        // Only positives empty.
        let neg: Vec<Embedding> = vec![vec![1.0, 0.0, 0.0]];
        let [max_pos, max_neg, margin] = compute_cosine_features(&text, &[], &neg);
        assert!(
            (max_pos - 0.0).abs() < 1e-6,
            "max_pos = {max_pos}, expected 0.0"
        );
        assert!(
            (max_neg - 1.0).abs() < 1e-6,
            "max_neg = {max_neg}, expected 1.0"
        );
        assert!(
            (margin - -1.0).abs() < 1e-6,
            "margin = {margin}, expected -1.0"
        );

        // Only negatives empty.
        let pos: Vec<Embedding> = vec![vec![1.0, 0.0, 0.0]];
        let [max_pos, max_neg, margin] = compute_cosine_features(&text, &pos, &[]);
        assert!(
            (max_pos - 1.0).abs() < 1e-6,
            "max_pos = {max_pos}, expected 1.0"
        );
        assert!(
            (max_neg - 0.0).abs() < 1e-6,
            "max_neg = {max_neg}, expected 0.0"
        );
        assert!(
            (margin - 1.0).abs() < 1e-6,
            "margin = {margin}, expected 1.0"
        );
    }

    #[test]
    fn content_hash_deterministic() {
        let pos = vec!["hello".to_string(), "world".to_string()];
        let neg = vec!["bad".to_string(), "ugly".to_string()];
        let h1 = content_hash(&pos, &neg);
        let h2 = content_hash(&pos, &neg);
        assert_eq!(h1, h2, "same phrases must produce same hash");

        // Reversed order should also produce the same hash (sorted internally).
        let pos_rev = vec!["world".to_string(), "hello".to_string()];
        let neg_rev = vec!["ugly".to_string(), "bad".to_string()];
        let h3 = content_hash(&pos_rev, &neg_rev);
        assert_eq!(
            h1, h3,
            "different input order must produce same hash after sorting"
        );
    }

    #[test]
    fn content_hash_invalidation() {
        let pos_a = vec!["hello".to_string()];
        let neg_a = vec!["bad".to_string()];
        let pos_b = vec!["hello".to_string(), "extra".to_string()];
        let neg_b = vec!["bad".to_string()];

        let h_a = content_hash(&pos_a, &neg_a);
        let h_b = content_hash(&pos_b, &neg_b);
        assert_ne!(h_a, h_b, "different phrases must produce different hashes");

        // Changing negatives also invalidates.
        let neg_c = vec!["worse".to_string()];
        let h_c = content_hash(&pos_a, &neg_c);
        assert_ne!(
            h_a, h_c,
            "different negative phrases must produce different hashes"
        );
    }

    #[test]
    fn cache_path_format() {
        let dir = Path::new("/tmp/test-cache");
        let hash = "abc123def456";
        let p = cache_path(dir, hash);
        assert_eq!(p, PathBuf::from("/tmp/test-cache/mlp/abc123def456.mpk"));
    }

    #[test]
    fn save_load_roundtrip() {
        let device = <TestBackend as Backend>::Device::default();
        let config = MlpConfig::new();
        let model = config.init::<TestBackend>(&device);

        // Create a deterministic input tensor.
        let input = Tensor::<TestBackend, 2>::ones([2, 387], &device);

        // Run forward pass on original model.
        let output_before: Vec<f32> = model.forward(input.clone()).into_data().to_vec().unwrap();

        // Save to a temp directory.
        let tmp_dir = std::env::temp_dir().join("csn_test_roundtrip");
        std::fs::create_dir_all(tmp_dir.join("mlp")).expect("create temp mlp dir");
        let weight_path = tmp_dir.join("mlp").join("test_weights.mpk");

        save_weights(model, &weight_path).expect("save_weights should succeed");

        // Load back.
        let loaded = load_weights::<TestBackend>(&config, &weight_path, &device)
            .expect("load_weights should succeed");

        // Run forward pass on loaded model.
        let output_after: Vec<f32> = loaded.forward(input).into_data().to_vec().unwrap();

        // Outputs must match exactly (same weights, deterministic forward pass).
        assert_eq!(
            output_before.len(),
            output_after.len(),
            "output lengths must match"
        );
        for (i, (a, b)) in output_before.iter().zip(output_after.iter()).enumerate() {
            assert!(
                (a - b).abs() < 1e-6,
                "output[{i}] mismatch: before={a}, after={b}"
            );
        }

        // Cleanup.
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
