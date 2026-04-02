use burn::backend::NdArray;
use burn::prelude::*;
use serde::{Deserialize, Serialize};

use crate::mlp::{
    CHAR_NGRAM_DIM, TrainedModel, TrainedMultiCatModel, char_ngram_features,
    compute_cosine_features, compute_multi_cosine_features,
};
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

/// Classify text against a reference set, with original text for character features.
///
/// When `trained_model` is `Some` and the reference set is binary, the binary MLP
/// classifier is used. When `trained_multi_model` is `Some` and the reference set
/// is multi-category, the multi-category MLP is used. Otherwise falls back to
/// cosine-only scoring.
fn classify_with_text(
    text_embedding: &Embedding,
    text: &str,
    reference_set: &ReferenceSet,
    trained_model: Option<&TrainedModel>,
    trained_multi_model: Option<&TrainedMultiCatModel>,
) -> ClassifyResult {
    let threshold = reference_set.metadata.threshold;

    match &reference_set.kind {
        ReferenceSetKind::Binary(binary) => {
            if let Some(model) = trained_model {
                return ClassifyResult::Binary(classify_with_mlp(text_embedding, text, model));
            }

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
            if let Some(model) = trained_multi_model {
                return ClassifyResult::MultiCategory(classify_with_multi_mlp(
                    text_embedding,
                    text,
                    model,
                    threshold,
                ));
            }

            // Cosine-only fallback.
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
    trained_model: Option<&TrainedModel>,
    trained_multi_model: Option<&TrainedMultiCatModel>,
) -> anyhow::Result<ClassifyResult> {
    let embedding = engine.embed_one(text)?;
    Ok(classify_with_text(
        &embedding,
        text,
        reference_set,
        trained_model,
        trained_multi_model,
    ))
}

/// Classify a text embedding using a trained MLP model.
///
/// Computes cosine features against the model's cached positive/negative
/// embeddings, concatenates them with the raw text embedding to form a 387-dim
/// input vector, and runs the MLP forward pass. Returns a `BinaryResult` where
/// `confidence` is the MLP sigmoid output, `scores` are the raw cosine maxima,
/// and `top_phrase` is the phrase with the highest positive cosine similarity.
pub fn classify_with_mlp(
    text_embedding: &Embedding,
    text: &str,
    trained_model: &TrainedModel,
) -> BinaryResult {
    // Compute cosine features: [max_pos, max_neg, margin].
    let cosine = compute_cosine_features(
        text_embedding,
        &trained_model.pos_embeddings,
        &trained_model.neg_embeddings,
    );

    // Compute character n-gram features (256-dim).
    let char_feats = char_ngram_features(text);

    // Build 643-dim input: embedding (384) + cosine features (3) + char n-grams (256).
    let mut input_vec: Vec<f32> = Vec::with_capacity(text_embedding.len() + 3 + CHAR_NGRAM_DIM);
    input_vec.extend_from_slice(text_embedding);
    input_vec.extend_from_slice(&cosine);
    input_vec.extend_from_slice(&char_feats);

    let input_dim = input_vec.len();

    // Create Burn tensor and run forward pass.
    let device = <NdArray<f32> as Backend>::Device::default();
    let data = TensorData::new(input_vec, [1, input_dim]);
    let input = Tensor::<NdArray<f32>, 2>::from_data(data, &device);

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

/// Classify a text embedding using a trained multi-category MLP model.
///
/// Computes per-category cosine features, char n-gram features, concatenates
/// with the raw embedding, and runs the MLP forward pass. Applies softmax to
/// get probability distribution. Maps to `MultiCategoryResult`.
pub fn classify_with_multi_mlp(
    text_embedding: &Embedding,
    text: &str,
    trained_model: &TrainedMultiCatModel,
    threshold: f32,
) -> MultiCategoryResult {
    let cosine_feats =
        compute_multi_cosine_features(text_embedding, &trained_model.category_embeddings);
    let char_feats = char_ngram_features(text);

    let num_classes = trained_model.category_names.len();
    let input_dim = text_embedding.len() + num_classes * 3 + CHAR_NGRAM_DIM;

    let mut input_vec: Vec<f32> = Vec::with_capacity(input_dim);
    input_vec.extend_from_slice(text_embedding);
    input_vec.extend_from_slice(&cosine_feats);
    input_vec.extend_from_slice(&char_feats);

    let device = <NdArray<f32> as Backend>::Device::default();
    let data = TensorData::new(input_vec, [1, input_dim]);
    let input = Tensor::<NdArray<f32>, 2>::from_data(data, &device);

    let logits = trained_model.classifier.forward(input);
    let probs = burn::tensor::activation::softmax(logits, 1);
    let probs_data: Vec<f32> = probs.into_data().to_vec().unwrap();

    // Build per-category scores with top phrase from cosine similarity.
    let mut all_scores: Vec<CategoryScore> = trained_model
        .category_names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let (_, phrase) = trained_model
                .category_phrases
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, phrases)| {
                    let embeddings = &trained_model.category_embeddings[i].1;
                    best_match(text_embedding, embeddings, phrases)
                })
                .unwrap_or((0.0, String::new()));

            CategoryScore {
                category: name.clone(),
                score: probs_data[i],
                top_phrase: phrase,
            }
        })
        .collect();

    // Sort by score descending; ties break alphabetically (already alphabetical,
    // so stable sort preserves order for equal scores).
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

    MultiCategoryResult {
        is_match: best.score >= threshold,
        category: best.category,
        confidence: best.score,
        top_phrase: best.top_phrase,
        all_scores,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mlp::{MlpConfig, TrainedModel};
    use burn::backend::NdArray;

    type TestBackend = NdArray;

    /// Helper: create a 384-dim embedding with a given base value and slight variation.
    fn synthetic_embedding(base: f32) -> Embedding {
        (0..384).map(|i| base + (i as f32) * 0.001).collect()
    }

    /// Helper: build a TrainedModel with synthetic embeddings for testing.
    fn make_trained_model() -> TrainedModel {
        let config = MlpConfig::new().with_input_dim(384 + 3 + CHAR_NGRAM_DIM);
        let device = <TestBackend as Backend>::Device::default();
        let classifier = config.init::<TestBackend>(&device);

        let pos_emb = vec![synthetic_embedding(0.5), synthetic_embedding(0.7)];
        let neg_emb = vec![synthetic_embedding(-0.5), synthetic_embedding(-0.3)];
        let pos_phrases = vec!["positive one".to_string(), "positive two".to_string()];

        TrainedModel {
            reference_set_name: "test-set".to_string(),
            content_hash: "testhash".to_string(),
            classifier,
            pos_embeddings: pos_emb,
            neg_embeddings: neg_emb,
            pos_phrases,
        }
    }

    #[test]
    fn classify_with_mlp_returns_valid_binary_result() {
        let model = make_trained_model();
        let text_embedding = synthetic_embedding(0.6);

        let result = classify_with_mlp(&text_embedding, "test text", &model);

        // Confidence must be in (0, 1) — sigmoid output.
        assert!(
            result.confidence > 0.0 && result.confidence < 1.0,
            "confidence = {} is not in (0, 1)",
            result.confidence
        );

        // is_match must be consistent with confidence threshold.
        assert_eq!(
            result.is_match,
            result.confidence > 0.5,
            "is_match should equal confidence > 0.5"
        );

        // top_phrase must be one of the positive phrases.
        assert!(
            result.top_phrase == "positive one" || result.top_phrase == "positive two",
            "top_phrase = '{}' is not a known positive phrase",
            result.top_phrase
        );
    }

    #[test]
    fn classify_with_mlp_scores_are_valid_cosine_values() {
        let model = make_trained_model();
        let text_embedding = synthetic_embedding(0.6);

        let result = classify_with_mlp(&text_embedding, "test text", &model);

        // Cosine similarity values must be in [-1, 1].
        assert!(
            result.scores.positive >= -1.0 && result.scores.positive <= 1.0,
            "scores.positive = {} is not in [-1, 1]",
            result.scores.positive
        );
        assert!(
            result.scores.negative >= -1.0 && result.scores.negative <= 1.0,
            "scores.negative = {} is not in [-1, 1]",
            result.scores.negative
        );
    }

    // --- Multi-category MLP tests ---

    fn make_trained_multi_model() -> TrainedMultiCatModel {
        let config = crate::mlp::MultiCatMlpConfig::new(384 + 3 * 2 + CHAR_NGRAM_DIM, 2);
        let device = <TestBackend as Backend>::Device::default();
        let classifier = config.init::<TestBackend>(&device);

        let cat_emb = vec![
            (
                "alpha".to_string(),
                vec![synthetic_embedding(0.5), synthetic_embedding(0.7)],
            ),
            (
                "beta".to_string(),
                vec![synthetic_embedding(-0.5), synthetic_embedding(-0.3)],
            ),
        ];
        let cat_phrases = vec![
            (
                "alpha".to_string(),
                vec!["alpha one".to_string(), "alpha two".to_string()],
            ),
            (
                "beta".to_string(),
                vec!["beta one".to_string(), "beta two".to_string()],
            ),
        ];
        let names = vec!["alpha".to_string(), "beta".to_string()];

        TrainedMultiCatModel {
            reference_set_name: "test-multi".to_string(),
            content_hash: "multitest".to_string(),
            classifier,
            category_embeddings: cat_emb,
            category_phrases: cat_phrases,
            category_names: names,
        }
    }

    #[test]
    fn classify_with_multi_mlp_returns_valid_result() {
        let model = make_trained_multi_model();
        let text_embedding = synthetic_embedding(0.6);

        let result = classify_with_multi_mlp(&text_embedding, "test text", &model, 0.5);

        // Confidence must be in (0, 1) — softmax output.
        assert!(
            result.confidence > 0.0 && result.confidence < 1.0,
            "confidence = {} is not in (0, 1)",
            result.confidence
        );

        // all_scores should have 2 entries.
        assert_eq!(result.all_scores.len(), 2);

        // Scores should sum to ~1.0 (softmax).
        let sum: f32 = result.all_scores.iter().map(|s| s.score).sum();
        assert!(
            (sum - 1.0).abs() < 1e-4,
            "softmax scores sum = {sum}, expected ~1.0"
        );

        // Category should be one of the known names.
        assert!(
            result.category == "alpha" || result.category == "beta",
            "category = '{}' is not a known category",
            result.category
        );
    }

    #[test]
    fn classify_with_multi_mlp_scores_match_categories() {
        let model = make_trained_multi_model();
        let text_embedding = synthetic_embedding(0.6);

        let result = classify_with_multi_mlp(&text_embedding, "test text", &model, 0.5);

        // Each score should have a valid category name and top_phrase.
        for score in &result.all_scores {
            assert!(
                score.category == "alpha" || score.category == "beta",
                "unexpected category: {}",
                score.category
            );
            assert!(
                !score.top_phrase.is_empty(),
                "top_phrase should not be empty"
            );
        }
    }

    #[test]
    fn classify_with_text_routes_to_multi_mlp() {
        use crate::reference_set::{
            CategoryEmbeddings, Metadata, Mode, MultiCategoryEmbeddings, ReferenceSet,
            ReferenceSetKind,
        };
        use std::collections::HashMap;
        use std::path::PathBuf;

        let model = make_trained_multi_model();

        // Build a matching multi-category reference set.
        let mut categories = HashMap::new();
        categories.insert(
            "alpha".to_string(),
            CategoryEmbeddings {
                embeddings: vec![synthetic_embedding(0.5), synthetic_embedding(0.7)],
                phrases: vec!["alpha one".to_string(), "alpha two".to_string()],
            },
        );
        categories.insert(
            "beta".to_string(),
            CategoryEmbeddings {
                embeddings: vec![synthetic_embedding(-0.5), synthetic_embedding(-0.3)],
                phrases: vec!["beta one".to_string(), "beta two".to_string()],
            },
        );

        let ref_set = ReferenceSet {
            metadata: Metadata {
                name: "test-multi".to_string(),
                description: None,
                mode: Mode::MultiCategory,
                threshold: 0.3,
                source: None,
            },
            kind: ReferenceSetKind::MultiCategory(MultiCategoryEmbeddings { categories }),
            content_hash: "test".to_string(),
            source_path: PathBuf::from("/tmp/test.toml"),
        };

        let text_emb = synthetic_embedding(0.6);
        let result = classify_with_text(&text_emb, "test", &ref_set, None, Some(&model));

        // Should route through multi-cat MLP, producing a MultiCategory result.
        match &result {
            ClassifyResult::MultiCategory(r) => {
                assert_eq!(r.all_scores.len(), 2);
                let sum: f32 = r.all_scores.iter().map(|s| s.score).sum();
                assert!(
                    (sum - 1.0).abs() < 1e-4,
                    "MLP softmax scores should sum to ~1.0, got {sum}"
                );
            }
            ClassifyResult::Binary(_) => {
                panic!("expected MultiCategory result, got Binary");
            }
        }
    }
}
