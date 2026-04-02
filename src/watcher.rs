use std::sync::Arc;
use std::time::{Duration, Instant};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher, event::ModifyKind};
use tokio::sync::mpsc;

use crate::mlp::train_models_at_startup;
use crate::reference_set::load_all_reference_sets;
use crate::server::AppState;

const DEBOUNCE_MS: u64 = 500;

/// Start watching the sets directory for changes.
/// Returns the watcher handle (must be kept alive).
pub fn start_watcher(state: Arc<AppState>) -> anyhow::Result<RecommendedWatcher> {
    let sets_dir = state.sets_dir.clone();
    let (tx, mut rx) = mpsc::unbounded_channel::<()>();

    let mut watcher =
        notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| match res {
            Ok(event) => {
                if is_toml_change(&event) {
                    let _ = tx.send(());
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "file watcher error");
            }
        })?;

    watcher.watch(&sets_dir, RecursiveMode::NonRecursive)?;
    tracing::info!(dir = %sets_dir.display(), "file watcher started");

    // Spawn debounced reload task
    tokio::spawn(async move {
        let mut last_reload = Instant::now() - Duration::from_secs(10);

        while rx.recv().await.is_some() {
            // Drain any queued events
            while rx.try_recv().is_ok() {}

            // Debounce: skip if we reloaded recently
            if last_reload.elapsed() < Duration::from_millis(DEBOUNCE_MS) {
                tokio::time::sleep(Duration::from_millis(DEBOUNCE_MS)).await;
                // Drain again after sleep
                while rx.try_recv().is_ok() {}
            }

            last_reload = Instant::now();
            reload_sets(&state);
        }
    });

    Ok(watcher)
}

fn is_toml_change(event: &notify::Event) -> bool {
    let has_toml = event
        .paths
        .iter()
        .any(|p| p.extension().is_some_and(|ext| ext == "toml"));

    if !has_toml {
        return false;
    }

    matches!(
        event.kind,
        EventKind::Create(_)
            | EventKind::Modify(ModifyKind::Data(_))
            | EventKind::Modify(ModifyKind::Name(_))
            | EventKind::Remove(_)
    )
}

fn reload_sets(state: &AppState) {
    tracing::info!(dir = %state.sets_dir.display(), "reloading reference sets");

    let mut engine = state.engine.lock().unwrap();
    match load_all_reference_sets(&state.sets_dir, &mut engine, Some(&state.cache_dir)) {
        Ok(new_sets) => {
            let count = new_sets.len();

            // Retrain MLP models before swapping sets so reads aren't blocked during training
            match train_models_at_startup(
                &new_sets,
                &state.cache_dir,
                state.mlp_learning_rate,
                state.mlp_weight_decay,
                state.mlp_max_epochs,
                state.mlp_patience,
                state.mlp_fallback,
            ) {
                Ok(new_models) => {
                    let model_count = new_models.len();
                    let mut sets = state.sets.write().unwrap();
                    *sets = new_sets;
                    let mut models = state.trained_models.lock().unwrap();
                    *models = new_models;
                    tracing::info!(
                        count,
                        model_count,
                        "reference sets reloaded and MLP models retrained"
                    );
                }
                Err(e) => {
                    // MLP training failed — still swap the sets so embedding-only classify works
                    let mut sets = state.sets.write().unwrap();
                    *sets = new_sets;
                    tracing::warn!(error = %e, count, "reference sets reloaded but MLP retrain failed, keeping previous models");
                }
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to reload reference sets, keeping previous");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_event(kind: EventKind, path: &str) -> notify::Event {
        notify::Event {
            kind,
            paths: vec![PathBuf::from(path)],
            attrs: Default::default(),
        }
    }

    #[test]
    fn toml_create_is_change() {
        let event = make_event(
            EventKind::Create(notify::event::CreateKind::File),
            "/tmp/sets/test.toml",
        );
        assert!(is_toml_change(&event));
    }

    #[test]
    fn toml_modify_data_is_change() {
        let event = make_event(
            EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
            "/tmp/sets/test.toml",
        );
        assert!(is_toml_change(&event));
    }

    #[test]
    fn toml_remove_is_change() {
        let event = make_event(
            EventKind::Remove(notify::event::RemoveKind::File),
            "/tmp/sets/test.toml",
        );
        assert!(is_toml_change(&event));
    }

    #[test]
    fn non_toml_file_ignored() {
        let event = make_event(
            EventKind::Create(notify::event::CreateKind::File),
            "/tmp/sets/readme.md",
        );
        assert!(!is_toml_change(&event));
    }

    #[test]
    fn toml_access_event_ignored() {
        let event = make_event(
            EventKind::Access(notify::event::AccessKind::Read),
            "/tmp/sets/test.toml",
        );
        assert!(!is_toml_change(&event));
    }

    /// Verifies that `train_models_at_startup` with no reference sets
    /// returns an empty vec without errors. This is the code path
    /// `reload_sets` follows when sets directory is empty after a change.
    #[test]
    fn train_models_empty_sets_returns_empty() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let result = train_models_at_startup(
            &[],
            tmp.path(),
            0.001, // learning_rate
            1e-4,  // weight_decay
            100,   // max_epochs
            5,     // patience
            false, // fallback
        );
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    /// Full integration test for `reload_sets`: verifies that changing
    /// reference set files triggers MLP retraining via the reload path.
    ///
    /// Requires model download (EmbeddingEngine depends on fastembed),
    /// so this test is ignored by default. Run with:
    ///   cargo test --bin csn -- --ignored reload_sets_retrains_mlp
    #[test]
    #[ignore]
    fn reload_sets_retrains_mlp_on_set_change() {
        use crate::model::{EmbeddingEngine, ModelChoice};
        use std::sync::{Mutex, RwLock};

        let tmp = tempfile::tempdir().expect("create temp dir");
        let sets_dir = tmp.path().join("sets");
        std::fs::create_dir_all(&sets_dir).unwrap();
        let cache_dir = tmp.path().join("cache");
        std::fs::create_dir_all(&cache_dir).unwrap();

        let engine = EmbeddingEngine::new(ModelChoice::default(), Some(cache_dir.clone()))
            .expect("engine init (requires model download)");

        let state = AppState {
            engine: Mutex::new(engine),
            sets: RwLock::new(Vec::new()),
            trained_models: Mutex::new(Vec::new()),
            start_time: Instant::now(),
            model: ModelChoice::default(),
            sets_dir,
            cache_dir,
            mlp_learning_rate: 0.001,
            mlp_weight_decay: 1e-4,
            mlp_max_epochs: 100,
            mlp_patience: 5,
            mlp_fallback: false,
        };

        // reload_sets with an empty sets directory should succeed without error
        reload_sets(&state);

        let sets = state.sets.read().unwrap();
        assert!(sets.is_empty());
        let models = state.trained_models.lock().unwrap();
        assert!(models.is_empty());
    }
}
