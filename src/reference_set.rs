use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::embedding_cache::{self, CachedEmbeddings, EmbeddingGroup};
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
/// If `cache_dir` is provided, cached embeddings are used when the content hash matches.
pub fn load_reference_set(
    path: &Path,
    engine: &mut EmbeddingEngine,
    cache_dir: Option<&Path>,
) -> Result<ReferenceSet> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;

    let content_hash = blake3::hash(content.as_bytes()).to_hex().to_string();

    let file: ReferenceSetFile =
        toml::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;

    // Try loading from cache
    let cached = cache_dir.and_then(|dir| {
        embedding_cache::load_cache(
            dir,
            engine.model().as_str(),
            &content_hash,
            engine.dimensions(),
        )
    });

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

            if let Some(ref c) = cached {
                // Restore from cache: expect groups named "positive" and optionally "negative"
                let pos = c.groups.iter().find(|g| g.name == "positive");
                let neg = c.groups.iter().find(|g| g.name == "negative");
                if let Some(pos) = pos {
                    let negative_phrases = phrases.negative.clone().unwrap_or_default();
                    ReferenceSetKind::Binary(BinaryEmbeddings {
                        positive: pos.embeddings.clone(),
                        positive_phrases: pos.phrases.clone(),
                        negative: neg.map(|n| n.embeddings.clone()).unwrap_or_default(),
                        negative_phrases,
                    })
                } else {
                    embed_binary(engine, phrases)?
                }
            } else {
                let kind = embed_binary(engine, phrases)?;
                save_binary_cache(cache_dir, engine, &content_hash, &kind);
                kind
            }
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

            if let Some(c) = cached {
                let mut cat_embeddings = HashMap::new();
                for group in &c.groups {
                    cat_embeddings.insert(
                        group.name.clone(),
                        CategoryEmbeddings {
                            embeddings: group.embeddings.clone(),
                            phrases: group.phrases.clone(),
                        },
                    );
                }
                ReferenceSetKind::MultiCategory(MultiCategoryEmbeddings {
                    categories: cat_embeddings,
                })
            } else {
                let kind = embed_multi_category(engine, &categories)?;
                save_multi_cache(cache_dir, engine, &content_hash, &kind);
                kind
            }
        }
    };

    Ok(ReferenceSet {
        metadata: file.metadata,
        kind,
        content_hash,
        source_path: path.to_owned(),
    })
}

fn embed_binary(engine: &mut EmbeddingEngine, phrases: BinaryPhrases) -> Result<ReferenceSetKind> {
    let positive = embed_phrases(engine, &phrases.positive)?;
    let negative_phrases = phrases.negative.unwrap_or_default();
    let negative = embed_phrases(engine, &negative_phrases)?;
    Ok(ReferenceSetKind::Binary(BinaryEmbeddings {
        positive,
        positive_phrases: phrases.positive,
        negative,
        negative_phrases,
    }))
}

fn embed_multi_category(
    engine: &mut EmbeddingEngine,
    categories: &HashMap<String, CategoryPhrases>,
) -> Result<ReferenceSetKind> {
    let mut cat_embeddings = HashMap::new();
    for (name, cat) in categories {
        let embeddings = embed_phrases(engine, &cat.phrases)?;
        cat_embeddings.insert(
            name.clone(),
            CategoryEmbeddings {
                embeddings,
                phrases: cat.phrases.clone(),
            },
        );
    }
    Ok(ReferenceSetKind::MultiCategory(MultiCategoryEmbeddings {
        categories: cat_embeddings,
    }))
}

fn save_binary_cache(
    cache_dir: Option<&Path>,
    engine: &EmbeddingEngine,
    content_hash: &str,
    kind: &ReferenceSetKind,
) {
    let Some(dir) = cache_dir else { return };
    let ReferenceSetKind::Binary(b) = kind else {
        return;
    };
    let cached = CachedEmbeddings {
        dimensions: engine.dimensions(),
        groups: vec![
            EmbeddingGroup {
                name: "positive".to_string(),
                phrases: b.positive_phrases.clone(),
                embeddings: b.positive.clone(),
            },
            EmbeddingGroup {
                name: "negative".to_string(),
                phrases: b.negative_phrases.clone(),
                embeddings: b.negative.clone(),
            },
        ],
    };
    if let Err(e) = embedding_cache::save_cache(dir, engine.model().as_str(), content_hash, &cached)
    {
        tracing::warn!(error = %e, "failed to save embedding cache");
    }
}

fn save_multi_cache(
    cache_dir: Option<&Path>,
    engine: &EmbeddingEngine,
    content_hash: &str,
    kind: &ReferenceSetKind,
) {
    let Some(dir) = cache_dir else { return };
    let ReferenceSetKind::MultiCategory(m) = kind else {
        return;
    };
    let groups = m
        .categories
        .iter()
        .map(|(name, cat)| EmbeddingGroup {
            name: name.clone(),
            phrases: cat.phrases.clone(),
            embeddings: cat.embeddings.clone(),
        })
        .collect();
    let cached = CachedEmbeddings {
        dimensions: engine.dimensions(),
        groups,
    };
    if let Err(e) = embedding_cache::save_cache(dir, engine.model().as_str(), content_hash, &cached)
    {
        tracing::warn!(error = %e, "failed to save embedding cache");
    }
}

/// Load all .toml reference sets from a directory.
pub fn load_all_reference_sets(
    dir: &Path,
    engine: &mut EmbeddingEngine,
    cache_dir: Option<&Path>,
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
            match load_reference_set(&path, engine, cache_dir) {
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

    #[test]
    fn corrections_toml_parses_as_multi_category() {
        let content = include_str!("../reference-sets/corrections.toml");
        let file: ReferenceSetFile = toml::from_str(content).unwrap();
        assert_eq!(file.metadata.mode, Mode::MultiCategory);
        assert_eq!(file.metadata.name, "corrections");

        let cats = file.categories.unwrap();
        assert!(
            cats.contains_key("correction"),
            "missing 'correction' category"
        );
        assert!(
            cats.contains_key("frustration"),
            "missing 'frustration' category"
        );
        assert!(cats.contains_key("neutral"), "missing 'neutral' category");

        // FR-002: each category must have at least 2 phrases.
        for (name, cat) in &cats {
            assert!(
                cat.phrases.len() >= 2,
                "category '{name}' has {} phrases, need ≥2",
                cat.phrases.len()
            );
        }

        // Total must be at least 4.
        let total: usize = cats.values().map(|c| c.phrases.len()).sum();
        assert!(total >= 4, "total phrases = {total}, need ≥4");
    }

    #[test]
    fn corrections_toml_no_duplicate_phrases() {
        let content = include_str!("../reference-sets/corrections.toml");
        let file: ReferenceSetFile = toml::from_str(content).unwrap();
        let cats = file.categories.unwrap();

        let mut all_phrases: Vec<&str> = Vec::new();
        for (_, cat) in &cats {
            for phrase in &cat.phrases {
                all_phrases.push(phrase);
            }
        }

        let mut seen = std::collections::HashSet::new();
        for phrase in &all_phrases {
            assert!(
                seen.insert(*phrase),
                "duplicate phrase across categories: '{phrase}'"
            );
        }
    }
}
