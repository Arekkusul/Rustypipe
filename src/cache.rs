use sha2::{Sha256, Digest};
use std::path::{Path, PathBuf};
use std::fs;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    key: String,
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
    timestamp: String,
}

pub struct Cache {
    cache_dir: PathBuf,
}

impl Cache {
    pub fn new(base_dir: &Path) -> Result<Self> {
        let cache_dir = base_dir.join("cache");
        fs::create_dir_all(&cache_dir)
            .with_context(|| format!("failed to create cache directory at {:?}", cache_dir))?;
        Ok(Self { cache_dir })
    }

    /// Generate a hash for the cache key
    fn hash_key(key: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Check if a cache entry exists and return it
    pub fn get(&self, key: &str) -> Result<Option<(String, String, Option<i32>)>> {
        let hash = Self::hash_key(key);
        let cache_file = self.cache_dir.join(format!("{}.json", hash));

        if !cache_file.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&cache_file)
            .with_context(|| format!("failed to read cache file {:?}", cache_file))?;

        let entry: CacheEntry = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse cache entry from {:?}", cache_file))?;

        Ok(Some((entry.stdout, entry.stderr, entry.exit_code)))
    }

    /// Store a cache entry
    pub fn set(&self, key: &str, stdout: &str, stderr: &str, exit_code: Option<i32>) -> Result<()> {
        let hash = Self::hash_key(key);
        let cache_file = self.cache_dir.join(format!("{}.json", hash));

        let entry = CacheEntry {
            key: key.to_string(),
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            exit_code,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        let content = serde_json::to_string_pretty(&entry)
            .context("failed to serialize cache entry")?;

        fs::write(&cache_file, content)
            .with_context(|| format!("failed to write cache file {:?}", cache_file))?;

        Ok(())
    }

    /// Clear all cache entries
    pub fn clear(&self) -> Result<()> {
        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir)
                .with_context(|| format!("failed to remove cache directory {:?}", self.cache_dir))?;
            fs::create_dir_all(&self.cache_dir)
                .with_context(|| format!("failed to recreate cache directory {:?}", self.cache_dir))?;
        }
        Ok(())
    }
}
