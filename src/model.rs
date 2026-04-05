use anyhow::{Context, Result};
use fastembed::{EmbeddingModel, TextEmbedding};
use std::path::PathBuf;

pub type Embedding = Vec<f32>;

/// Supported models for benchmarking and configuration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ModelChoice {
    AllMiniLML6V2,
    AllMiniLML6V2Q,
    BGESmallENV15,
    BGESmallENV15Q,
    BGELargeENV15,
    BGELargeENV15Q,
    NomicEmbedTextV15,
    #[default]
    NomicEmbedTextV15Q,
    GTELargeENV15,
    GTELargeENV15Q,
    SnowflakeArcticEmbedS,
    SnowflakeArcticEmbedSQ,
}

impl ModelChoice {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AllMiniLML6V2 => "all-MiniLM-L6-v2",
            Self::AllMiniLML6V2Q => "all-MiniLM-L6-v2-Q",
            Self::BGESmallENV15 => "bge-small-en-v1.5",
            Self::BGESmallENV15Q => "bge-small-en-v1.5-Q",
            Self::BGELargeENV15 => "bge-large-en-v1.5",
            Self::BGELargeENV15Q => "bge-large-en-v1.5-Q",
            Self::NomicEmbedTextV15 => "nomic-embed-text-v1.5",
            Self::NomicEmbedTextV15Q => "nomic-embed-text-v1.5-Q",
            Self::GTELargeENV15 => "gte-large-en-v1.5",
            Self::GTELargeENV15Q => "gte-large-en-v1.5-Q",
            Self::SnowflakeArcticEmbedS => "snowflake-arctic-embed-s",
            Self::SnowflakeArcticEmbedSQ => "snowflake-arctic-embed-s-Q",
        }
    }

    #[must_use]
    pub const fn dimensions(self) -> usize {
        match self {
            Self::AllMiniLML6V2
            | Self::AllMiniLML6V2Q
            | Self::BGESmallENV15
            | Self::BGESmallENV15Q
            | Self::SnowflakeArcticEmbedS
            | Self::SnowflakeArcticEmbedSQ => 384,
            Self::NomicEmbedTextV15 | Self::NomicEmbedTextV15Q => 768,
            Self::BGELargeENV15
            | Self::BGELargeENV15Q
            | Self::GTELargeENV15
            | Self::GTELargeENV15Q => 1024,
        }
    }

    const fn to_fastembed(self) -> EmbeddingModel {
        match self {
            Self::AllMiniLML6V2 => EmbeddingModel::AllMiniLML6V2,
            Self::AllMiniLML6V2Q => EmbeddingModel::AllMiniLML6V2Q,
            Self::BGESmallENV15 => EmbeddingModel::BGESmallENV15,
            Self::BGESmallENV15Q => EmbeddingModel::BGESmallENV15Q,
            Self::BGELargeENV15 => EmbeddingModel::BGELargeENV15,
            Self::BGELargeENV15Q => EmbeddingModel::BGELargeENV15Q,
            Self::NomicEmbedTextV15 => EmbeddingModel::NomicEmbedTextV15,
            Self::NomicEmbedTextV15Q => EmbeddingModel::NomicEmbedTextV15Q,
            Self::GTELargeENV15 => EmbeddingModel::GTELargeENV15,
            Self::GTELargeENV15Q => EmbeddingModel::GTELargeENV15Q,
            Self::SnowflakeArcticEmbedS => EmbeddingModel::SnowflakeArcticEmbedS,
            Self::SnowflakeArcticEmbedSQ => EmbeddingModel::SnowflakeArcticEmbedSQ,
        }
    }

    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::AllMiniLML6V2,
            Self::AllMiniLML6V2Q,
            Self::BGESmallENV15,
            Self::BGESmallENV15Q,
            Self::BGELargeENV15,
            Self::BGELargeENV15Q,
            Self::NomicEmbedTextV15,
            Self::NomicEmbedTextV15Q,
            Self::GTELargeENV15,
            Self::GTELargeENV15Q,
            Self::SnowflakeArcticEmbedS,
            Self::SnowflakeArcticEmbedSQ,
        ]
    }
}

impl std::str::FromStr for ModelChoice {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "all-MiniLM-L6-v2" => Ok(Self::AllMiniLML6V2),
            "all-MiniLM-L6-v2-Q" => Ok(Self::AllMiniLML6V2Q),
            "bge-small-en-v1.5" => Ok(Self::BGESmallENV15),
            "bge-small-en-v1.5-Q" => Ok(Self::BGESmallENV15Q),
            "bge-large-en-v1.5" => Ok(Self::BGELargeENV15),
            "bge-large-en-v1.5-Q" => Ok(Self::BGELargeENV15Q),
            "nomic-embed-text-v1.5" => Ok(Self::NomicEmbedTextV15),
            "nomic-embed-text-v1.5-Q" => Ok(Self::NomicEmbedTextV15Q),
            "gte-large-en-v1.5" => Ok(Self::GTELargeENV15),
            "gte-large-en-v1.5-Q" => Ok(Self::GTELargeENV15Q),
            "snowflake-arctic-embed-s" => Ok(Self::SnowflakeArcticEmbedS),
            "snowflake-arctic-embed-s-Q" => Ok(Self::SnowflakeArcticEmbedSQ),
            _ => anyhow::bail!(
                "unknown model: {s}. Available: {}",
                Self::all()
                    .iter()
                    .map(|m| m.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

impl std::fmt::Display for ModelChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Wrapper around fastembed's `TextEmbedding` with cosine similarity.
pub struct EmbeddingEngine {
    inner: TextEmbedding,
    model: ModelChoice,
}

impl EmbeddingEngine {
    pub fn new(model: ModelChoice, cache_dir: Option<PathBuf>) -> Result<Self> {
        let mut options =
            fastembed::InitOptions::new(model.to_fastembed()).with_show_download_progress(true);
        if let Some(dir) = cache_dir {
            options = options.with_cache_dir(dir);
        }
        let inner = TextEmbedding::try_new(options)
            .with_context(|| format!("failed to load model {model}"))?;
        Ok(Self { inner, model })
    }

    #[must_use]
    pub const fn model(&self) -> ModelChoice {
        self.model
    }

    #[must_use]
    pub const fn dimensions(&self) -> usize {
        self.model.dimensions()
    }

    pub fn embed(&mut self, texts: &[&str]) -> Result<Vec<Embedding>> {
        self.inner.embed(texts, None).context("embedding failed")
    }

    pub fn embed_one(&mut self, text: &str) -> Result<Embedding> {
        let mut results = self.embed(&[text])?;
        results.pop().context("empty embedding result")
    }
}

/// Cosine similarity between two embedding vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "embedding dimensions must match");
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 { 0.0 } else { dot / denom }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_identical_vectors() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_orthogonal_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn model_roundtrip() {
        for m in ModelChoice::all() {
            let parsed: ModelChoice = m.as_str().parse().unwrap();
            assert_eq!(*m, parsed);
        }
    }
}
