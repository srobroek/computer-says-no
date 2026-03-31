use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::model::{Embedding, EmbeddingEngine};

/// Classification mode for a reference set.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    Binary,
    MultiCategory,
}

/// TOML metadata block.
#[derive(Debug, Clone, Deserialize)]
pub struct Metadata {
    pub name: String,
    pub description: Option<String>,
    pub mode: Mode,
    pub threshold: f32,
    pub source: Option<String>,
}

/// Raw TOML structure for binary sets.
#[derive(Debug, Clone, Deserialize)]
pub struct BinaryPhrases {
    pub positive: Vec<String>,
    pub negative: Option<Vec<String>>,
}

/// Raw TOML structure for multi-category sets.
#[derive(Debug, Clone, Deserialize)]
pub struct CategoryPhrases {
    pub phrases: Vec<String>,
}

/// Raw TOML file structure.
#[derive(Debug, Clone, Deserialize)]
pub struct ReferenceSetFile {
    pub metadata: Metadata,
    pub phrases: Option<BinaryPhrases>,
    pub categories: Option<HashMap<String, CategoryPhrases>>,
}

/// Precomputed embeddings for a binary reference set.
#[derive(Debug, Clone)]
pub struct BinaryEmbeddings {
    pub positive: Vec<Embedding>,
    pub positive_phrases: Vec<String>,
    pub negative: Vec<Embedding>,
    pub negative_phrases: Vec<String>,
}

/// Precomputed embeddings for a single category.
#[derive(Debug, Clone)]
pub struct CategoryEmbeddings {
    pub embeddings: Vec<Embedding>,
    pub phrases: Vec<String>,
}

/// Precomputed embeddings for a multi-category reference set.
#[derive(Debug, Clone)]
pub struct MultiCategoryEmbeddings {
    pub categories: HashMap<String, CategoryEmbeddings>,
}

/// A fully loaded and embedded reference set, ready for classification.
#[derive(Debug, Clone)]
pub struct ReferenceSet {
    pub metadata: Metadata,
    pub kind: ReferenceSetKind,
    pub content_hash: String,
    pub source_path: PathBuf,
}

#[derive(Debug, Clone)]
pub enum ReferenceSetKind {
    Binary(BinaryEmbeddings),
    MultiCategory(MultiCategoryEmbeddings),
}

impl ReferenceSet {
    pub fn phrase_count(&self) -> usize {
        match &self.kind {
            ReferenceSetKind::Binary(b) => b.positive_phrases.len() + b.negative_phrases.len(),
            ReferenceSetKind::MultiCategory(m) => {
                m.categories.values().map(|c| c.phrases.len()).sum()
            }
        }
    }
}

/// Load and embed a reference set from a TOML file.
pub fn load_reference_set(path: &Path, engine: &mut EmbeddingEngine) -> Result<ReferenceSet> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;

    let content_hash = blake3::hash(content.as_bytes()).to_hex().to_string();

    let file: ReferenceSetFile =
        toml::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;

    let kind = match file.metadata.mode {
        Mode::Binary => {
            let phrases = file.phrases.with_context(|| {
                format!(
                    "{}: binary mode requires [phrases] section with positive phrases",
                    path.display()
                )
            })?;

            anyhow::ensure!(
                !phrases.positive.is_empty(),
                "{}: binary reference set must have at least one positive phrase",
                path.display()
            );

            let positive = embed_phrases(engine, &phrases.positive)?;
            let negative_phrases = phrases.negative.unwrap_or_default();
            let negative = embed_phrases(engine, &negative_phrases)?;

            ReferenceSetKind::Binary(BinaryEmbeddings {
                positive,
                positive_phrases: phrases.positive,
                negative,
                negative_phrases,
            })
        }
        Mode::MultiCategory => {
            let categories = file.categories.with_context(|| {
                format!(
                    "{}: multi-category mode requires [categories] section",
                    path.display()
                )
            })?;

            anyhow::ensure!(
                !categories.is_empty(),
                "{}: multi-category reference set must have at least one category",
                path.display()
            );

            for (name, cat) in &categories {
                anyhow::ensure!(
                    !cat.phrases.is_empty(),
                    "{}: category '{}' must have at least one phrase",
                    path.display(),
                    name
                );
            }

            let mut cat_embeddings = HashMap::new();
            for (name, cat) in &categories {
                let embeddings = embed_phrases(engine, &cat.phrases)?;
                cat_embeddings.insert(
                    name.clone(),
                    CategoryEmbeddings {
                        embeddings,
                        phrases: cat.phrases.clone(),
                    },
                );
            }

            ReferenceSetKind::MultiCategory(MultiCategoryEmbeddings {
                categories: cat_embeddings,
            })
        }
    };

    Ok(ReferenceSet {
        metadata: file.metadata,
        kind,
        content_hash,
        source_path: path.to_owned(),
    })
}

/// Load all .toml reference sets from a directory.
pub fn load_all_reference_sets(
    dir: &Path,
    engine: &mut EmbeddingEngine,
) -> Result<Vec<ReferenceSet>> {
    let mut sets = Vec::new();
    if !dir.exists() {
        return Ok(sets);
    }

    let entries: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("reading directory {}", dir.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    for entry in entries {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "toml") {
            match load_reference_set(&path, engine) {
                Ok(set) => {
                    tracing::info!(
                        name = %set.metadata.name,
                        phrases = set.phrase_count(),
                        "loaded reference set"
                    );
                    sets.push(set);
                }
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "skipping invalid reference set");
                }
            }
        }
    }

    Ok(sets)
}

fn embed_phrases(engine: &mut EmbeddingEngine, phrases: &[String]) -> Result<Vec<Embedding>> {
    if phrases.is_empty() {
        return Ok(Vec::new());
    }
    let refs: Vec<&str> = phrases.iter().map(String::as_str).collect();
    engine.embed(&refs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_binary_toml() {
        let toml_str = r#"
[metadata]
name = "test"
mode = "binary"
threshold = 0.5

[phrases]
positive = ["yes", "correct"]
negative = ["no", "wrong"]
"#;
        let file: ReferenceSetFile = toml::from_str(toml_str).unwrap();
        assert_eq!(file.metadata.mode, Mode::Binary);
        assert_eq!(file.phrases.unwrap().positive.len(), 2);
    }

    #[test]
    fn parse_multi_category_toml() {
        let toml_str = r#"
[metadata]
name = "test"
mode = "multi-category"
threshold = 0.4

[categories.feat]
phrases = ["add feature", "implement"]

[categories.fix]
phrases = ["fix bug", "resolve"]
"#;
        let file: ReferenceSetFile = toml::from_str(toml_str).unwrap();
        assert_eq!(file.metadata.mode, Mode::MultiCategory);
        let cats = file.categories.unwrap();
        assert_eq!(cats.len(), 2);
        assert!(cats.contains_key("feat"));
    }
}
