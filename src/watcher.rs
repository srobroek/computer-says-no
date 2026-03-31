use std::sync::Arc;
use std::time::{Duration, Instant};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher, event::ModifyKind};
use tokio::sync::mpsc;

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
            let mut sets = state.sets.write().unwrap();
            *sets = new_sets;
            tracing::info!(count, "reference sets reloaded");
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
}
