mod benchmark;
mod classifier;
mod config;
mod dataset;
mod embedding_cache;
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

    /// Run model benchmark
    Benchmark {
        #[command(subcommand)]
        action: BenchmarkAction,
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

#[derive(Subcommand)]
enum BenchmarkAction {
    /// Run benchmark across models and datasets
    Run {
        /// Test only this model
        #[arg(long)]
        model: Option<ModelChoice>,

        /// Test only this dataset
        #[arg(long)]
        dataset: Option<String>,

        /// Measured iterations per prompt (default: 20)
        #[arg(long, default_value_t = 20)]
        iterations: usize,

        /// Warm-up iterations before measuring (default: 5)
        #[arg(long, default_value_t = 5)]
        warmup: usize,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Save results to file
        #[arg(long)]
        output: Option<PathBuf>,

        /// Compare against previous run
        #[arg(long)]
        compare: Option<PathBuf>,

        /// Path to datasets directory
        #[arg(long)]
        datasets_dir: Option<PathBuf>,
    },

    /// Generate labeled test datasets from reference sets
    GenerateDatasets {
        /// Path to reference sets directory
        #[arg(long)]
        sets_dir: Option<PathBuf>,

        /// Path to output datasets directory
        #[arg(long)]
        output_dir: Option<PathBuf>,
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

        Command::Benchmark { action } => match action {
            BenchmarkAction::Run {
                model,
                dataset,
                iterations,
                warmup,
                json,
                output,
                compare,
                datasets_dir,
            } => {
                let model_filter = model;
                let config = AppConfig::load(CliOverrides {
                    datasets_dir,
                    ..Default::default()
                })?;
                init_tracing(&config.log_level);
                cmd_benchmark_run(
                    &config,
                    model_filter,
                    dataset,
                    iterations,
                    warmup,
                    json,
                    output,
                    compare,
                )
            }
            BenchmarkAction::GenerateDatasets {
                sets_dir,
                output_dir,
            } => {
                let config = AppConfig::load(CliOverrides {
                    sets_dir,
                    ..Default::default()
                })?;
                init_tracing(&config.log_level);
                cmd_generate_datasets(&config, output_dir)
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
    let sets = load_all_reference_sets(&sets_dir, &mut engine, Some(&config.cache_dir))?;

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

    let mut engine = EmbeddingEngine::new(config.model, Some(config.model_cache_dir()))?;
    let sets = load_all_reference_sets(&dir, &mut engine, Some(&config.cache_dir))?;

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

// --- Benchmark commands ---

#[allow(clippy::too_many_arguments)]
fn cmd_benchmark_run(
    config: &AppConfig,
    model_filter: Option<ModelChoice>,
    dataset_filter: Option<String>,
    iterations: usize,
    warmup: usize,
    json: bool,
    output: Option<PathBuf>,
    compare: Option<PathBuf>,
) -> Result<()> {
    let datasets_dir = &config.datasets_dir;
    let mut datasets = dataset::load_all_datasets(datasets_dir)?;

    if datasets.is_empty() {
        anyhow::bail!(
            "no datasets found in {}. Run `csn benchmark generate-datasets` first.",
            datasets_dir.display()
        );
    }

    // Apply dataset filter
    if let Some(ref filter) = dataset_filter {
        datasets.retain(|d| d.name == *filter);
        if datasets.is_empty() {
            let available: Vec<_> = dataset::load_all_datasets(datasets_dir)?
                .iter()
                .map(|d| d.name.clone())
                .collect();
            anyhow::bail!(
                "dataset '{}' not found. Available: {}",
                filter,
                available.join(", ")
            );
        }
    }

    // Determine models to test
    let models: Vec<ModelChoice> = if let Some(m) = model_filter {
        vec![m]
    } else {
        ModelChoice::all().to_vec()
    };

    let sets_dir = config.resolve_sets_dir();
    let run = benchmark::run_benchmark(
        &models,
        &datasets,
        &sets_dir,
        &config.cache_dir,
        warmup,
        iterations,
    )?;

    // Output results
    if json {
        let json_str = serde_json::to_string_pretty(&run)?;
        println!("{json_str}");
    } else {
        benchmark::print_table(&run);
    }

    // Save to file if requested
    if let Some(ref path) = output {
        let json_str = serde_json::to_string_pretty(&run)?;
        std::fs::write(path, json_str)
            .with_context(|| format!("writing results to {}", path.display()))?;
        println!("\nResults saved to {}", path.display());
    }

    // Compare against previous run
    if let Some(ref path) = compare {
        let prev_json = std::fs::read_to_string(path)
            .with_context(|| format!("reading previous results from {}", path.display()))?;
        let prev_run: benchmark::BenchmarkRun = serde_json::from_str(&prev_json)
            .with_context(|| format!("parsing previous results from {}", path.display()))?;
        benchmark::print_comparison(&run, &prev_run);
    }

    Ok(())
}

fn cmd_generate_datasets(config: &AppConfig, output_dir: Option<PathBuf>) -> Result<()> {
    let sets_dir = config.resolve_sets_dir();
    let output = output_dir.unwrap_or_else(|| config.datasets_dir.clone());

    std::fs::create_dir_all(&output)
        .with_context(|| format!("creating datasets dir {}", output.display()))?;

    // Load reference sets (need a model to parse them)
    let mut engine = model::EmbeddingEngine::new(config.model, Some(config.model_cache_dir()))?;
    let sets =
        reference_set::load_all_reference_sets(&sets_dir, &mut engine, Some(&config.cache_dir))?;

    if sets.is_empty() {
        anyhow::bail!("no reference sets found in {}", sets_dir.display());
    }

    for set in &sets {
        let mode = match &set.kind {
            reference_set::ReferenceSetKind::Binary(_) => "binary",
            reference_set::ReferenceSetKind::MultiCategory(_) => "multi-category",
        };

        // Collect seed phrases
        let seeds: Vec<String> = match &set.kind {
            reference_set::ReferenceSetKind::Binary(b) => b.positive_phrases.clone(),
            reference_set::ReferenceSetKind::MultiCategory(m) => m
                .categories
                .values()
                .flat_map(|c| c.phrases.clone())
                .collect(),
        };

        let scaffold = dataset::generate_scaffold(&set.metadata.name, mode, &seeds);
        let path = output.join(format!("{}.json", set.metadata.name));
        let json = serde_json::to_string_pretty(&scaffold)?;
        std::fs::write(&path, json).with_context(|| format!("writing {}", path.display()))?;
        println!(
            "Generated scaffold: {} ({} seed prompts)",
            path.display(),
            scaffold.prompts.len()
        );
    }

    println!(
        "\nScaffolds written to {}. Fill with LLM-generated prompts (500 per dataset).",
        output.display()
    );
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
