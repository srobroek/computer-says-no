use burn::backend::NdArray;
use burn::prelude::*;
use serde::{Deserialize, Serialize};

use crate::mlp::{TrainedModel, compute_cosine_features};
use crate::model::{Embedding, EmbeddingEngine, cosine_similarity};
use crate::reference_set::{ReferenceSet, ReferenceSetKind};

/// Result of a binary classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryResult {
    #[serde(rename = "match")]
    pub is_match: bool,
    pub confidence: f32,
    pub top_phrase: String,
    pub scores: BinaryScores,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryScores {
    pub positive: f32,
    pub negative: f32,
}

/// Result of a multi-category classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiCategoryResult {
    #[serde(rename = "match")]
    pub is_match: bool,
    pub category: String,
    pub confidence: f32,
    pub top_phrase: String,
    pub all_scores: Vec<CategoryScore>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryScore {
    pub category: String,
    pub score: f32,
    pub top_phrase: String,
}

/// Unified classification result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ClassifyResult {
    Binary(BinaryResult),
    MultiCategory(MultiCategoryResult),
}

#[allow(dead_code)]
impl ClassifyResult {
    pub fn is_match(&self) -> bool {
        match self {
            Self::Binary(r) => r.is_match,
            Self::MultiCategory(r) => r.is_match,
        }
    }

    pub fn confidence(&self) -> f32 {
        match self {
            Self::Binary(r) => r.confidence,
            Self::MultiCategory(r) => r.confidence,
        }
    }
}

/// Classify text against a reference set.
pub fn classify(text_embedding: &Embedding, reference_set: &ReferenceSet) -> ClassifyResult {
    let threshold = reference_set.metadata.threshold;

    match &reference_set.kind {
        ReferenceSetKind::Binary(binary) => {
            let (pos_score, pos_phrase) =
                best_match(text_embedding, &binary.positive, &binary.positive_phrases);
            let (neg_score, _neg_phrase) = if binary.negative.is_empty() {
                (0.0, String::new())
            } else {
                best_match(text_embedding, &binary.negative, &binary.negative_phrases)
            };

            let is_match = pos_score >= threshold && pos_score > neg_score;

            ClassifyResult::Binary(BinaryResult {
                is_match,
                confidence: pos_score,
                top_phrase: pos_phrase,
                scores: BinaryScores {
                    positive: pos_score,
                    negative: neg_score,
                },
            })
        }
        ReferenceSetKind::MultiCategory(multi) => {
            let mut all_scores: Vec<CategoryScore> = multi
                .categories
                .iter()
                .map(|(name, cat)| {
                    let (score, phrase) = best_match(text_embedding, &cat.embeddings, &cat.phrases);
                    CategoryScore {
                        category: name.clone(),
                        score,
                        top_phrase: phrase,
                    }
                })
                .collect();

            all_scores.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let best = all_scores.first().cloned().unwrap_or(CategoryScore {
                category: String::new(),
                score: 0.0,
                top_phrase: String::new(),
            });

            ClassifyResult::MultiCategory(MultiCategoryResult {
                is_match: best.score >= threshold,
                category: best.category,
                confidence: best.score,
                top_phrase: best.top_phrase,
                all_scores,
            })
        }
    }
}

/// Classify text by first embedding it, then comparing.
pub fn classify_text(
    engine: &mut EmbeddingEngine,
    text: &str,
    reference_set: &ReferenceSet,
) -> anyhow::Result<ClassifyResult> {
    let embedding = engine.embed_one(text)?;
    Ok(classify(&embedding, reference_set))
}

/// Classify a text embedding using a trained MLP model.
///
/// Computes cosine features against the model's cached positive/negative
/// embeddings, concatenates them with the raw text embedding to form a 387-dim
/// input vector, and runs the MLP forward pass. Returns a `BinaryResult` where
/// `confidence` is the MLP sigmoid output, `scores` are the raw cosine maxima,
/// and `top_phrase` is the phrase with the highest positive cosine similarity.
#[allow(dead_code)]
pub fn classify_with_mlp(text_embedding: &Embedding, trained_model: &TrainedModel) -> BinaryResult {
    // Compute cosine features: [max_pos, max_neg, margin].
    let cosine = compute_cosine_features(
        text_embedding,
        &trained_model.pos_embeddings,
        &trained_model.neg_embeddings,
    );

    // Build 387-dim input: embedding (384) + cosine features (3).
    let mut input_vec: Vec<f32> = Vec::with_capacity(text_embedding.len() + 3);
    input_vec.extend_from_slice(text_embedding);
    input_vec.extend_from_slice(&cosine);

    let input_dim = input_vec.len();

    // Create Burn tensor and run forward pass.
    let device = <NdArray<f32> as Backend>::Device::default();
    let data = TensorData::from(input_vec.as_slice());
    let input = Tensor::<NdArray<f32>, 2>::from_data(data, &device).reshape([1, input_dim as i64]);

    let output = trained_model.classifier.forward(input);
    let confidence: f32 = output.into_scalar().elem();

    // Get top phrase from positive embeddings using best_match.
    let (_pos_score, top_phrase) = best_match(
        text_embedding,
        &trained_model.pos_embeddings,
        &trained_model.pos_phrases,
    );

    BinaryResult {
        is_match: confidence > 0.5,
        confidence,
        top_phrase,
        scores: BinaryScores {
            positive: cosine[0], // max_pos
            negative: cosine[1], // max_neg
        },
    }
}

/// Find the best matching phrase and its similarity score.
fn best_match(query: &Embedding, embeddings: &[Embedding], phrases: &[String]) -> (f32, String) {
    let mut best_score = f32::NEG_INFINITY;
    let mut best_phrase = String::new();

    for (emb, phrase) in embeddings.iter().zip(phrases.iter()) {
        let score = cosine_similarity(query, emb);
        if score > best_score {
            best_score = score;
            best_phrase = phrase.clone();
        }
    }

    if best_score == f32::NEG_INFINITY {
        (0.0, String::new())
    } else {
        (best_score, best_phrase)
    }
}
