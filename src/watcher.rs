use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher, event::ModifyKind};
use tokio::sync::mpsc;

use crate::model::EmbeddingEngine;
use crate::reference_set::{ReferenceSet, load_all_reference_sets};
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
    let dominated_by_toml = event
        .paths
        .iter()
        .any(|p| p.extension().is_some_and(|ext| ext == "toml"));

    if !dominated_by_toml {
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
    match load_all_reference_sets(&state.sets_dir, &mut engine) {
        Ok(new_sets) => {
            let count = new_sets.len();
            let mut sets = state.sets.write().unwrap();
            *sets = new_sets;
            tracing::info!(count, "reference sets reloaded");
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to reload reference sets, keeping previous");
        }
    }
}
