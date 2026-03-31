use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use crate::classifier;
use crate::config::AppConfig;
use crate::model::{EmbeddingEngine, ModelChoice, cosine_similarity};
use crate::reference_set::{ReferenceSet, load_all_reference_sets};

pub struct AppState {
    pub engine: Mutex<EmbeddingEngine>,
    pub sets: RwLock<Vec<ReferenceSet>>,
    pub start_time: Instant,
    pub model: ModelChoice,
    pub sets_dir: PathBuf,
    pub cache_dir: PathBuf,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

struct AppError {
    status: StatusCode,
    message: String,
}

impl AppError {
    fn not_found(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: msg.into(),
        }
    }

    fn internal(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: msg.into(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = Json(ErrorResponse {
            error: self.message,
        });
        (self.status, body).into_response()
    }
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

pub async fn serve(config: &AppConfig) -> anyhow::Result<()> {
    let start = Instant::now();

    tracing::info!(model = %config.model, "loading model");
    let mut engine = EmbeddingEngine::new(config.model, Some(config.model_cache_dir()))?;

    let sets_dir = config.resolve_sets_dir();
    tracing::info!(dir = %sets_dir.display(), "loading reference sets");
    let sets = load_all_reference_sets(&sets_dir, &mut engine, Some(&config.cache_dir))?;
    tracing::info!(count = sets.len(), "reference sets loaded");

    let state = Arc::new(AppState {
        engine: Mutex::new(engine),
        sets: RwLock::new(sets),
        start_time: start,
        model: config.model,
        sets_dir: sets_dir.clone(),
        cache_dir: config.cache_dir.clone(),
    });

    // Start file watcher for hot-reload
    let _watcher = crate::watcher::start_watcher(state.clone())?;

    let app = Router::new()
        .route("/classify", post(handle_classify))
        .route("/embed", post(handle_embed))
        .route("/similarity", post(handle_similarity))
        .route("/health", get(handle_health))
        .route("/sets", get(handle_sets))
        .with_state(state);

    let addr = format!("127.0.0.1:{}", config.port);
    tracing::info!(%addr, elapsed = ?start.elapsed(), "server ready");
    let listener = tokio::net::TcpListener::bind(&addr).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::AddrInUse {
            anyhow::anyhow!(
                "port {} is already in use — is another csn instance running?",
                config.port
            )
        } else {
            anyhow::anyhow!("failed to bind {}: {}", addr, e)
        }
    })?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("server shut down");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to register SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => tracing::info!("received SIGINT"),
            _ = sigterm.recv() => tracing::info!("received SIGTERM"),
        }
    }
    #[cfg(not(unix))]
    {
        ctrl_c.await.ok();
        tracing::info!("received SIGINT");
    }
}

async fn handle_classify(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ClassifyRequest>,
) -> Result<Json<classifier::ClassifyResult>, AppError> {
    let sets = state.sets.read().unwrap();
    let set = sets
        .iter()
        .find(|s| s.metadata.name == req.reference_set)
        .ok_or_else(|| {
            let available: Vec<_> = sets.iter().map(|s| s.metadata.name.as_str()).collect();
            AppError::not_found(format!(
                "reference set '{}' not found. Available: {}",
                req.reference_set,
                available.join(", ")
            ))
        })?;

    let mut engine = state.engine.lock().unwrap();
    let result = classifier::classify_text(&mut engine, &req.text, set)
        .map_err(|e| AppError::internal(e.to_string()))?;

    Ok(Json(result))
}

async fn handle_embed(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EmbedRequest>,
) -> Result<Json<EmbedResponse>, AppError> {
    let mut engine = state.engine.lock().unwrap();
    let embedding = engine
        .embed_one(&req.text)
        .map_err(|e| AppError::internal(e.to_string()))?;
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
) -> Result<Json<SimilarityResponse>, AppError> {
    let mut engine = state.engine.lock().unwrap();
    let embeddings = engine
        .embed(&[req.a.as_str(), req.b.as_str()])
        .map_err(|e| AppError::internal(e.to_string()))?;

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

    let sets = state.sets.read().unwrap();
    Json(HealthResponse {
        status: "ok",
        model: state.model.to_string(),
        sets: sets.len(),
        uptime: uptime_str,
    })
}

async fn handle_sets(State(state): State<Arc<AppState>>) -> Json<Vec<SetInfo>> {
    let sets = state.sets.read().unwrap();
    let infos = sets
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
    Json(infos)
}
