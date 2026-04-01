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
use crate::mlp::{TrainedModel, train_models_at_startup};
use crate::model::{EmbeddingEngine, ModelChoice, cosine_similarity};
use crate::reference_set::{ReferenceSet, load_all_reference_sets};

pub struct AppState {
    pub engine: Mutex<EmbeddingEngine>,
    pub sets: RwLock<Vec<ReferenceSet>>,
    pub trained_models: Mutex<Vec<TrainedModel>>,
    pub start_time: Instant,
    pub model: ModelChoice,
    pub sets_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub mlp_learning_rate: f64,
    pub mlp_weight_decay: f64,
    pub mlp_max_epochs: usize,
    pub mlp_patience: usize,
    pub mlp_fallback: bool,
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

    tracing::info!("training MLP classifiers");
    let trained_models = train_models_at_startup(
        &sets,
        &config.cache_dir,
        config.mlp_learning_rate,
        config.mlp_weight_decay,
        config.mlp_max_epochs,
        config.mlp_patience,
        config.mlp_fallback,
    )?;
    tracing::info!(count = trained_models.len(), "MLP classifiers ready");

    let state = Arc::new(AppState {
        engine: Mutex::new(engine),
        sets: RwLock::new(sets),
        trained_models: Mutex::new(trained_models),
        start_time: start,
        model: config.model,
        sets_dir: sets_dir.clone(),
        cache_dir: config.cache_dir.clone(),
        mlp_learning_rate: config.mlp_learning_rate,
        mlp_weight_decay: config.mlp_weight_decay,
        mlp_max_epochs: config.mlp_max_epochs,
        mlp_patience: config.mlp_patience,
        mlp_fallback: config.mlp_fallback,
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
    let sets = state
        .sets
        .read()
        .map_err(|_| AppError::internal("lock poisoned".to_string()))?;
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

    let trained_models = state
        .trained_models
        .lock()
        .map_err(|_| AppError::internal("lock poisoned".to_string()))?;
    let trained_model = trained_models
        .iter()
        .find(|m| m.reference_set_name == req.reference_set);

    let mut engine = state
        .engine
        .lock()
        .map_err(|_| AppError::internal("lock poisoned".to_string()))?;
    let result = classifier::classify_text(&mut engine, &req.text, set, trained_model)
        .map_err(|e| AppError::internal(e.to_string()))?;

    Ok(Json(result))
}

async fn handle_embed(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EmbedRequest>,
) -> Result<Json<EmbedResponse>, AppError> {
    let mut engine = state
        .engine
        .lock()
        .map_err(|_| AppError::internal("lock poisoned".to_string()))?;
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
    let mut engine = state
        .engine
        .lock()
        .map_err(|_| AppError::internal("lock poisoned".to_string()))?;
    let embeddings = engine
        .embed(&[req.a.as_str(), req.b.as_str()])
        .map_err(|e| AppError::internal(e.to_string()))?;

    let similarity = cosine_similarity(&embeddings[0], &embeddings[1]);
    Ok(Json(SimilarityResponse { similarity }))
}

async fn handle_health(
    State(state): State<Arc<AppState>>,
) -> Result<Json<HealthResponse>, AppError> {
    let uptime = state.start_time.elapsed();
    let secs = uptime.as_secs();
    let uptime_str = if secs >= 3600 {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m{}s", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    };

    let sets = state
        .sets
        .read()
        .map_err(|_| AppError::internal("lock poisoned".to_string()))?;
    Ok(Json(HealthResponse {
        status: "ok",
        model: state.model.to_string(),
        sets: sets.len(),
        uptime: uptime_str,
    }))
}

async fn handle_sets(State(state): State<Arc<AppState>>) -> Result<Json<Vec<SetInfo>>, AppError> {
    let sets = state
        .sets
        .read()
        .map_err(|_| AppError::internal("lock poisoned".to_string()))?;
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
    Ok(Json(infos))
}
