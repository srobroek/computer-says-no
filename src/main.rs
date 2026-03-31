mod classifier;
mod model;
mod reference_set;
mod server;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
        #[arg(short, long, default_value_t = ModelChoice::default())]
        model: ModelChoice,

        /// Path to reference sets directory
        #[arg(long)]
        sets_dir: Option<PathBuf>,
    },

    /// Generate embedding vector for text
    Embed {
        /// Text to embed
        text: String,

        /// Model to use
        #[arg(short, long, default_value_t = ModelChoice::default())]
        model: ModelChoice,
    },

    /// Compute cosine similarity between two texts
    Similarity {
        /// First text
        a: String,

        /// Second text
        b: String,

        /// Model to use
        #[arg(short, long, default_value_t = ModelChoice::default())]
        model: ModelChoice,
    },

    /// List available embedding models
    Models,

    /// Start the daemon server
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value_t = 9847)]
        port: u16,

        /// Model to use
        #[arg(short, long, default_value_t = ModelChoice::default())]
        model: ModelChoice,

        /// Path to reference sets directory
        #[arg(long)]
        sets_dir: Option<PathBuf>,
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

fn default_sets_dir() -> PathBuf {
    directories::ProjectDirs::from("", "", "computer-says-no")
        .map(|d| d.config_dir().join("reference-sets"))
        .unwrap_or_else(|| PathBuf::from("reference-sets"))
}

fn resolve_sets_dir(override_dir: Option<PathBuf>) -> PathBuf {
    override_dir.unwrap_or_else(default_sets_dir)
}

/// Find the bundled reference-sets/ directory (next to the binary or in the project).
fn find_bundled_sets_dir() -> Option<PathBuf> {
    // Check next to binary
    if let Ok(exe) = std::env::current_exe() {
        let dir = exe.parent()?.join("reference-sets");
        if dir.exists() {
            return Some(dir);
        }
    }
    // Check current working directory
    let cwd = PathBuf::from("reference-sets");
    if cwd.exists() {
        return Some(cwd);
    }
    None
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Classify {
            text,
            set,
            json,
            model,
            sets_dir,
        } => cmd_classify(&text, &set, json, model, sets_dir),

        Command::Embed { text, model } => cmd_embed(&text, model),

        Command::Similarity { a, b, model } => cmd_similarity(&a, &b, model),

        Command::Serve {
            port,
            model,
            sets_dir,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(server::serve(model, resolve_sets_dir(sets_dir), port))
        }

        Command::Models => cmd_models(),

        Command::Sets { action } => match action {
            SetsAction::List { sets_dir } => cmd_sets_list(sets_dir),
        },
    }
}

fn cmd_classify(
    text: &str,
    set_name: &str,
    json: bool,
    model: ModelChoice,
    sets_dir: Option<PathBuf>,
) -> Result<()> {
    let dir = resolve_sets_dir(sets_dir);
    let mut engine = EmbeddingEngine::new(model, None)?;
    let sets = load_all_reference_sets(&dir, &mut engine)?;

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

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        match &result {
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

    Ok(())
}

fn cmd_embed(text: &str, model: ModelChoice) -> Result<()> {
    let mut engine = EmbeddingEngine::new(model, None)?;
    let embedding = engine.embed_one(text)?;
    let output = serde_json::json!({
        "embedding": embedding,
        "dimensions": embedding.len(),
        "model": model.as_str(),
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn cmd_similarity(a: &str, b: &str, model: ModelChoice) -> Result<()> {
    let mut engine = EmbeddingEngine::new(model, None)?;
    let embeddings = engine.embed(&[a, b])?;
    let sim = cosine_similarity(&embeddings[0], &embeddings[1]);
    println!("{:.4}", sim);
    Ok(())
}

fn cmd_models() -> Result<()> {
    println!("{:<30} {:>6}", "MODEL", "DIM");
    println!("{}", "-".repeat(38));
    for m in ModelChoice::all() {
        println!("{:<30} {:>6}", m.as_str(), m.dimensions());
    }
    Ok(())
}

fn cmd_sets_list(sets_dir: Option<PathBuf>) -> Result<()> {
    let dir = resolve_sets_dir(sets_dir);

    if !dir.exists() {
        println!("No reference sets directory at {}", dir.display());
        if let Some(bundled) = find_bundled_sets_dir() {
            println!("Bundled sets available at: {}", bundled.display());
        }
        return Ok(());
    }

    // We need an engine just to parse — use the smallest model
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
