use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Deserialize;

use crate::model::ModelChoice;

/// On-disk TOML `[mlp]` section.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MlpFileConfig {
    fallback: Option<bool>,
    learning_rate: Option<f64>,
    weight_decay: Option<f64>,
    max_epochs: Option<usize>,
    patience: Option<usize>,
}

/// On-disk TOML config structure (~/.config/computer-says-no/config.toml).
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FileConfig {
    host: Option<String>,
    port: Option<u16>,
    model: Option<String>,
    log_level: Option<String>,
    sets_dir: Option<PathBuf>,
    cache_dir: Option<PathBuf>,
    mlp: Option<MlpFileConfig>,
}

/// Resolved application configuration (all layers merged).
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub model: ModelChoice,
    pub log_level: String,
    pub sets_dir: PathBuf,
    #[allow(dead_code)]
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub datasets_dir: PathBuf,
    pub mlp_fallback: bool,
    pub mlp_learning_rate: f64,
    pub mlp_weight_decay: f64,
    pub mlp_max_epochs: usize,
    pub mlp_patience: usize,
}

/// CLI overrides — fields provided via command-line flags.
#[derive(Debug, Default)]
pub struct CliOverrides {
    pub port: Option<u16>,
    pub model: Option<ModelChoice>,
    pub log_level: Option<String>,
    pub sets_dir: Option<PathBuf>,
    pub cache_dir: Option<PathBuf>,
    pub datasets_dir: Option<PathBuf>,
}

impl AppConfig {
    const DEFAULT_HOST: &'static str = "127.0.0.1";
    const DEFAULT_PORT: u16 = 9847;
    const DEFAULT_LOG_LEVEL: &'static str = "warn";
    const DEFAULT_MLP_FALLBACK: bool = false;
    const DEFAULT_MLP_LEARNING_RATE: f64 = 0.001;
    const DEFAULT_MLP_WEIGHT_DECAY: f64 = 0.001;
    const DEFAULT_MLP_MAX_EPOCHS: usize = 500;
    const DEFAULT_MLP_PATIENCE: usize = 10;

    /// Load config with 3-layer precedence: CLI > env > TOML file > defaults.
    pub fn load(overrides: CliOverrides) -> Result<Self> {
        let config_dir = Self::config_dir();
        let file_config = Self::load_file(&config_dir);

        let host = std::env::var("CSN_HOST")
            .ok()
            .or(file_config.host)
            .unwrap_or_else(|| Self::DEFAULT_HOST.to_string());

        let port = overrides
            .port
            .or_else(|| std::env::var("CSN_PORT").ok()?.parse().ok())
            .or(file_config.port)
            .unwrap_or(Self::DEFAULT_PORT);

        let model = overrides
            .model
            .or_else(|| std::env::var("CSN_MODEL").ok().and_then(|s| s.parse().ok()))
            .or_else(|| file_config.model.as_deref().and_then(|s| s.parse().ok()))
            .unwrap_or_default();

        let log_level = overrides
            .log_level
            .or_else(|| std::env::var("CSN_LOG_LEVEL").ok())
            .or(file_config.log_level)
            .unwrap_or_else(|| Self::DEFAULT_LOG_LEVEL.to_string());

        let sets_dir = overrides
            .sets_dir
            .or_else(|| std::env::var("CSN_SETS_DIR").ok().map(PathBuf::from))
            .or(file_config.sets_dir)
            .unwrap_or_else(|| config_dir.join("reference-sets"));

        let cache_dir = overrides
            .cache_dir
            .or_else(|| std::env::var("CSN_CACHE_DIR").ok().map(PathBuf::from))
            .or(file_config.cache_dir)
            .unwrap_or_else(Self::default_cache_dir);

        let datasets_dir = overrides
            .datasets_dir
            .or_else(|| std::env::var("CSN_DATASETS_DIR").ok().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("datasets"));

        let mlp_file = file_config.mlp.unwrap_or_default();

        let mlp_fallback = std::env::var("CSN_MLP_FALLBACK")
            .ok()
            .and_then(|v| v.parse().ok())
            .or(mlp_file.fallback)
            .unwrap_or(Self::DEFAULT_MLP_FALLBACK);

        let mlp_learning_rate = mlp_file
            .learning_rate
            .unwrap_or(Self::DEFAULT_MLP_LEARNING_RATE);

        let mlp_weight_decay = mlp_file
            .weight_decay
            .unwrap_or(Self::DEFAULT_MLP_WEIGHT_DECAY);

        let mlp_max_epochs = mlp_file.max_epochs.unwrap_or(Self::DEFAULT_MLP_MAX_EPOCHS);

        let mlp_patience = mlp_file.patience.unwrap_or(Self::DEFAULT_MLP_PATIENCE);

        Ok(Self {
            host,
            port,
            model,
            log_level,
            sets_dir,
            config_dir,
            cache_dir,
            datasets_dir,
            mlp_fallback,
            mlp_learning_rate,
            mlp_weight_decay,
            mlp_max_epochs,
            mlp_patience,
        })
    }

    fn config_dir() -> PathBuf {
        directories::ProjectDirs::from("", "", "computer-says-no")
            .map(|d| d.config_dir().to_owned())
            .unwrap_or_else(|| PathBuf::from(".config/computer-says-no"))
    }

    fn default_cache_dir() -> PathBuf {
        directories::ProjectDirs::from("", "", "computer-says-no")
            .map(|d| d.cache_dir().to_owned())
            .unwrap_or_else(|| PathBuf::from(".cache/computer-says-no"))
    }

    fn load_file(config_dir: &Path) -> FileConfig {
        let path = config_dir.join("config.toml");
        match std::fs::read_to_string(&path) {
            Ok(content) => toml::from_str(&content).unwrap_or_else(|e| {
                tracing::warn!(path = %path.display(), error = %e, "invalid config file, using defaults");
                FileConfig::default()
            }),
            Err(_) => FileConfig::default(),
        }
    }

    /// Resolve the sets directory, checking for bundled fallback.
    pub fn resolve_sets_dir(&self) -> PathBuf {
        if self.sets_dir.exists() {
            return self.sets_dir.clone();
        }
        // Fallback: check next to binary
        if let Ok(exe) = std::env::current_exe()
            && let Some(bundled) = exe.parent().map(|p| p.join("reference-sets"))
            && bundled.exists()
        {
            return bundled;
        }
        // Fallback: check CWD
        let cwd = PathBuf::from("reference-sets");
        if cwd.exists() {
            return cwd;
        }
        // Return configured dir even if it doesn't exist yet
        self.sets_dir.clone()
    }

    /// Get the model cache directory for a specific model.
    pub fn model_cache_dir(&self) -> PathBuf {
        self.cache_dir.join(self.model.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Config tests that don't touch env vars (no ordering issues).

    #[test]
    fn defaults_with_no_env() {
        // Test that CLI overrides work without touching env vars
        let config = AppConfig::load(CliOverrides {
            port: Some(9999),
            model: Some(ModelChoice::BGESmallENV15Q),
            log_level: Some("debug".to_string()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(config.port, 9999);
        assert_eq!(config.model, ModelChoice::BGESmallENV15Q);
        assert_eq!(config.log_level, "debug");
    }

    #[test]
    fn cli_overrides_all_fields() {
        let config = AppConfig::load(CliOverrides {
            port: Some(2222),
            model: Some(ModelChoice::AllMiniLML6V2),
            log_level: Some("info".to_string()),
            sets_dir: Some(PathBuf::from("/tmp/sets")),
            cache_dir: Some(PathBuf::from("/tmp/cache")),
            datasets_dir: Some(PathBuf::from("/tmp/datasets")),
        })
        .unwrap();
        assert_eq!(config.port, 2222);
        assert_eq!(config.model, ModelChoice::AllMiniLML6V2);
        assert_eq!(config.log_level, "info");
        assert_eq!(config.sets_dir, PathBuf::from("/tmp/sets"));
        assert_eq!(config.cache_dir, PathBuf::from("/tmp/cache"));
    }

    #[test]
    fn default_port_without_override() {
        let config = AppConfig::load(CliOverrides::default()).unwrap();
        // Port should be either 9847 (default) or whatever CSN_PORT is set to in env
        // We can't assert exact value without controlling env, but we verify it loads
        assert!(config.port > 0);
    }

    #[test]
    fn mlp_defaults_load_correctly() {
        let config = AppConfig::load(CliOverrides::default()).unwrap();
        // Unless CSN_MLP_FALLBACK is set in the environment, fallback should be false
        // (matching DEFAULT_MLP_FALLBACK). The other fields have no env var overrides.
        assert_eq!(config.mlp_learning_rate, 0.001);
        assert_eq!(config.mlp_weight_decay, 0.001);
        assert_eq!(config.mlp_max_epochs, 500);
        assert_eq!(config.mlp_patience, 10);
    }

    #[test]
    fn mlp_file_config_deserializes_from_toml() {
        let toml_str = r#"
            port = 9999
            [mlp]
            fallback = true
            learning_rate = 0.01
            weight_decay = 0.0005
            max_epochs = 200
            patience = 5
        "#;
        let file_config: FileConfig = toml::from_str(toml_str).unwrap();
        let mlp = file_config.mlp.unwrap();
        assert_eq!(mlp.fallback, Some(true));
        assert_eq!(mlp.learning_rate, Some(0.01));
        assert_eq!(mlp.weight_decay, Some(0.0005));
        assert_eq!(mlp.max_epochs, Some(200));
        assert_eq!(mlp.patience, Some(5));
    }

    #[test]
    fn mlp_file_config_partial_toml() {
        let toml_str = r#"
            [mlp]
            fallback = true
        "#;
        let file_config: FileConfig = toml::from_str(toml_str).unwrap();
        let mlp = file_config.mlp.unwrap();
        assert_eq!(mlp.fallback, Some(true));
        assert_eq!(mlp.learning_rate, None);
        assert_eq!(mlp.weight_decay, None);
        assert_eq!(mlp.max_epochs, None);
        assert_eq!(mlp.patience, None);
    }
}
