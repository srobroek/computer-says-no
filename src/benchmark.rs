use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use comfy_table::{Cell, Table};
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};

use crate::classifier;
use crate::dataset::{LabeledDataset, LabeledPrompt, Polarity, Tier};
use crate::model::{EmbeddingEngine, ModelChoice};
use crate::reference_set::load_all_reference_sets;

/// Configuration for a benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    pub models: Vec<String>,
    pub datasets: Vec<String>,
    pub warmup_iterations: usize,
    pub measured_iterations: usize,
}

/// Results for a single model-dataset combination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetResult {
    pub dataset: String,
    pub accuracy: f64,
    pub accuracy_by_tier: TierAccuracy,
    pub precision: f64,
    pub recall: f64,
    pub latency_p50_ms: f64,
    pub latency_p95_ms: f64,
    pub latency_p99_ms: f64,
    pub latency_cv: f64,
    pub total_prompts: usize,
    pub correct: usize,
}

/// Per-tier accuracy breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierAccuracy {
    pub clear_pos: f64,
    pub moderate_pos: f64,
    pub edge_pos: f64,
    pub clear_neg: f64,
    pub moderate_neg: f64,
    pub edge_neg: f64,
}

/// Results for a single model across all datasets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResult {
    pub model: String,
    pub dimensions: usize,
    pub cold_startup_ms: f64,
    pub datasets: Vec<DatasetResult>,
}

/// A complete benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRun {
    pub timestamp: String,
    pub config: BenchmarkConfig,
    pub results: Vec<ModelResult>,
    pub system_info: String,
}

/// Compute percentile from a sorted slice of durations.
pub fn percentile(sorted: &[Duration], pct: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let idx = (((sorted.len() - 1) as f64 * pct / 100.0) as usize).min(sorted.len() - 1);
    sorted[idx]
}

/// Compute coefficient of variation (std_dev / mean) for durations.
pub fn coefficient_of_variation(durations: &[Duration]) -> f64 {
    if durations.is_empty() {
        return 0.0;
    }
    let mean = durations.iter().map(|d| d.as_secs_f64()).sum::<f64>() / durations.len() as f64;
    if mean == 0.0 {
        return 0.0;
    }
    let variance = durations
        .iter()
        .map(|d| {
            let diff = d.as_secs_f64() - mean;
            diff * diff
        })
        .sum::<f64>()
        / durations.len() as f64;
    variance.sqrt() / mean
}

/// Check if a classification result is correct for a labeled prompt.
pub fn is_correct(
    result: &crate::classifier::ClassifyResult,
    prompt: &LabeledPrompt,
    reference_set_mode: &str,
) -> bool {
    match reference_set_mode {
        "binary" => {
            let matched = result.is_match();
            match prompt.polarity {
                Polarity::Positive => {
                    // Positive prompt: expected to match
                    if prompt.expected_label == "match" {
                        matched
                    } else {
                        !matched
                    }
                }
                Polarity::Negative => {
                    // Negative prompt: expected NOT to match
                    !matched
                }
            }
        }
        "multi-category" => {
            match prompt.polarity {
                Polarity::Positive => {
                    // Should match the expected category
                    if let crate::classifier::ClassifyResult::MultiCategory(r) = result {
                        r.is_match && r.category == prompt.expected_label
                    } else {
                        false
                    }
                }
                Polarity::Negative => {
                    // Should NOT match any category
                    !result.is_match()
                }
            }
        }
        _ => false,
    }
}

/// Compute per-tier accuracy from a list of (prompt, correct) pairs.
pub fn compute_tier_accuracy(results: &[(LabeledPrompt, bool)]) -> TierAccuracy {
    let tier_acc = |tier: Tier, pol: Polarity| -> f64 {
        let matching: Vec<_> = results
            .iter()
            .filter(|(p, _)| p.tier == tier && p.polarity == pol)
            .collect();
        if matching.is_empty() {
            return 0.0;
        }
        let correct = matching.iter().filter(|(_, c)| *c).count();
        correct as f64 / matching.len() as f64
    };

    TierAccuracy {
        clear_pos: tier_acc(Tier::Clear, Polarity::Positive),
        moderate_pos: tier_acc(Tier::Moderate, Polarity::Positive),
        edge_pos: tier_acc(Tier::Edge, Polarity::Positive),
        clear_neg: tier_acc(Tier::Clear, Polarity::Negative),
        moderate_neg: tier_acc(Tier::Moderate, Polarity::Negative),
        edge_neg: tier_acc(Tier::Edge, Polarity::Negative),
    }
}

/// Run the full benchmark across models and datasets.
pub fn run_benchmark(
    models: &[ModelChoice],
    datasets: &[LabeledDataset],
    sets_dir: &Path,
    cache_dir: &Path,
    warmup: usize,
    iterations: usize,
) -> Result<BenchmarkRun> {
    let total_steps = models.len() * datasets.len();
    let pb = ProgressBar::new(total_steps as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=> "),
    );

    let mut results = Vec::new();

    for &model in models {
        pb.set_message(format!("loading {}", model.as_str()));

        // Measure cold startup
        let cold_start = Instant::now();
        let mut engine = EmbeddingEngine::new(model, Some(cache_dir.join(model.as_str())))
            .with_context(|| format!("loading model {model}"))?;
        // Force a first embedding to complete initialization
        let _ = engine.embed_one("warmup")?;
        let cold_startup_ms = cold_start.elapsed().as_secs_f64() * 1000.0;

        // Load reference sets with this engine
        let sets = load_all_reference_sets(sets_dir, &mut engine, Some(cache_dir))?;

        let mut dataset_results = Vec::new();

        for ds in datasets {
            pb.set_message(format!("{} × {}", model.as_str(), ds.name));

            // Find matching reference set
            let ref_set = sets.iter().find(|s| s.metadata.name == ds.reference_set);
            let Some(ref_set) = ref_set else {
                tracing::warn!(
                    model = model.as_str(),
                    dataset = ds.name.as_str(),
                    "reference set '{}' not found, skipping",
                    ds.reference_set
                );
                pb.inc(1);
                continue;
            };

            // Warm-up
            for _ in 0..warmup {
                if let Some(prompt) = ds.prompts.first() {
                    let _ = classifier::classify_text(&mut engine, &prompt.text, ref_set);
                }
            }

            // Measured iterations: classify each prompt `iterations` times, collect latencies
            let mut all_latencies = Vec::new();
            let mut prompt_results: Vec<(LabeledPrompt, bool)> = Vec::new();

            for prompt in &ds.prompts {
                let mut correct = false;
                for i in 0..iterations {
                    let start = Instant::now();
                    let result = classifier::classify_text(&mut engine, &prompt.text, ref_set)
                        .with_context(|| format!("classifying '{}' with {}", prompt.text, model))?;
                    all_latencies.push(start.elapsed());

                    // Use first iteration for accuracy
                    if i == 0 {
                        correct = is_correct(&result, prompt, &ds.mode);
                    }
                }
                prompt_results.push((prompt.clone(), correct));
            }

            // Compute metrics
            all_latencies.sort();
            let total = prompt_results.len();
            let correct_count = prompt_results.iter().filter(|(_, c)| *c).count();
            let accuracy = if total > 0 {
                correct_count as f64 / total as f64
            } else {
                0.0
            };

            // Precision and recall for binary datasets
            let (precision, recall) = if ds.mode == "binary" {
                let tp = prompt_results
                    .iter()
                    .filter(|(p, c)| p.polarity == Polarity::Positive && *c)
                    .count();
                let fp = prompt_results
                    .iter()
                    .filter(|(p, c)| p.polarity == Polarity::Negative && !*c)
                    .count();
                let fn_ = prompt_results
                    .iter()
                    .filter(|(p, c)| p.polarity == Polarity::Positive && !*c)
                    .count();
                let prec = if tp + fp > 0 {
                    tp as f64 / (tp + fp) as f64
                } else {
                    0.0
                };
                let rec = if tp + fn_ > 0 {
                    tp as f64 / (tp + fn_) as f64
                } else {
                    0.0
                };
                (prec, rec)
            } else {
                (accuracy, accuracy) // For multi-category, precision ≈ accuracy
            };

            let tier_acc = compute_tier_accuracy(&prompt_results);

            dataset_results.push(DatasetResult {
                dataset: ds.name.clone(),
                accuracy,
                accuracy_by_tier: tier_acc,
                precision,
                recall,
                latency_p50_ms: percentile(&all_latencies, 50.0).as_secs_f64() * 1000.0,
                latency_p95_ms: percentile(&all_latencies, 95.0).as_secs_f64() * 1000.0,
                latency_p99_ms: percentile(&all_latencies, 99.0).as_secs_f64() * 1000.0,
                latency_cv: coefficient_of_variation(&all_latencies),
                total_prompts: total,
                correct: correct_count,
            });

            pb.inc(1);
        }

        results.push(ModelResult {
            model: model.as_str().to_string(),
            dimensions: model.dimensions(),
            cold_startup_ms,
            datasets: dataset_results,
        });
    }

    pb.finish_with_message("done");

    Ok(BenchmarkRun {
        timestamp: format!(
            "{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        ),
        config: BenchmarkConfig {
            models: models.iter().map(|m| m.as_str().to_string()).collect(),
            datasets: datasets.iter().map(|d| d.name.clone()).collect(),
            warmup_iterations: warmup,
            measured_iterations: iterations,
        },
        results,
        system_info: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
    })
}

/// Print results as a human-readable comparison table.
pub fn print_table(run: &BenchmarkRun) {
    for ds_name in &run.config.datasets {
        println!("\n{ds_name}");

        let mut table = Table::new();
        table.set_header(vec![
            Cell::new("Model"),
            Cell::new("Accuracy"),
            Cell::new("Edge Acc"),
            Cell::new("p50 (ms)"),
            Cell::new("p95 (ms)"),
            Cell::new("Cold (s)"),
        ]);

        let mut best_model = String::new();
        let mut best_accuracy = 0.0f64;

        for model_result in &run.results {
            if let Some(ds) = model_result.datasets.iter().find(|d| d.dataset == *ds_name) {
                let edge_acc = (ds.accuracy_by_tier.edge_pos + ds.accuracy_by_tier.edge_neg) / 2.0;

                if ds.accuracy > best_accuracy {
                    best_accuracy = ds.accuracy;
                    best_model = model_result.model.clone();
                }

                table.add_row(vec![
                    Cell::new(&model_result.model),
                    Cell::new(format!("{:.1}%", ds.accuracy * 100.0)),
                    Cell::new(format!("{:.1}%", edge_acc * 100.0)),
                    Cell::new(format!("{:.1}", ds.latency_p50_ms)),
                    Cell::new(format!("{:.1}", ds.latency_p95_ms)),
                    Cell::new(format!("{:.1}", model_result.cold_startup_ms / 1000.0)),
                ]);
            }
        }

        println!("{table}");
        if !best_model.is_empty() {
            println!(
                "Best: {} ({:.1}% accuracy)",
                best_model,
                best_accuracy * 100.0
            );
        }
    }
}

/// Print comparison between current and previous benchmark runs.
pub fn print_comparison(current: &BenchmarkRun, previous: &BenchmarkRun) {
    println!("\nRegression Report (vs previous run)\n");

    for ds_name in &current.config.datasets {
        println!("{ds_name}:");

        for curr_model in &current.results {
            let prev_model = previous
                .results
                .iter()
                .find(|m| m.model == curr_model.model);
            let Some(prev_model) = prev_model else {
                println!("  {}: NEW (no previous data)", curr_model.model);
                continue;
            };

            let curr_ds = curr_model.datasets.iter().find(|d| d.dataset == *ds_name);
            let prev_ds = prev_model.datasets.iter().find(|d| d.dataset == *ds_name);

            if let (Some(curr), Some(prev)) = (curr_ds, prev_ds) {
                let acc_diff = (curr.accuracy - prev.accuracy) * 100.0;
                let lat_diff_pct = if prev.latency_p50_ms > 0.0 {
                    ((curr.latency_p50_ms - prev.latency_p50_ms) / prev.latency_p50_ms) * 100.0
                } else {
                    0.0
                };

                let arrow = if acc_diff >= 0.0 { "▲" } else { "▼" };
                let warn = if acc_diff < -1.0 || lat_diff_pct > 20.0 {
                    " ⚠️"
                } else {
                    ""
                };

                println!(
                    "  {}: accuracy {:.1}% → {:.1}% ({}{:.1}%){}",
                    curr_model.model,
                    prev.accuracy * 100.0,
                    curr.accuracy * 100.0,
                    arrow,
                    acc_diff.abs(),
                    warn
                );
            }
        }
        println!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_basic() {
        let durations: Vec<Duration> = (1..=100).map(|i| Duration::from_millis(i)).collect();
        assert_eq!(percentile(&durations, 50.0), Duration::from_millis(50));
        assert_eq!(percentile(&durations, 95.0), Duration::from_millis(95));
        assert_eq!(percentile(&durations, 99.0), Duration::from_millis(99));
    }

    #[test]
    fn percentile_empty() {
        assert_eq!(percentile(&[], 50.0), Duration::ZERO);
    }

    #[test]
    fn cv_stable_values() {
        let durations = vec![Duration::from_millis(10); 20];
        assert!(coefficient_of_variation(&durations) < 1e-10);
    }

    #[test]
    fn cv_varied_values() {
        let durations: Vec<Duration> = (1..=10).map(|i| Duration::from_millis(i)).collect();
        let cv = coefficient_of_variation(&durations);
        assert!(cv > 0.0 && cv < 1.0);
    }

    #[test]
    fn tier_accuracy_computation() {
        let results = vec![
            (
                LabeledPrompt {
                    text: "a".into(),
                    expected_label: "match".into(),
                    tier: Tier::Clear,
                    polarity: Polarity::Positive,
                },
                true,
            ),
            (
                LabeledPrompt {
                    text: "b".into(),
                    expected_label: "match".into(),
                    tier: Tier::Clear,
                    polarity: Polarity::Positive,
                },
                false,
            ),
        ];
        let acc = compute_tier_accuracy(&results);
        assert!((acc.clear_pos - 0.5).abs() < 1e-6);
    }
}
