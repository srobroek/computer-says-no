mod benchmark;
mod classifier;
mod config;
mod dataset;
mod embedding_cache;
#[allow(clippy::enum_variant_names)]
mod mcp;
mod mlp;
mod model;
mod reference_set;

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
    },

    /// Generate embedding vector for text
    Embed {
        /// Text to embed
        text: String,

        /// Model to use
        #[arg(short, long)]
        model: Option<ModelChoice>,
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
    },

    /// List available embedding models
    Models,

    /// Run as MCP server over stdio (for Claude Code, Cursor, etc.)
    Mcp,

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

    /// Compare scoring strategies on a dataset
    CompareStrategies {
        /// Model to test with
        #[arg(long, default_value = "bge-small-en-v1.5-Q")]
        model: ModelChoice,

        /// Dataset to test
        #[arg(long)]
        dataset: String,

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
        } => {
            let config = AppConfig::load(CliOverrides {
                model,
                sets_dir,
                ..Default::default()
            })?;
            init_tracing(&config.log_level);
            cmd_classify(&config, &text, &set, json)
        }

        Command::Embed { text, model } => {
            let config = AppConfig::load(CliOverrides {
                model,
                ..Default::default()
            })?;
            init_tracing(&config.log_level);
            cmd_embed(&config, &text)
        }

        Command::Similarity { a, b, model } => {
            let config = AppConfig::load(CliOverrides {
                model,
                ..Default::default()
            })?;
            init_tracing(&config.log_level);
            cmd_similarity(&config, &a, &b)
        }

        Command::Models => cmd_models(),

        Command::Mcp => {
            let config = AppConfig::load(CliOverrides::default())?;
            init_tracing(&config.log_level);
            cmd_mcp(&config)
        }

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
            BenchmarkAction::CompareStrategies {
                model,
                dataset,
                datasets_dir,
            } => {
                let config = AppConfig::load(CliOverrides {
                    datasets_dir,
                    ..Default::default()
                })?;
                init_tracing(&config.log_level);
                cmd_compare_strategies(&config, model, &dataset)
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

// --- In-process commands ---

fn cmd_classify(config: &AppConfig, text: &str, set_name: &str, json: bool) -> Result<()> {
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

    eprintln!("Loading MLP models (training or loading from cache)...");
    let trained_models = mlp::train_models_at_startup(
        &sets,
        &config.cache_dir,
        config.mlp_learning_rate,
        config.mlp_weight_decay,
        config.mlp_max_epochs,
        config.mlp_patience,
        config.mlp_fallback,
    )?;
    eprintln!("MLP ready ({} model(s) loaded)", trained_models.len());
    let trained_model = trained_models
        .iter()
        .find(|m| m.reference_set_name == set_name);

    let result = classifier::classify_text(&mut engine, text, reference_set, trained_model)?;
    print_classify_result(&result, json);
    Ok(())
}

fn cmd_embed(config: &AppConfig, text: &str) -> Result<()> {
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

fn cmd_similarity(config: &AppConfig, a: &str, b: &str) -> Result<()> {
    let mut engine = EmbeddingEngine::new(config.model, Some(config.model_cache_dir()))?;
    let embeddings = engine.embed(&[a, b])?;
    let sim = cosine_similarity(&embeddings[0], &embeddings[1]);
    println!("{:.4}", sim);
    Ok(())
}

fn cmd_mcp(config: &AppConfig) -> Result<()> {
    use rust_mcp_sdk::mcp_server::{McpServerOptions, ToMcpServerHandler, server_runtime};
    use rust_mcp_sdk::schema::{
        Implementation, InitializeResult, ProtocolVersion, ServerCapabilities,
        ServerCapabilitiesTools,
    };
    use rust_mcp_sdk::{StdioTransport, TransportOptions};

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

    let handler = mcp::McpHandler::new(engine, sets, trained_models, config.model);

    let server_details = InitializeResult {
        server_info: Implementation {
            name: "csn".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            title: Some("Computer Says No".into()),
            description: Some("Local embedding classifier for text classification".into()),
            icons: vec![],
            website_url: None,
        },
        capabilities: ServerCapabilities {
            tools: Some(ServerCapabilitiesTools { list_changed: None }),
            ..Default::default()
        },
        protocol_version: ProtocolVersion::V2025_11_25.into(),
        instructions: None,
        meta: None,
    };

    let transport = StdioTransport::new(TransportOptions::default())
        .map_err(|e| anyhow::anyhow!("failed to create stdio transport: {e}"))?;

    let server = server_runtime::create_server(McpServerOptions {
        server_details,
        transport,
        handler: handler.to_mcp_server_handler(),
        task_store: None,
        client_task_store: None,
        message_observer: None,
    });

    eprintln!("MCP server ready (stdio)");

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        use rust_mcp_sdk::McpServer;
        server.start().await
    })
    .map_err(|e| anyhow::anyhow!("MCP server error: {e}"))
}

// --- Local commands ---

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
        output.as_deref(),
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

fn cmd_compare_strategies(
    config: &AppConfig,
    model_choice: ModelChoice,
    dataset_name: &str,
) -> Result<()> {
    let ds_path = config.datasets_dir.join(format!("{dataset_name}.json"));
    let ds = dataset::load_dataset(&ds_path)?;

    let sets_dir = config.resolve_sets_dir();
    let mut engine = model::EmbeddingEngine::new(
        model_choice,
        Some(config.cache_dir.join(model_choice.as_str())),
    )?;
    let sets =
        reference_set::load_all_reference_sets(&sets_dir, &mut engine, Some(&config.cache_dir))?;

    let ref_set = sets
        .iter()
        .find(|s| s.metadata.name == ds.reference_set)
        .with_context(|| format!("reference set '{}' not found", ds.reference_set))?;

    println!(
        "Comparing strategies: {} × {} ({} prompts)\n",
        model_choice,
        dataset_name,
        ds.prompts.len()
    );

    let results = benchmark::compare_strategies(&mut engine, ref_set, &ds);

    println!("{:<20} {:>10}", "Strategy", "Accuracy");
    println!("{}", "-".repeat(32));
    for (name, accuracy) in &results {
        println!("{:<20} {:>9.1}%", name, accuracy * 100.0);
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
