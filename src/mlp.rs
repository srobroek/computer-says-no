// TODO(T003): remove allow(dead_code) once forward() is implemented
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use burn::nn::{Linear, LinearConfig, Relu};
use burn::prelude::*;
use burn::record::{FullPrecisionSettings, NamedMpkFileRecorder};

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
pub fn save_weights<B: Backend>(
    model: MlpClassifier<B>,
    path: &Path,
) -> anyhow::Result<()> {
    let recorder = NamedMpkFileRecorder::<FullPrecisionSettings>::new();
    // Burn's FileRecorder appends the extension automatically, so strip it.
    let base = path.with_extension("");
    model
        .save_file(base, &recorder)
        .map_err(|e| anyhow::anyhow!("failed to save MLP weights: {e}"))?;
    Ok(())
}

/// Load MLP model weights from disk using the Burn `NamedMpkFileRecorder`.
///
/// Initialises a fresh model from `config`, then loads the saved record into it.
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
