use std::sync::Mutex;

use async_trait::async_trait;
use rust_mcp_sdk::McpServer;
use rust_mcp_sdk::macros::{JsonSchema, mcp_tool};
use rust_mcp_sdk::mcp_server::ServerHandler;
use rust_mcp_sdk::schema::{
    CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams, RpcError,
    TextContent, schema_utils::CallToolError,
};
use rust_mcp_sdk::tool_box;

use crate::classifier;
use crate::mlp::{TrainedModel, TrainedMultiCatModel};
use crate::model::{EmbeddingEngine, ModelChoice, cosine_similarity};
use crate::reference_set::{ReferenceSet, ReferenceSetKind};

/// Shared state for the MCP server handler.
///
/// Uses Mutex for engine (not Sync due to fastembed internals) and trained_models
/// (not Sync due to Burn NdArray). The Mutex makes McpHandler Send+Sync as
/// required by the async ServerHandler trait.
pub struct McpHandler {
    engine: Mutex<EmbeddingEngine>,
    reference_sets: Vec<ReferenceSet>,
    trained_models: Mutex<Vec<TrainedModel>>,
    trained_multi_models: Mutex<Vec<TrainedMultiCatModel>>,
    model_choice: ModelChoice,
}

// SAFETY: McpHandler fields are protected by Mutex, making concurrent access safe.
// The non-Sync types (EmbeddingEngine, TrainedModel, TrainedMultiCatModel) are never shared directly.
unsafe impl Sync for McpHandler {}

impl McpHandler {
    pub fn new(
        engine: EmbeddingEngine,
        reference_sets: Vec<ReferenceSet>,
        trained_models: Vec<TrainedModel>,
        trained_multi_models: Vec<TrainedMultiCatModel>,
        model_choice: ModelChoice,
    ) -> Self {
        Self {
            engine: Mutex::new(engine),
            reference_sets,
            trained_models: Mutex::new(trained_models),
            trained_multi_models: Mutex::new(trained_multi_models),
            model_choice,
        }
    }
}

// --- Tool definitions ---

#[mcp_tool(
    name = "classify",
    description = "Classify text against a reference set. Returns match status, confidence, top matching phrase, and similarity scores."
)]
#[derive(Debug, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct ClassifyTool {
    /// The text to classify
    pub text: String,
    /// Name of the reference set to classify against
    pub reference_set: String,
}

#[mcp_tool(
    name = "list_sets",
    description = "List all available reference sets with their name, mode, and phrase count."
)]
#[derive(Debug, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct ListSetsTool {}

#[mcp_tool(
    name = "embed",
    description = "Generate an embedding vector for the given text. Returns the vector, dimension count, and model name."
)]
#[derive(Debug, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct EmbedTool {
    /// The text to embed
    pub text: String,
}

#[mcp_tool(
    name = "similarity",
    description = "Compute cosine similarity between two texts. Returns a score from -1 (opposite) to 1 (identical)."
)]
#[derive(Debug, serde::Deserialize, serde::Serialize, JsonSchema)]
pub struct SimilarityTool {
    /// First text to compare
    pub a: String,
    /// Second text to compare
    pub b: String,
}

tool_box!(
    CsnTools,
    [ClassifyTool, ListSetsTool, EmbedTool, SimilarityTool]
);

// --- Tool implementations ---

impl McpHandler {
    fn handle_classify(&self, tool: ClassifyTool) -> Result<CallToolResult, CallToolError> {
        let set = self
            .reference_sets
            .iter()
            .find(|s| s.metadata.name == tool.reference_set)
            .ok_or_else(|| {
                let available: Vec<_> = self
                    .reference_sets
                    .iter()
                    .map(|s| s.metadata.name.as_str())
                    .collect();
                CallToolError::from_message(format!(
                    "reference set '{}' not found. Available: {}",
                    tool.reference_set,
                    available.join(", ")
                ))
            })?;

        let trained_models = self
            .trained_models
            .lock()
            .map_err(|_| CallToolError::from_message("models lock poisoned".to_string()))?;
        let trained_model = trained_models
            .iter()
            .find(|m| m.reference_set_name == tool.reference_set);

        let trained_multi_models = self
            .trained_multi_models
            .lock()
            .map_err(|_| CallToolError::from_message("multi models lock poisoned".to_string()))?;
        let trained_multi_model = trained_multi_models
            .iter()
            .find(|m| m.reference_set_name == tool.reference_set);

        let mut engine = self
            .engine
            .lock()
            .map_err(|_| CallToolError::from_message("engine lock poisoned".to_string()))?;

        let result =
            classifier::classify_text(&mut engine, &tool.text, set, trained_model, trained_multi_model)
                .map_err(|e| CallToolError::from_message(e.to_string()))?;

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| CallToolError::from_message(format!("serialization error: {e}")))?;

        Ok(CallToolResult::text_content(vec![TextContent::from(json)]))
    }

    fn handle_list_sets(&self) -> Result<CallToolResult, CallToolError> {
        let sets: Vec<serde_json::Value> = self
            .reference_sets
            .iter()
            .map(|s| {
                let mode = match &s.kind {
                    ReferenceSetKind::Binary(_) => "binary",
                    ReferenceSetKind::MultiCategory(_) => "multi-category",
                };
                serde_json::json!({
                    "name": s.metadata.name,
                    "mode": mode,
                    "phrase_count": s.phrase_count(),
                })
            })
            .collect();

        let json = serde_json::to_string_pretty(&sets)
            .map_err(|e| CallToolError::from_message(format!("serialization error: {e}")))?;

        Ok(CallToolResult::text_content(vec![TextContent::from(json)]))
    }

    fn handle_embed(&self, tool: EmbedTool) -> Result<CallToolResult, CallToolError> {
        let mut engine = self
            .engine
            .lock()
            .map_err(|_| CallToolError::from_message("engine lock poisoned".to_string()))?;

        let embedding = engine
            .embed_one(&tool.text)
            .map_err(|e| CallToolError::from_message(e.to_string()))?;

        let model_name = self.model_choice.as_str().to_string();
        let json = serde_json::to_string_pretty(&serde_json::json!({
            "embedding": embedding,
            "dimensions": embedding.len(),
            "model": model_name,
        }))
        .map_err(|e| CallToolError::from_message(format!("serialization error: {e}")))?;

        Ok(CallToolResult::text_content(vec![TextContent::from(json)]))
    }

    fn handle_similarity(&self, tool: SimilarityTool) -> Result<CallToolResult, CallToolError> {
        let mut engine = self
            .engine
            .lock()
            .map_err(|_| CallToolError::from_message("engine lock poisoned".to_string()))?;

        let embeddings = engine
            .embed(&[&tool.a, &tool.b])
            .map_err(|e| CallToolError::from_message(e.to_string()))?;

        let sim = cosine_similarity(&embeddings[0], &embeddings[1]);
        let model_name = self.model_choice.as_str().to_string();

        let json = serde_json::to_string_pretty(&serde_json::json!({
            "similarity": sim,
            "model": model_name,
        }))
        .map_err(|e| CallToolError::from_message(format!("serialization error: {e}")))?;

        Ok(CallToolResult::text_content(vec![TextContent::from(json)]))
    }
}

// --- ServerHandler implementation ---

#[async_trait]
impl ServerHandler for McpHandler {
    async fn handle_list_tools_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            tools: CsnTools::tools(),
            meta: None,
            next_cursor: None,
        })
    }

    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _runtime: std::sync::Arc<dyn McpServer>,
    ) -> Result<CallToolResult, CallToolError> {
        let tool: CsnTools =
            CsnTools::try_from(params).map_err(|e| CallToolError::from_message(e.to_string()))?;

        match tool {
            CsnTools::ClassifyTool(t) => self.handle_classify(t),
            CsnTools::ListSetsTool(_) => self.handle_list_sets(),
            CsnTools::EmbedTool(t) => self.handle_embed(t),
            CsnTools::SimilarityTool(t) => self.handle_similarity(t),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classifier::{BinaryResult, BinaryScores, ClassifyResult};
    use crate::model::Embedding;
    use crate::reference_set::{BinaryEmbeddings, Metadata, Mode, ReferenceSet, ReferenceSetKind};
    use std::path::PathBuf;

    fn synthetic_embedding(base: f32) -> Embedding {
        (0..384).map(|i| base + (i as f32) * 0.001).collect()
    }

    fn make_reference_set(
        name: &str,
        positive_count: usize,
        negative_count: usize,
    ) -> ReferenceSet {
        let pos_embeddings: Vec<Embedding> = (0..positive_count)
            .map(|i| synthetic_embedding(0.5 + i as f32 * 0.1))
            .collect();
        let neg_embeddings: Vec<Embedding> = (0..negative_count)
            .map(|i| synthetic_embedding(-0.5 - i as f32 * 0.1))
            .collect();
        let pos_phrases: Vec<String> = (0..positive_count)
            .map(|i| format!("positive {i}"))
            .collect();
        let neg_phrases: Vec<String> = (0..negative_count)
            .map(|i| format!("negative {i}"))
            .collect();

        ReferenceSet {
            metadata: Metadata {
                name: name.to_string(),
                description: Some(format!("{name} description")),
                mode: Mode::Binary,
                threshold: 0.5,
                source: None,
            },
            kind: ReferenceSetKind::Binary(BinaryEmbeddings {
                positive: pos_embeddings,
                positive_phrases: pos_phrases,
                negative: neg_embeddings,
                negative_phrases: neg_phrases,
            }),
            content_hash: "testhash".to_string(),
            source_path: PathBuf::from("/tmp/test.toml"),
        }
    }

    #[test]
    fn tools_list_contains_four_tools() {
        let tools = CsnTools::tools();
        assert_eq!(tools.len(), 4);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"classify"));
        assert!(names.contains(&"list_sets"));
        assert!(names.contains(&"embed"));
        assert!(names.contains(&"similarity"));
    }

    #[test]
    fn list_sets_returns_correct_metadata() {
        let sets = [
            make_reference_set("corrections", 3, 2),
            make_reference_set("safety", 5, 0),
        ];

        let sets_json: Vec<serde_json::Value> = sets
            .iter()
            .map(|s| {
                let mode = match &s.kind {
                    ReferenceSetKind::Binary(_) => "binary",
                    ReferenceSetKind::MultiCategory(_) => "multi-category",
                };
                serde_json::json!({
                    "name": s.metadata.name,
                    "mode": mode,
                    "phrase_count": s.phrase_count(),
                })
            })
            .collect();

        assert_eq!(sets_json.len(), 2);

        assert_eq!(sets_json[0]["name"], "corrections");
        assert_eq!(sets_json[0]["mode"], "binary");
        assert_eq!(sets_json[0]["phrase_count"], 5); // 3 pos + 2 neg

        assert_eq!(sets_json[1]["name"], "safety");
        assert_eq!(sets_json[1]["mode"], "binary");
        assert_eq!(sets_json[1]["phrase_count"], 5); // 5 pos + 0 neg
    }

    #[test]
    fn list_sets_empty_returns_empty_array() {
        let sets: Vec<ReferenceSet> = vec![];
        let sets_json: Vec<serde_json::Value> = sets
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.metadata.name,
                    "mode": "binary",
                    "phrase_count": s.phrase_count(),
                })
            })
            .collect();

        assert!(sets_json.is_empty());
        let json = serde_json::to_string_pretty(&sets_json).unwrap();
        assert_eq!(json, "[]");
    }

    #[test]
    fn classify_result_serializes_with_expected_fields() {
        let result = ClassifyResult::Binary(BinaryResult {
            is_match: true,
            confidence: 0.85,
            top_phrase: "test phrase".to_string(),
            scores: BinaryScores {
                positive: 0.9,
                negative: 0.3,
            },
        });

        let json = serde_json::to_string_pretty(&result).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["match"], true);
        assert!(parsed["confidence"].as_f64().unwrap() > 0.8);
        assert_eq!(parsed["top_phrase"], "test phrase");
        assert!(parsed["scores"]["positive"].as_f64().is_some());
        assert!(parsed["scores"]["negative"].as_f64().is_some());
    }

    #[test]
    fn embed_result_has_expected_json_structure() {
        let embedding = synthetic_embedding(0.5);
        let model_name = ModelChoice::default().as_str().to_string();
        let json_val = serde_json::json!({
            "embedding": embedding,
            "dimensions": embedding.len(),
            "model": model_name,
        });

        assert_eq!(json_val["dimensions"], 384);
        assert_eq!(json_val["embedding"].as_array().unwrap().len(), 384);
        assert!(json_val["model"].as_str().is_some());
    }

    #[test]
    fn similarity_identical_embeddings_near_one() {
        let emb_a = synthetic_embedding(0.5);
        let emb_b = synthetic_embedding(0.5);
        let sim = cosine_similarity(&emb_a, &emb_b);
        let model_name = ModelChoice::default().as_str().to_string();

        let json_val = serde_json::json!({
            "similarity": sim,
            "model": model_name,
        });

        let score = json_val["similarity"].as_f64().unwrap();
        assert!(
            score > 0.99,
            "identical embeddings should have similarity ~1.0, got {score}"
        );
        assert!(json_val["model"].as_str().is_some());
    }

    #[test]
    fn similarity_different_embeddings_less_than_one() {
        let emb_a = synthetic_embedding(0.5);
        let emb_b = synthetic_embedding(-0.5);
        let sim = cosine_similarity(&emb_a, &emb_b);

        assert!(
            sim < 0.99,
            "different embeddings should have similarity < 1.0, got {sim}"
        );
    }
}
