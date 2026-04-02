use std::sync::Mutex;

use async_trait::async_trait;
use rust_mcp_sdk::macros::{JsonSchema, mcp_tool};
use rust_mcp_sdk::mcp_server::ServerHandler;
use rust_mcp_sdk::schema::{
    CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams, RpcError,
    TextContent, schema_utils::CallToolError,
};
use rust_mcp_sdk::tool_box;
use rust_mcp_sdk::McpServer;

use crate::classifier;
use crate::mlp::TrainedModel;
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
    model_choice: ModelChoice,
}

// SAFETY: McpHandler fields are protected by Mutex, making concurrent access safe.
// The non-Sync types (EmbeddingEngine, TrainedModel) are never shared directly.
unsafe impl Sync for McpHandler {}

impl McpHandler {
    pub fn new(
        engine: EmbeddingEngine,
        reference_sets: Vec<ReferenceSet>,
        trained_models: Vec<TrainedModel>,
        model_choice: ModelChoice,
    ) -> Self {
        Self {
            engine: Mutex::new(engine),
            reference_sets,
            trained_models: Mutex::new(trained_models),
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

tool_box!(CsnTools, [ClassifyTool, ListSetsTool, EmbedTool, SimilarityTool]);

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

        let mut engine = self
            .engine
            .lock()
            .map_err(|_| CallToolError::from_message("engine lock poisoned".to_string()))?;

        let result = classifier::classify_text(&mut engine, &tool.text, set, trained_model)
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
        let tool: CsnTools = CsnTools::try_from(params).map_err(|e| CallToolError::from_message(e.to_string()))?;

        match tool {
            CsnTools::ClassifyTool(t) => self.handle_classify(t),
            CsnTools::ListSetsTool(_) => self.handle_list_sets(),
            CsnTools::EmbedTool(t) => self.handle_embed(t),
            CsnTools::SimilarityTool(t) => self.handle_similarity(t),
        }
    }
}
