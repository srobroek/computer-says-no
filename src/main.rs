mod classifier;
mod config;
mod model;
mod reference_set;
mod server;
mod watcher;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::config::{AppConfig, CliOverrides};
use crate::model::{EmbeddingEngine, ModelChoice, cosine_similarity};
use crate::reference_set::load_all_reference_sets;

/// Local embedding service for text classification.
#[derive(Parser)]
#[command(name = "csn", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Classify text against a reference set
    Classify {
        /// Text to classify
        text: String,

        /// Reference set name
        #[arg(short, long)]
        set: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Model to use
        #[arg(short, long)]
        model: Option<ModelChoice>,

        /// Path to reference sets directory
        #[arg(long)]
        sets_dir: Option<PathBuf>,

        /// Classify in-process without connecting to daemon
        #[arg(long)]
        standalone: bool,
    },

    /// Generate embedding vector for text
    Embed {
        /// Text to embed
        text: String,

        /// Model to use
        #[arg(short, long)]
        model: Option<ModelChoice>,

        /// Embed in-process without connecting to daemon
        #[arg(long)]
        standalone: bool,
    },

    /// Compute cosine similarity between two texts
    Similarity {
        /// First text
        a: String,

        /// Second text
        b: String,

        /// Model to use
        #[arg(short, long)]
        model: Option<ModelChoice>,

        /// Compute in-process without connecting to daemon
        #[arg(long)]
        standalone: bool,
    },

    /// List available embedding models
    Models,

    /// Start the daemon server
    Serve {
        /// Port to listen on
        #[arg(short, long)]
        port: Option<u16>,

        /// Model to use
        #[arg(short, long)]
        model: Option<ModelChoice>,

        /// Path to reference sets directory
        #[arg(long)]
        sets_dir: Option<PathBuf>,

        /// Log level (trace, debug, info, warn, error)
        #[arg(long)]
        log_level: Option<String>,
    },

    /// Manage reference sets
    Sets {
        #[command(subcommand)]
        action: SetsAction,
    },
}

#[derive(Subcommand)]
enum SetsAction {
    /// List loaded reference sets
    List {
        /// Path to reference sets directory
        #[arg(long)]
        sets_dir: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Classify {
            text,
            set,
            json,
            model,
            sets_dir,
            standalone,
        } => {
            let config = AppConfig::load(CliOverrides {
                model,
                sets_dir,
                ..Default::default()
            })?;
            init_tracing(&config.log_level);

            if standalone {
                cmd_classify_standalone(&config, &text, &set, json)
            } else {
                cmd_classify_remote(&config, &text, &set, json)
            }
        }

        Command::Embed {
            text,
            model,
            standalone,
        } => {
            let config = AppConfig::load(CliOverrides {
                model,
                ..Default::default()
            })?;
            init_tracing(&config.log_level);

            if standalone {
                cmd_embed_standalone(&config, &text)
            } else {
                cmd_embed_remote(&config, &text)
            }
        }

        Command::Similarity {
            a,
            b,
            model,
            standalone,
        } => {
            let config = AppConfig::load(CliOverrides {
                model,
                ..Default::default()
            })?;
            init_tracing(&config.log_level);

            if standalone {
                cmd_similarity_standalone(&config, &a, &b)
            } else {
                cmd_similarity_remote(&config, &a, &b)
            }
        }

        Command::Serve {
            port,
            model,
            sets_dir,
            log_level,
        } => {
            let config = AppConfig::load(CliOverrides {
                port,
                model,
                sets_dir,
                log_level,
                ..Default::default()
            })?;
            init_tracing(&config.log_level);

            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(server::serve(&config))
        }

        Command::Models => cmd_models(),

        Command::Sets { action } => match action {
            SetsAction::List { sets_dir } => {
                let config = AppConfig::load(CliOverrides {
                    sets_dir,
                    ..Default::default()
                })?;
                init_tracing(&config.log_level);
                cmd_sets_list(&config)
            }
        },
    }
}

fn init_tracing(log_level: &str) {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level)),
        )
        .init();
}

// --- Remote (daemon) commands ---

fn daemon_url(config: &AppConfig, path: &str) -> String {
    format!("http://127.0.0.1:{}{}", config.port, path)
}

fn check_daemon(config: &AppConfig) -> Result<reqwest::blocking::Client> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    client
        .get(daemon_url(config, "/health"))
        .send()
        .with_context(|| {
            format!(
                "daemon not reachable at 127.0.0.1:{}. Start it with `csn serve` or use --standalone",
                config.port
            )
        })?;

    Ok(client)
}

fn cmd_classify_remote(config: &AppConfig, text: &str, set_name: &str, json: bool) -> Result<()> {
    let client = check_daemon(config)?;

    let resp = client
        .post(daemon_url(config, "/classify"))
        .json(&serde_json::json!({"text": text, "reference_set": set_name}))
        .send()?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body: serde_json::Value = resp.json().unwrap_or_default();
        let msg = body["error"].as_str().unwrap_or("unknown error");
        anyhow::bail!("classify failed ({}): {}", status, msg);
    }

    let result: classifier::ClassifyResult = resp.json()?;
    print_classify_result(&result, json);
    Ok(())
}

fn cmd_embed_remote(config: &AppConfig, text: &str) -> Result<()> {
    let client = check_daemon(config)?;

    let resp = client
        .post(daemon_url(config, "/embed"))
        .json(&serde_json::json!({"text": text}))
        .send()?;

    if !resp.status().is_success() {
        let body: serde_json::Value = resp.json().unwrap_or_default();
        anyhow::bail!(
            "embed failed: {}",
            body["error"].as_str().unwrap_or("unknown error")
        );
    }

    let body: serde_json::Value = resp.json()?;
    println!("{}", serde_json::to_string_pretty(&body)?);
    Ok(())
}

fn cmd_similarity_remote(config: &AppConfig, a: &str, b: &str) -> Result<()> {
    let client = check_daemon(config)?;

    let resp = client
        .post(daemon_url(config, "/similarity"))
        .json(&serde_json::json!({"a": a, "b": b}))
        .send()?;

    if !resp.status().is_success() {
        let body: serde_json::Value = resp.json().unwrap_or_default();
        anyhow::bail!(
            "similarity failed: {}",
            body["error"].as_str().unwrap_or("unknown error")
        );
    }

    let body: serde_json::Value = resp.json()?;
    let sim = body["similarity"].as_f64().unwrap_or(0.0);
    println!("{:.4}", sim);
    Ok(())
}

// --- Standalone (in-process) commands ---

fn cmd_classify_standalone(
    config: &AppConfig,
    text: &str,
    set_name: &str,
    json: bool,
) -> Result<()> {
    let sets_dir = config.resolve_sets_dir();
    let mut engine = EmbeddingEngine::new(config.model, Some(config.model_cache_dir()))?;
    let sets = load_all_reference_sets(&sets_dir, &mut engine)?;

    let reference_set = sets
        .iter()
        .find(|s| s.metadata.name == set_name)
        .with_context(|| {
            let available: Vec<_> = sets.iter().map(|s| s.metadata.name.as_str()).collect();
            format!(
                "reference set '{}' not found. Available: {}",
                set_name,
                available.join(", ")
            )
        })?;

    let result = classifier::classify_text(&mut engine, text, reference_set)?;
    print_classify_result(&result, json);
    Ok(())
}

fn cmd_embed_standalone(config: &AppConfig, text: &str) -> Result<()> {
    let mut engine = EmbeddingEngine::new(config.model, Some(config.model_cache_dir()))?;
    let embedding = engine.embed_one(text)?;
    let output = serde_json::json!({
        "embedding": embedding,
        "dimensions": embedding.len(),
        "model": config.model.as_str(),
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn cmd_similarity_standalone(config: &AppConfig, a: &str, b: &str) -> Result<()> {
    let mut engine = EmbeddingEngine::new(config.model, Some(config.model_cache_dir()))?;
    let embeddings = engine.embed(&[a, b])?;
    let sim = cosine_similarity(&embeddings[0], &embeddings[1]);
    println!("{:.4}", sim);
    Ok(())
}

// --- Local commands (no daemon needed) ---

fn cmd_models() -> Result<()> {
    println!("{:<30} {:>6}", "MODEL", "DIM");
    println!("{}", "-".repeat(38));
    for m in ModelChoice::all() {
        println!("{:<30} {:>6}", m.as_str(), m.dimensions());
    }
    Ok(())
}

fn cmd_sets_list(config: &AppConfig) -> Result<()> {
    let dir = config.resolve_sets_dir();

    if !dir.exists() {
        println!("No reference sets directory at {}", dir.display());
        return Ok(());
    }

    let mut engine = EmbeddingEngine::new(ModelChoice::BGESmallENV15Q, None)?;
    let sets = load_all_reference_sets(&dir, &mut engine)?;

    if sets.is_empty() {
        println!("No reference sets found in {}", dir.display());
        return Ok(());
    }

    println!("{:<20} {:<16} {:>8}", "NAME", "MODE", "PHRASES");
    println!("{}", "-".repeat(46));
    for s in &sets {
        let mode = match &s.kind {
            reference_set::ReferenceSetKind::Binary(_) => "binary",
            reference_set::ReferenceSetKind::MultiCategory(_) => "multi-category",
        };
        println!(
            "{:<20} {:<16} {:>8}",
            s.metadata.name,
            mode,
            s.phrase_count()
        );
    }

    Ok(())
}

// --- Output formatting ---

fn print_classify_result(result: &classifier::ClassifyResult, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(result).expect("serialization failed")
        );
    } else {
        match result {
            classifier::ClassifyResult::Binary(r) => {
                let status = if r.is_match { "MATCH" } else { "no match" };
                println!("{status} (confidence: {:.2})", r.confidence);
                println!("  top phrase: {}", r.top_phrase);
                println!(
                    "  scores: positive={:.2}, negative={:.2}",
                    r.scores.positive, r.scores.negative
                );
            }
            classifier::ClassifyResult::MultiCategory(r) => {
                let status = if r.is_match { "MATCH" } else { "no match" };
                println!("{status}: {} (confidence: {:.2})", r.category, r.confidence);
                for cs in &r.all_scores {
                    println!("  {}: {:.2} ({})", cs.category, cs.score, cs.top_phrase);
                }
            }
        }
    }
}
