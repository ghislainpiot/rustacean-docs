use crate::{Cache, CacheStats};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Disk-based cache implementation using cacache
pub struct DiskCache<K, V> {
    cache_dir: PathBuf,
    stats: Arc<RwLock<CacheStats>>,
    _phantom: PhantomData<(K, V)>,
}

impl<K, V> DiskCache<K, V> {
    /// Create a new disk cache with the specified directory
    pub fn new<P: AsRef<Path>>(cache_dir: P) -> Self {
        Self {
            cache_dir: cache_dir.as_ref().to_path_buf(),
            stats: Arc::new(RwLock::new(CacheStats {
                capacity: usize::MAX, // Disk cache doesn't have a fixed item capacity
                ..Default::default()
            })),
            _phantom: PhantomData,
        }
    }

    /// Get the cache directory path
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }
}

#[async_trait]
impl<K, V> Cache for DiskCache<K, V>
where
    K: ToString + Send + Sync,
    V: Serialize + DeserializeOwned + Send + Sync,
{
    type Key = K;
    type Value = V;
    type Error = anyhow::Error;

    async fn get(&self, key: &Self::Key) -> Result<Option<Self::Value>, Self::Error> {
        let mut stats = self.stats.write().await;
        let key_str = key.to_string();

        match cacache::read(&self.cache_dir, &key_str).await {
            Ok(data) => {
                stats.hits += 1;
                let value: V = serde_json::from_slice(&data)
                    .context("Failed to deserialize cached value")?;
                Ok(Some(value))
            }
            Err(e) => {
                // Check if it's a not found error by examining the error message
                let error_str = e.to_string();
                if error_str.contains("not found") || error_str.contains("NotFound") || error_str.contains("Entry not found") {
                    stats.misses += 1;
                    Ok(None)
                } else {
                    Err(anyhow::Error::from(e)).context("Failed to read from disk cache")
                }
            }
        }
    }

    async fn insert(&self, key: Self::Key, value: Self::Value) -> Result<(), Self::Error> {
        let key_str = key.to_string();
        let data = serde_json::to_vec(&value).context("Failed to serialize value")?;

        cacache::write(&self.cache_dir, &key_str, data)
            .await
            .context("Failed to write to disk cache")?;

        // Update size estimate
        let mut stats = self.stats.write().await;
        stats.size = stats.size.saturating_add(1);

        Ok(())
    }

    async fn remove(&self, key: &Self::Key) -> Result<(), Self::Error> {
        let key_str = key.to_string();

        cacache::remove(&self.cache_dir, &key_str)
            .await
            .context("Failed to remove from disk cache")?;

        // Update size estimate
        let mut stats = self.stats.write().await;
        if stats.size > 0 {
            stats.size = stats.size.saturating_sub(1);
        }

        Ok(())
    }

    async fn clear(&self) -> Result<(), Self::Error> {
        cacache::clear(&self.cache_dir)
            .await
            .context("Failed to clear disk cache")?;

        let mut stats = self.stats.write().await;
        stats.size = 0;

        Ok(())
    }

    fn stats(&self) -> CacheStats {
        self.stats
            .try_read()
            .map(|s| s.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_disk_cache_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let cache = DiskCache::<String, String>::new(temp_dir.path());

        // Test insert and get
        cache
            .insert("key1".to_string(), "value1".to_string())
            .await
            .unwrap();
        let result = cache.get(&"key1".to_string()).await.unwrap();
        assert_eq!(result, Some("value1".to_string()));

        // Test miss
        let result = cache.get(&"key2".to_string()).await.unwrap();
        assert_eq!(result, None);

        // Test remove
        cache.remove(&"key1".to_string()).await.unwrap();
        let result = cache.get(&"key1".to_string()).await.unwrap();
        assert_eq!(result, None);

        // Test stats
        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 2);
    }

    #[tokio::test]
    async fn test_disk_cache_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().to_path_buf();

        // Create cache and insert data
        {
            let cache = DiskCache::<String, String>::new(&cache_path);
            cache
                .insert("persistent".to_string(), "data".to_string())
                .await
                .unwrap();
        }

        // Create new cache instance and verify data persists
        {
            let cache = DiskCache::<String, String>::new(&cache_path);
            let result = cache.get(&"persistent".to_string()).await.unwrap();
            assert_eq!(result, Some("data".to_string()));
        }
    }

    #[tokio::test]
    async fn test_disk_cache_clear() {
        let temp_dir = TempDir::new().unwrap();
        let cache = DiskCache::<String, String>::new(temp_dir.path());

        // Add some items
        for i in 0..5 {
            cache
                .insert(format!("key{}", i), format!("value{}", i))
                .await
                .unwrap();
        }

        // Clear cache
        cache.clear().await.unwrap();

        let stats = cache.stats();
        assert_eq!(stats.size, 0);

        // Verify items are gone
        assert_eq!(cache.get(&"key0".to_string()).await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_disk_cache_with_complex_types() {
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
        struct ComplexValue {
            id: u64,
            name: String,
            tags: Vec<String>,
        }

        let temp_dir = TempDir::new().unwrap();
        let cache = DiskCache::<String, ComplexValue>::new(temp_dir.path());

        let value = ComplexValue {
            id: 42,
            name: "Test".to_string(),
            tags: vec!["tag1".to_string(), "tag2".to_string()],
        };

        cache
            .insert("complex".to_string(), value.clone())
            .await
            .unwrap();
        let result = cache.get(&"complex".to_string()).await.unwrap();
        assert_eq!(result, Some(value));
    }
}