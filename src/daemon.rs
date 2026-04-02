use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::broadcast;

use crate::classifier;
use crate::config::AppConfig;
use crate::mlp::TrainedModel;
use crate::model::{EmbeddingEngine, ModelChoice, cosine_similarity};
use crate::reference_set::ReferenceSet;

// --- Wire protocol types (contracts/socket-protocol.md) ---

#[derive(Debug, Serialize, Deserialize)]
pub struct DaemonRequest {
    pub command: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DaemonResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl DaemonResponse {
    pub fn success(result: serde_json::Value) -> Self {
        Self {
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            result: None,
            error: Some(msg.into()),
        }
    }
}

// --- Idle tracker ---

#[derive(Clone)]
struct IdleTracker {
    last_request: Arc<AtomicU64>,
    timeout_secs: u64,
}

impl IdleTracker {
    fn new(timeout_secs: u64) -> Self {
        Self {
            last_request: Arc::new(AtomicU64::new(Self::now())),
            timeout_secs,
        }
    }

    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    fn touch(&self) {
        self.last_request.store(Self::now(), Ordering::Relaxed);
    }

    fn is_idle(&self) -> bool {
        Self::now().saturating_sub(self.last_request.load(Ordering::Relaxed)) >= self.timeout_secs
    }
}

// --- Daemon handler ---

struct DaemonHandler {
    engine: Mutex<EmbeddingEngine>,
    reference_sets: Vec<ReferenceSet>,
    trained_models: Mutex<Vec<TrainedModel>>,
    model_choice: ModelChoice,
}

impl DaemonHandler {
    fn new(
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

    fn dispatch(&self, req: DaemonRequest) -> DaemonResponse {
        match req.command.as_str() {
            "classify" => self.handle_classify(req.args),
            "embed" => self.handle_embed(req.args),
            "similarity" => self.handle_similarity(req.args),
            other => DaemonResponse::error(format!("unknown command: {other}")),
        }
    }

    fn handle_classify(&self, args: serde_json::Value) -> DaemonResponse {
        let text = match args.get("text").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => return DaemonResponse::error("missing 'text' field"),
        };
        let set_name = match args.get("set").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return DaemonResponse::error("missing 'set' field"),
        };

        let set = match self
            .reference_sets
            .iter()
            .find(|s| s.metadata.name == set_name)
        {
            Some(s) => s,
            None => {
                let available: Vec<_> = self
                    .reference_sets
                    .iter()
                    .map(|s| s.metadata.name.as_str())
                    .collect();
                return DaemonResponse::error(format!(
                    "reference set '{}' not found. Available: {}",
                    set_name,
                    available.join(", ")
                ));
            }
        };

        let trained_models = match self.trained_models.lock() {
            Ok(m) => m,
            Err(_) => return DaemonResponse::error("models lock poisoned"),
        };
        let trained_model = trained_models
            .iter()
            .find(|m| m.reference_set_name == set_name);

        let mut engine = match self.engine.lock() {
            Ok(e) => e,
            Err(_) => return DaemonResponse::error("engine lock poisoned"),
        };

        match classifier::classify_text(&mut engine, text, set, trained_model) {
            Ok(result) => match serde_json::to_value(&result) {
                Ok(v) => DaemonResponse::success(v),
                Err(e) => DaemonResponse::error(format!("serialization error: {e}")),
            },
            Err(e) => DaemonResponse::error(e.to_string()),
        }
    }

    fn handle_embed(&self, args: serde_json::Value) -> DaemonResponse {
        let text = match args.get("text").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => return DaemonResponse::error("missing 'text' field"),
        };

        let mut engine = match self.engine.lock() {
            Ok(e) => e,
            Err(_) => return DaemonResponse::error("engine lock poisoned"),
        };

        match engine.embed_one(text) {
            Ok(embedding) => {
                let model_name = self.model_choice.as_str();
                DaemonResponse::success(serde_json::json!({
                    "embedding": embedding,
                    "dimensions": embedding.len(),
                    "model": model_name,
                }))
            }
            Err(e) => DaemonResponse::error(e.to_string()),
        }
    }

    fn handle_similarity(&self, args: serde_json::Value) -> DaemonResponse {
        let a = match args.get("a").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => return DaemonResponse::error("missing 'a' field"),
        };
        let b = match args.get("b").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => return DaemonResponse::error("missing 'b' field"),
        };

        let mut engine = match self.engine.lock() {
            Ok(e) => e,
            Err(_) => return DaemonResponse::error("engine lock poisoned"),
        };

        match engine.embed(&[a, b]) {
            Ok(embeddings) => {
                let sim = cosine_similarity(&embeddings[0], &embeddings[1]);
                let model_name = self.model_choice.as_str();
                DaemonResponse::success(serde_json::json!({
                    "similarity": sim,
                    "model": model_name,
                }))
            }
            Err(e) => DaemonResponse::error(e.to_string()),
        }
    }
}

// SAFETY: DaemonHandler fields are protected by Mutex, making concurrent access safe.
unsafe impl Sync for DaemonHandler {}

// --- Main daemon entry point ---

pub fn run_daemon(config: &AppConfig) -> Result<()> {
    use crate::mlp;
    use crate::reference_set::load_all_reference_sets;

    let socket_path = config.socket_path();
    let pid_path = config.pid_path();

    // Ensure cache directory exists
    std::fs::create_dir_all(&config.cache_dir)
        .with_context(|| format!("creating cache dir {}", config.cache_dir.display()))?;

    // Clean up stale socket
    let _ = std::fs::remove_file(&socket_path);

    // Write PID file
    std::fs::write(&pid_path, std::process::id().to_string())
        .with_context(|| format!("writing PID file {}", pid_path.display()))?;

    eprintln!("Loading embedding model...");
    let mut engine = EmbeddingEngine::new(config.model, Some(config.model_cache_dir()))?;

    eprintln!("Loading reference sets...");
    let sets_dir = config.resolve_sets_dir();
    let sets = load_all_reference_sets(&sets_dir, &mut engine, Some(&config.cache_dir))?;
    eprintln!("{} reference set(s) loaded", sets.len());

    eprintln!("Training MLP models...");
    let trained_models = mlp::train_models_at_startup(
        &sets,
        &config.cache_dir,
        config.mlp_learning_rate,
        config.mlp_weight_decay,
        config.mlp_max_epochs,
        config.mlp_patience,
        config.mlp_fallback,
    )?;
    eprintln!("MLP ready ({} model(s))", trained_models.len());

    let handler = Arc::new(DaemonHandler::new(
        engine,
        sets,
        trained_models,
        config.model,
    ));
    let tracker = IdleTracker::new(config.idle_timeout);

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let listener = UnixListener::bind(&socket_path)
            .with_context(|| format!("binding socket {}", socket_path.display()))?;

        eprintln!("Daemon ready (socket: {})", socket_path.display());

        let (shutdown_tx, _) = broadcast::channel::<()>(1);

        // Idle timeout task
        let idle_tracker = tracker.clone();
        let idle_shutdown = shutdown_tx.clone();
        tokio::spawn(async move {
            let check_interval = Duration::from_secs(30);
            loop {
                tokio::time::sleep(check_interval).await;
                if idle_tracker.is_idle() {
                    eprintln!("Idle timeout reached, shutting down");
                    let _ = idle_shutdown.send(());
                    break;
                }
            }
        });

        // SIGTERM handler
        let term_shutdown = shutdown_tx.clone();
        tokio::spawn(async move {
            if let Ok(mut sig) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                sig.recv().await;
                eprintln!("SIGTERM received, shutting down");
                let _ = term_shutdown.send(());
            }
        });

        let mut shutdown_rx = shutdown_tx.subscribe();

        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, _)) => {
                            tracker.touch();
                            let handler = handler.clone();
                            tokio::spawn(async move {
                                let (reader, mut writer) = stream.into_split();
                                let mut lines = BufReader::new(reader).lines();
                                if let Ok(Some(line)) = lines.next_line().await {
                                    let response = match serde_json::from_str::<DaemonRequest>(&line) {
                                        Ok(req) => handler.dispatch(req),
                                        Err(e) => DaemonResponse::error(format!("invalid request: {e}")),
                                    };
                                    if let Ok(json) = serde_json::to_string(&response) {
                                        let _ = writer.write_all(json.as_bytes()).await;
                                        let _ = writer.write_all(b"\n").await;
                                    }
                                }
                            });
                        }
                        Err(e) => {
                            eprintln!("Accept error: {e}");
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    break;
                }
            }
        }

        // Cleanup
        let _ = std::fs::remove_file(&socket_path);
        let _ = std::fs::remove_file(&pid_path);
        eprintln!("Daemon stopped");

        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_request_response_roundtrip() {
        let req = DaemonRequest {
            command: "classify".to_string(),
            args: serde_json::json!({"text": "hello", "set": "corrections"}),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: DaemonRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.command, "classify");
        assert_eq!(parsed.args["text"], "hello");
        assert_eq!(parsed.args["set"], "corrections");
    }

    #[test]
    fn daemon_response_success_serialization() {
        let resp = DaemonResponse::success(serde_json::json!({"match": true, "confidence": 0.95}));
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["ok"], true);
        assert_eq!(parsed["result"]["match"], true);
        assert!(parsed.get("error").is_none());
    }

    #[test]
    fn daemon_response_error_serialization() {
        let resp = DaemonResponse::error("not found");
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["ok"], false);
        assert_eq!(parsed["error"], "not found");
        assert!(parsed.get("result").is_none());
    }

    #[test]
    fn idle_tracker_starts_not_idle() {
        let tracker = IdleTracker::new(300);
        assert!(!tracker.is_idle());
    }

    #[test]
    fn idle_tracker_with_zero_timeout_is_immediately_idle() {
        let tracker = IdleTracker::new(0);
        assert!(tracker.is_idle());
    }
}
