use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::dataset::{LabeledDataset, LabeledPrompt, Polarity, Tier};
use crate::model::ModelChoice;

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
