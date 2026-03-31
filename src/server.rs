use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::classifier;
use crate::model::{EmbeddingEngine, ModelChoice, cosine_similarity};
use crate::reference_set::{ReferenceSet, load_all_reference_sets};

pub struct AppState {
    engine: Mutex<EmbeddingEngine>,
    sets: Vec<ReferenceSet>,
    start_time: Instant,
    model: ModelChoice,
}

#[derive(Deserialize)]
pub struct ClassifyRequest {
    text: String,
    reference_set: String,
}

#[derive(Deserialize)]
pub struct EmbedRequest {
    text: String,
}

#[derive(Serialize)]
pub struct EmbedResponse {
    embedding: Vec<f32>,
    dimensions: usize,
    model: String,
}

#[derive(Deserialize)]
pub struct SimilarityRequest {
    a: String,
    b: String,
}

#[derive(Serialize)]
pub struct SimilarityResponse {
    similarity: f32,
}

#[derive(Serialize)]
pub struct HealthResponse {
    status: &'static str,
    model: String,
    sets: usize,
    uptime: String,
}

#[derive(Serialize)]
pub struct SetInfo {
    name: String,
    phrases: usize,
    mode: String,
}

pub async fn serve(model: ModelChoice, sets_dir: PathBuf, port: u16) -> anyhow::Result<()> {
    let start = Instant::now();

    tracing::info!(%model, "loading model");
    let mut engine = EmbeddingEngine::new(model, None)?;

    tracing::info!(dir = %sets_dir.display(), "loading reference sets");
    let sets = load_all_reference_sets(&sets_dir, &mut engine)?;
    tracing::info!(count = sets.len(), "reference sets loaded");

    let state = Arc::new(AppState {
        engine: Mutex::new(engine),
        sets,
        start_time: start,
        model,
    });

    let app = Router::new()
        .route("/classify", post(handle_classify))
        .route("/embed", post(handle_embed))
        .route("/similarity", post(handle_similarity))
        .route("/health", get(handle_health))
        .route("/sets", get(handle_sets))
        .with_state(state);

    let addr = format!("127.0.0.1:{port}");
    tracing::info!(%addr, elapsed = ?start.elapsed(), "server ready");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_classify(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ClassifyRequest>,
) -> Result<Json<classifier::ClassifyResult>, (StatusCode, String)> {
    let set = state
        .sets
        .iter()
        .find(|s| s.metadata.name == req.reference_set)
        .ok_or_else(|| {
            let available: Vec<_> = state
                .sets
                .iter()
                .map(|s| s.metadata.name.as_str())
                .collect();
            (
                StatusCode::NOT_FOUND,
                format!(
                    "reference set '{}' not found. Available: {}",
                    req.reference_set,
                    available.join(", ")
                ),
            )
        })?;

    let mut engine = state.engine.lock().unwrap();
    let result = classifier::classify_text(&mut engine, &req.text, set)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(result))
}

async fn handle_embed(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EmbedRequest>,
) -> Result<Json<EmbedResponse>, (StatusCode, String)> {
    let mut engine = state.engine.lock().unwrap();
    let embedding = engine
        .embed_one(&req.text)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let dimensions = embedding.len();

    Ok(Json(EmbedResponse {
        embedding,
        dimensions,
        model: state.model.to_string(),
    }))
}

async fn handle_similarity(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SimilarityRequest>,
) -> Result<Json<SimilarityResponse>, (StatusCode, String)> {
    let mut engine = state.engine.lock().unwrap();
    let embeddings = engine
        .embed(&[req.a.as_str(), req.b.as_str()])
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let similarity = cosine_similarity(&embeddings[0], &embeddings[1]);
    Ok(Json(SimilarityResponse { similarity }))
}

async fn handle_health(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    let uptime = state.start_time.elapsed();
    let secs = uptime.as_secs();
    let uptime_str = if secs >= 3600 {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m{}s", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    };

    Json(HealthResponse {
        status: "ok",
        model: state.model.to_string(),
        sets: state.sets.len(),
        uptime: uptime_str,
    })
}

async fn handle_sets(State(state): State<Arc<AppState>>) -> Json<Vec<SetInfo>> {
    let sets = state
        .sets
        .iter()
        .map(|s| {
            let mode = match &s.kind {
                crate::reference_set::ReferenceSetKind::Binary(_) => "binary",
                crate::reference_set::ReferenceSetKind::MultiCategory(_) => "multi-category",
            };
            SetInfo {
                name: s.metadata.name.clone(),
                phrases: s.phrase_count(),
                mode: mode.to_string(),
            }
        })
        .collect();
    Json(sets)
}
