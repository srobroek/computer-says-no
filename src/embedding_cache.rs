use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::model::Embedding;

/// Header for the binary cache file format.
/// Format: magic(4) + version(1) + dimensions(4 LE) + num_groups(4 LE)
/// Per group: name_len(4 LE) + name(utf8) + num_embeddings(4 LE) + embedding_data(f32 LE)
const CACHE_MAGIC: &[u8; 4] = b"CSN\x00";
const CACHE_VERSION: u8 = 1;

/// A group of named embeddings (e.g., "positive", "negative", or category names).
#[derive(Debug, Clone)]
pub struct EmbeddingGroup {
    pub name: String,
    pub phrases: Vec<String>,
    pub embeddings: Vec<Embedding>,
}

/// Cached embedding data for a reference set.
#[derive(Debug, Clone)]
pub struct CachedEmbeddings {
    pub groups: Vec<EmbeddingGroup>,
    pub dimensions: usize,
}

/// Build the cache file path for a given content hash and model.
pub fn cache_path(cache_dir: &Path, model_name: &str, content_hash: &str) -> PathBuf {
    cache_dir
        .join(model_name)
        .join(format!("{content_hash}.bin"))
}

/// Try to load cached embeddings. Returns None if cache miss or dimension mismatch.
pub fn load_cache(
    cache_dir: &Path,
    model_name: &str,
    content_hash: &str,
    expected_dimensions: usize,
) -> Option<CachedEmbeddings> {
    let path = cache_path(cache_dir, model_name, content_hash);
    let data = std::fs::read(&path).ok()?;
    let cached = deserialize(&data).ok()?;

    if cached.dimensions != expected_dimensions {
        tracing::warn!(
            cached = cached.dimensions,
            expected = expected_dimensions,
            path = %path.display(),
            "embedding dimension mismatch — re-embedding"
        );
        // Remove stale cache file
        let _ = std::fs::remove_file(&path);
        return None;
    }

    tracing::debug!(path = %path.display(), "loaded embeddings from cache");
    Some(cached)
}

/// Save embeddings to cache.
pub fn save_cache(
    cache_dir: &Path,
    model_name: &str,
    content_hash: &str,
    cached: &CachedEmbeddings,
) -> Result<()> {
    let path = cache_path(cache_dir, model_name, content_hash);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating cache dir {}", parent.display()))?;
    }
    let data = serialize(cached)?;
    std::fs::write(&path, data).with_context(|| format!("writing cache {}", path.display()))?;
    tracing::debug!(path = %path.display(), "saved embeddings to cache");
    Ok(())
}

fn serialize(cached: &CachedEmbeddings) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    buf.write_all(CACHE_MAGIC)?;
    buf.write_all(&[CACHE_VERSION])?;
    buf.write_all(&(cached.dimensions as u32).to_le_bytes())?;
    buf.write_all(&(cached.groups.len() as u32).to_le_bytes())?;

    for group in &cached.groups {
        // Group name
        let name_bytes = group.name.as_bytes();
        buf.write_all(&(name_bytes.len() as u32).to_le_bytes())?;
        buf.write_all(name_bytes)?;

        // Number of embeddings
        buf.write_all(&(group.embeddings.len() as u32).to_le_bytes())?;

        // Phrases
        for phrase in &group.phrases {
            let phrase_bytes = phrase.as_bytes();
            buf.write_all(&(phrase_bytes.len() as u32).to_le_bytes())?;
            buf.write_all(phrase_bytes)?;
        }

        // Embedding data (f32 LE)
        for embedding in &group.embeddings {
            for &val in embedding {
                buf.write_all(&val.to_le_bytes())?;
            }
        }
    }

    Ok(buf)
}

fn deserialize(data: &[u8]) -> Result<CachedEmbeddings> {
    let mut cursor = std::io::Cursor::new(data);

    let mut magic = [0u8; 4];
    cursor.read_exact(&mut magic)?;
    anyhow::ensure!(magic == *CACHE_MAGIC, "invalid cache magic");

    let mut version = [0u8; 1];
    cursor.read_exact(&mut version)?;
    anyhow::ensure!(version[0] == CACHE_VERSION, "unsupported cache version");

    let dimensions = read_u32(&mut cursor)? as usize;
    let num_groups = read_u32(&mut cursor)? as usize;

    let mut groups = Vec::with_capacity(num_groups);
    for _ in 0..num_groups {
        let name = read_string(&mut cursor)?;
        let num_embeddings = read_u32(&mut cursor)? as usize;

        let mut phrases = Vec::with_capacity(num_embeddings);
        for _ in 0..num_embeddings {
            phrases.push(read_string(&mut cursor)?);
        }

        let mut embeddings = Vec::with_capacity(num_embeddings);
        for _ in 0..num_embeddings {
            let mut embedding = vec![0.0f32; dimensions];
            for val in &mut embedding {
                let mut bytes = [0u8; 4];
                cursor.read_exact(&mut bytes)?;
                *val = f32::from_le_bytes(bytes);
            }
            embeddings.push(embedding);
        }

        groups.push(EmbeddingGroup {
            name,
            phrases,
            embeddings,
        });
    }

    Ok(CachedEmbeddings { groups, dimensions })
}

fn read_u32(cursor: &mut std::io::Cursor<&[u8]>) -> Result<u32> {
    let mut buf = [0u8; 4];
    cursor.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_string(cursor: &mut std::io::Cursor<&[u8]>) -> Result<String> {
    let len = read_u32(cursor)? as usize;
    let mut buf = vec![0u8; len];
    cursor.read_exact(&mut buf)?;
    Ok(String::from_utf8(buf)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_serialize_deserialize() {
        let cached = CachedEmbeddings {
            dimensions: 3,
            groups: vec![
                EmbeddingGroup {
                    name: "positive".to_string(),
                    phrases: vec!["hello".to_string(), "world".to_string()],
                    embeddings: vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]],
                },
                EmbeddingGroup {
                    name: "negative".to_string(),
                    phrases: vec!["bye".to_string()],
                    embeddings: vec![vec![7.0, 8.0, 9.0]],
                },
            ],
        };

        let data = serialize(&cached).unwrap();
        let restored = deserialize(&data).unwrap();

        assert_eq!(restored.dimensions, 3);
        assert_eq!(restored.groups.len(), 2);
        assert_eq!(restored.groups[0].name, "positive");
        assert_eq!(restored.groups[0].phrases, vec!["hello", "world"]);
        assert_eq!(restored.groups[0].embeddings[0], vec![1.0, 2.0, 3.0]);
        assert_eq!(restored.groups[1].name, "negative");
        assert_eq!(restored.groups[1].embeddings[0], vec![7.0, 8.0, 9.0]);
    }

    #[test]
    fn dimension_mismatch_returns_none() {
        let dir = std::env::temp_dir().join("csn-test-cache-mismatch");
        let _ = std::fs::remove_dir_all(&dir);

        let cached = CachedEmbeddings {
            dimensions: 384,
            groups: vec![EmbeddingGroup {
                name: "test".to_string(),
                phrases: vec!["a".to_string()],
                embeddings: vec![vec![0.0; 384]],
            }],
        };

        save_cache(&dir, "model-a", "hash123", &cached).unwrap();
        // Request different dimensions → should return None
        let result = load_cache(&dir, "model-a", "hash123", 768);
        assert!(result.is_none());

        // Cache file should be removed
        let path = cache_path(&dir, "model-a", "hash123");
        assert!(!path.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cache_miss_returns_none() {
        let result = load_cache(Path::new("/nonexistent"), "model", "hash", 384);
        assert!(result.is_none());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join("csn-test-cache-roundtrip");
        let _ = std::fs::remove_dir_all(&dir);

        let cached = CachedEmbeddings {
            dimensions: 2,
            groups: vec![EmbeddingGroup {
                name: "cat".to_string(),
                phrases: vec!["meow".to_string()],
                embeddings: vec![vec![0.5, -0.3]],
            }],
        };

        save_cache(&dir, "tiny-model", "abc123", &cached).unwrap();
        let loaded = load_cache(&dir, "tiny-model", "abc123", 2).unwrap();

        assert_eq!(loaded.dimensions, 2);
        assert_eq!(loaded.groups[0].phrases[0], "meow");
        assert_eq!(loaded.groups[0].embeddings[0], vec![0.5, -0.3]);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
