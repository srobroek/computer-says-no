use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Difficulty tier for a labeled prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Clear,
    Moderate,
    Edge,
}

/// Whether the prompt should match (positive) or not match (negative).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Polarity {
    Positive,
    Negative,
}

/// A single labeled test entry in a benchmark dataset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabeledPrompt {
    pub text: String,
    pub expected_label: String,
    pub tier: Tier,
    pub polarity: Polarity,
}

/// A complete labeled dataset for benchmarking a reference set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabeledDataset {
    pub name: String,
    pub reference_set: String,
    pub mode: String,
    pub generated: String,
    pub prompts: Vec<LabeledPrompt>,
}

impl LabeledDataset {
    /// Count prompts by tier and polarity combination.
    pub fn count_by_bucket(&self) -> Vec<(Tier, Polarity, usize)> {
        let mut buckets = Vec::new();
        for tier in [Tier::Clear, Tier::Moderate, Tier::Edge] {
            for pol in [Polarity::Positive, Polarity::Negative] {
                let count = self
                    .prompts
                    .iter()
                    .filter(|p| p.tier == tier && p.polarity == pol)
                    .count();
                buckets.push((tier, pol, count));
            }
        }
        buckets
    }
}

/// Load a single dataset from a JSON file.
pub fn load_dataset(path: &Path) -> Result<LabeledDataset> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&content).with_context(|| format!("parsing {}", path.display()))
}

/// Load all .json datasets from a directory.
pub fn load_all_datasets(dir: &Path) -> Result<Vec<LabeledDataset>> {
    let mut datasets = Vec::new();
    if !dir.exists() {
        return Ok(datasets);
    }

    let entries: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("reading directory {}", dir.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    for entry in entries {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            match load_dataset(&path) {
                Ok(ds) => {
                    tracing::info!(name = %ds.name, prompts = ds.prompts.len(), "loaded dataset");
                    datasets.push(ds);
                }
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "skipping invalid dataset");
                }
            }
        }
    }

    Ok(datasets)
}

/// Generate a scaffold JSON template for a reference set.
/// The scaffold contains the structure with seed phrases but needs LLM-generated prompts.
pub fn generate_scaffold(set_name: &str, mode: &str, seed_phrases: &[String]) -> LabeledDataset {
    let mut prompts = Vec::new();

    // Add a few seed examples per bucket to show the expected format
    for tier in [Tier::Clear, Tier::Moderate, Tier::Edge] {
        for polarity in [Polarity::Positive, Polarity::Negative] {
            let label = match polarity {
                Polarity::Positive => "match",
                Polarity::Negative => "no_match",
            };

            // Add seed phrase as first example if available
            if let Some(phrase) = seed_phrases.first() {
                prompts.push(LabeledPrompt {
                    text: format!("[REPLACE] seed: {phrase}"),
                    expected_label: label.to_string(),
                    tier,
                    polarity,
                });
            }
        }
    }

    LabeledDataset {
        name: set_name.to_string(),
        reference_set: set_name.to_string(),
        mode: mode.to_string(),
        generated: chrono_now(),
        prompts,
    }
}

fn chrono_now() -> String {
    // Simple ISO 8601 without chrono dependency
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{now}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labeled_prompt_serde_roundtrip() {
        let prompt = LabeledPrompt {
            text: "no, use X instead".to_string(),
            expected_label: "match".to_string(),
            tier: Tier::Clear,
            polarity: Polarity::Positive,
        };
        let json = serde_json::to_string(&prompt).unwrap();
        let restored: LabeledPrompt = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.text, "no, use X instead");
        assert_eq!(restored.tier, Tier::Clear);
        assert_eq!(restored.polarity, Polarity::Positive);
    }

    #[test]
    fn dataset_count_by_bucket() {
        let ds = LabeledDataset {
            name: "test".to_string(),
            reference_set: "test".to_string(),
            mode: "binary".to_string(),
            generated: "0".to_string(),
            prompts: vec![
                LabeledPrompt {
                    text: "a".to_string(),
                    expected_label: "match".to_string(),
                    tier: Tier::Clear,
                    polarity: Polarity::Positive,
                },
                LabeledPrompt {
                    text: "b".to_string(),
                    expected_label: "match".to_string(),
                    tier: Tier::Clear,
                    polarity: Polarity::Positive,
                },
                LabeledPrompt {
                    text: "c".to_string(),
                    expected_label: "no_match".to_string(),
                    tier: Tier::Edge,
                    polarity: Polarity::Negative,
                },
            ],
        };
        let buckets = ds.count_by_bucket();
        let clear_pos = buckets
            .iter()
            .find(|(t, p, _)| *t == Tier::Clear && *p == Polarity::Positive)
            .unwrap();
        assert_eq!(clear_pos.2, 2);
    }

    #[test]
    fn generate_scaffold_has_6_buckets() {
        let scaffold = generate_scaffold("test", "binary", &["hello".to_string()]);
        assert_eq!(scaffold.prompts.len(), 6); // 3 tiers × 2 polarities
    }
}
