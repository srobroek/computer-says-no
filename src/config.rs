use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Deserialize;

use crate::model::ModelChoice;

/// On-disk TOML config structure (~/.config/computer-says-no/config.toml).
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct FileConfig {
    port: Option<u16>,
    model: Option<String>,
    log_level: Option<String>,
    sets_dir: Option<PathBuf>,
    cache_dir: Option<PathBuf>,
}

/// Resolved application configuration (all layers merged).
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub port: u16,
    pub model: ModelChoice,
    pub log_level: String,
    pub sets_dir: PathBuf,
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
}

/// CLI overrides — fields provided via command-line flags.
#[derive(Debug, Default)]
pub struct CliOverrides {
    pub port: Option<u16>,
    pub model: Option<ModelChoice>,
    pub log_level: Option<String>,
    pub sets_dir: Option<PathBuf>,
    pub cache_dir: Option<PathBuf>,
}

impl AppConfig {
    const DEFAULT_PORT: u16 = 9847;
    const DEFAULT_LOG_LEVEL: &'static str = "warn";

    /// Load config with 3-layer precedence: CLI > env > TOML file > defaults.
    pub fn load(overrides: CliOverrides) -> Result<Self> {
        let config_dir = Self::config_dir();
        let file_config = Self::load_file(&config_dir);

        let port = overrides
            .port
            .or_else(|| std::env::var("CSN_PORT").ok()?.parse().ok())
            .or(file_config.port)
            .unwrap_or(Self::DEFAULT_PORT);

        let model = overrides
            .model
            .or_else(|| {
                std::env::var("CSN_MODEL")
                    .ok()
                    .and_then(|s| s.parse().ok())
            })
            .or_else(|| {
                file_config
                    .model
                    .as_deref()
                    .and_then(|s| s.parse().ok())
            })
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

        Ok(Self {
            port,
            model,
            log_level,
            sets_dir,
            config_dir,
            cache_dir,
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
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                let bundled = parent.join("reference-sets");
                if bundled.exists() {
                    return bundled;
                }
            }
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

    #[test]
    fn defaults_when_nothing_set() {
        // Clear env vars that might interfere
        std::env::remove_var("CSN_PORT");
        std::env::remove_var("CSN_MODEL");
        std::env::remove_var("CSN_LOG_LEVEL");
        std::env::remove_var("CSN_SETS_DIR");
        std::env::remove_var("CSN_CACHE_DIR");

        let config = AppConfig::load(CliOverrides::default()).unwrap();
        assert_eq!(config.port, 9847);
        assert_eq!(config.model, ModelChoice::default());
        assert_eq!(config.log_level, "warn");
    }

    #[test]
    fn cli_overrides_take_precedence() {
        std::env::set_var("CSN_PORT", "1111");
        let config = AppConfig::load(CliOverrides {
            port: Some(2222),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(config.port, 2222);
        std::env::remove_var("CSN_PORT");
    }

    #[test]
    fn env_var_override() {
        std::env::set_var("CSN_PORT", "3333");
        let config = AppConfig::load(CliOverrides::default()).unwrap();
        assert_eq!(config.port, 3333);
        std::env::remove_var("CSN_PORT");
    }
}
