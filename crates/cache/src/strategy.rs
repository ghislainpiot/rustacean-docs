use anyhow::Result;
use rustacean_docs_core::CacheLayerStats;
use serde::{de::DeserializeOwned, Serialize};
use std::path::Path;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, trace};

use crate::{DiskCache, MemoryCache};

/// Combined cache statistics from both memory and disk layers
#[derive(Debug, Clone)]
pub struct CombinedCacheStats {
    pub memory: CacheLayerStats,
    pub disk: CacheLayerStats,
    pub total_requests: u64,
    pub total_hits: u64,
    pub total_misses: u64,
    pub combined_hit_rate: f64,
}

/// Multi-tier cache that coordinates between memory and disk caches
///
/// The strategy is:
/// 1. Always check memory cache first (fastest)
/// 2. On memory miss, check disk cache
/// 3. On disk hit, populate memory cache with the value
/// 4. On complete miss, return None
/// 5. All writes go to both caches (write-through)
pub struct TieredCache<K, V> {
    memory: MemoryCache<K, V>,
    disk: DiskCache,
    stats: RwLock<CombinedCacheStats>,
}

impl<K, V> TieredCache<K, V>
where
    K: std::hash::Hash + Eq + Clone + Send + Sync + std::fmt::Display,
    V: Clone + Send + Sync + Serialize + DeserializeOwned,
{
    /// Create a new tiered cache with both memory and disk layers
    pub async fn new<P: AsRef<Path>>(
        memory_capacity: usize,
        memory_ttl: Duration,
        disk_cache_dir: P,
        disk_ttl: Duration,
        disk_max_size_bytes: u64,
    ) -> Result<Self> {
        let memory = MemoryCache::new(memory_capacity, memory_ttl);
        let disk = DiskCache::new(disk_cache_dir, disk_ttl, disk_max_size_bytes).await?;

        let initial_stats = CombinedCacheStats {
            memory: memory.stats().await,
            disk: disk.stats().await?,
            total_requests: 0,
            total_hits: 0,
            total_misses: 0,
            combined_hit_rate: 0.0,
        };

        debug!(
            "Created tiered cache with memory capacity {} (TTL: {:?}) and disk max size {} bytes (TTL: {:?})",
            memory_capacity, memory_ttl, disk_max_size_bytes, disk_ttl
        );

        Ok(Self {
            memory,
            disk,
            stats: RwLock::new(initial_stats),
        })
    }

    /// Get a value from the cache, checking memory first, then disk
    pub async fn get(&self, key: &K) -> Result<Option<V>> {
        let key_str = key.to_string();

        // Update total request count
        {
            let mut stats = self.stats.write().await;
            stats.total_requests += 1;
        }

        trace!("Getting key '{key_str}' from tiered cache");

        // First, try memory cache
        if let Some(value) = self.memory.get(key).await {
            trace!("Memory cache hit for key '{key_str}'");

            // Update combined stats
            {
                let mut stats = self.stats.write().await;
                stats.total_hits += 1;
                stats.combined_hit_rate =
                    (stats.total_hits as f64 / stats.total_requests as f64) * 100.0;
            }

            return Ok(Some(value));
        }

        trace!("Memory cache miss for key '{key_str}', checking disk cache");

        // Memory miss, try disk cache
        if let Some(value) = self.disk.get::<V>(&key_str).await? {
            trace!("Disk cache hit for key '{key_str}', promoting to memory");

            // Promote to memory cache (don't wait for this)
            let _ = self.memory.insert(key.clone(), value.clone()).await;

            // Update combined stats
            {
                let mut stats = self.stats.write().await;
                stats.total_hits += 1;
                stats.combined_hit_rate =
                    (stats.total_hits as f64 / stats.total_requests as f64) * 100.0;
            }

            return Ok(Some(value));
        }

        trace!("Complete cache miss for key '{key_str}'");

        // Complete miss
        {
            let mut stats = self.stats.write().await;
            stats.total_misses += 1;
            stats.combined_hit_rate =
                (stats.total_hits as f64 / stats.total_requests as f64) * 100.0;
        }

        Ok(None)
    }

    /// Insert a value into both memory and disk caches (write-through)
    pub async fn insert(&self, key: K, value: V) -> Result<()> {
        let key_str = key.to_string();
        trace!("Inserting key '{key_str}' into tiered cache");

        // Write to both caches concurrently
        let memory_result = self.memory.insert(key, value.clone());
        let disk_result = self.disk.insert(key_str, value);

        // Wait for both operations
        let (_, disk_res) = tokio::join!(memory_result, disk_result);

        // Return disk result since it's the one that can fail
        disk_res
    }

    /// Remove a key from both caches
    pub async fn remove(&self, key: &K) -> Result<bool> {
        let key_str = key.to_string();
        trace!("Removing key '{key_str}' from tiered cache");

        // Remove from both caches concurrently
        let memory_result = self.memory.remove(key);
        let disk_result = self.disk.remove(&key_str);

        let (memory_removed, disk_removed) = tokio::join!(memory_result, disk_result);

        // Return true if removed from either cache
        Ok(memory_removed.is_some() || disk_removed?)
    }

    /// Check if either cache contains the key
    pub async fn contains(&self, key: &K) -> bool {
        let key_str = key.to_string();

        // Check memory first (faster)
        if self.memory.contains(key).await {
            return true;
        }

        // Check disk
        self.disk.contains(&key_str).await
    }

    /// Clear both caches
    pub async fn clear(&self) -> Result<(usize, usize)> {
        debug!("Clearing all caches");

        // Clear both caches concurrently
        let memory_result = self.memory.clear();
        let disk_result = self.disk.clear();

        let (memory_count, disk_result) = tokio::join!(memory_result, disk_result);
        let disk_count = disk_result?;

        debug!(
            "Cleared {} items from memory, {} items from disk",
            memory_count, disk_count
        );

        Ok((memory_count, disk_count))
    }

    /// Get comprehensive cache statistics
    pub async fn stats(&self) -> Result<CombinedCacheStats> {
        let memory_stats = self.memory.stats().await;
        let disk_stats = self.disk.stats().await?;
        let combined_stats = self.stats.read().await.clone();

        Ok(CombinedCacheStats {
            memory: memory_stats,
            disk: disk_stats,
            total_requests: combined_stats.total_requests,
            total_hits: combined_stats.total_hits,
            total_misses: combined_stats.total_misses,
            combined_hit_rate: combined_stats.combined_hit_rate,
        })
    }

    /// Cleanup expired entries from both caches
    pub async fn cleanup_expired(&self) -> Result<(usize, usize)> {
        debug!("Cleaning up expired entries from both caches");

        // Cleanup both caches concurrently
        let memory_result = self.memory.cleanup_expired();
        let disk_result = self.disk.cleanup_expired();

        let (memory_count, disk_result) = tokio::join!(memory_result, disk_result);
        let disk_count = disk_result?;

        debug!(
            "Cleaned {} expired from memory, {} from disk",
            memory_count, disk_count
        );

        Ok((memory_count, disk_count))
    }

    /// Enforce disk cache size limits
    pub async fn enforce_size_limits(&self) -> Result<usize> {
        debug!("Enforcing disk cache size limits");
        self.disk.enforce_size_limit().await
    }

    /// Get memory cache reference (for advanced operations)
    pub fn memory_cache(&self) -> &MemoryCache<K, V> {
        &self.memory
    }

    /// Get disk cache reference (for advanced operations)
    pub fn disk_cache(&self) -> &DiskCache {
        &self.disk
    }

    /// Run periodic maintenance (cleanup expired entries and enforce size limits)
    pub async fn maintenance(&self) -> Result<MaintenanceReport> {
        debug!("Running cache maintenance");

        let (memory_expired, disk_expired) = self.cleanup_expired().await?;
        let size_enforced = self.enforce_size_limits().await?;

        let report = MaintenanceReport {
            memory_expired,
            disk_expired,
            size_enforced,
        };

        debug!("Maintenance completed: {report:?}");
        Ok(report)
    }
}

/// Report from cache maintenance operations
#[derive(Debug, Clone)]
pub struct MaintenanceReport {
    pub memory_expired: usize,
    pub disk_expired: usize,
    pub size_enforced: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::time::sleep;

    async fn create_test_tiered_cache() -> (std::sync::Arc<TieredCache<String, String>>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let cache = TieredCache::new(
            5,                       // memory capacity
            Duration::from_secs(60), // memory TTL
            temp_dir.path(),
            Duration::from_secs(60), // disk TTL
            1024 * 1024,             // 1MB disk size
        )
        .await
        .unwrap();
        (std::sync::Arc::new(cache), temp_dir)
    }

    #[tokio::test]
    async fn test_basic_operations() {
        let (cache, _temp_dir) = create_test_tiered_cache().await;

        // Test insert and get
        cache
            .insert("key1".to_string(), "value1".to_string())
            .await
            .unwrap();
        assert_eq!(
            cache.get(&"key1".to_string()).await.unwrap(),
            Some("value1".to_string())
        );

        // Test contains
        assert!(cache.contains(&"key1".to_string()).await);
        assert!(!cache.contains(&"nonexistent".to_string()).await);

        // Test remove
        assert!(cache.remove(&"key1".to_string()).await.unwrap());
        assert_eq!(cache.get(&"key1".to_string()).await.unwrap(), None);
        assert!(!cache.contains(&"key1".to_string()).await);
    }

    #[tokio::test]
    async fn test_memory_to_disk_promotion() {
        let temp_dir = TempDir::new().unwrap();
        let cache = TieredCache::new(
            2, // small memory capacity
            Duration::from_secs(60),
            temp_dir.path(),
            Duration::from_secs(60),
            1024 * 1024,
        )
        .await
        .unwrap();

        // Fill memory cache beyond capacity
        cache
            .insert("key1".to_string(), "value1".to_string())
            .await
            .unwrap();
        cache
            .insert("key2".to_string(), "value2".to_string())
            .await
            .unwrap();
        cache
            .insert("key3".to_string(), "value3".to_string())
            .await
            .unwrap(); // Should evict key1 from memory

        // key1 should be evicted from memory but still in disk
        assert_eq!(cache.memory_cache().get(&"key1".to_string()).await, None);

        // But we should still be able to get it (from disk, then promoted to memory)
        assert_eq!(
            cache.get(&"key1".to_string()).await.unwrap(),
            Some("value1".to_string())
        );

        // Now it should be back in memory
        assert_eq!(
            cache.memory_cache().get(&"key1".to_string()).await,
            Some("value1".to_string())
        );
    }

    #[tokio::test]
    async fn test_write_through() {
        let (cache, _temp_dir) = create_test_tiered_cache().await;

        // Insert a value
        cache
            .insert("key1".to_string(), "value1".to_string())
            .await
            .unwrap();

        // Should be in both caches
        assert_eq!(
            cache.memory_cache().get(&"key1".to_string()).await,
            Some("value1".to_string())
        );
        assert_eq!(
            cache.disk_cache().get::<String>("key1").await.unwrap(),
            Some("value1".to_string())
        );
    }

    #[tokio::test]
    async fn test_clear_both_caches() {
        let (cache, _temp_dir) = create_test_tiered_cache().await;

        // Insert multiple values
        cache
            .insert("key1".to_string(), "value1".to_string())
            .await
            .unwrap();
        cache
            .insert("key2".to_string(), "value2".to_string())
            .await
            .unwrap();
        cache
            .insert("key3".to_string(), "value3".to_string())
            .await
            .unwrap();

        // Clear all
        let (memory_count, disk_count) = cache.clear().await.unwrap();
        assert!(memory_count > 0);
        assert!(disk_count > 0);

        // All entries should be gone
        assert_eq!(cache.get(&"key1".to_string()).await.unwrap(), None);
        assert_eq!(cache.get(&"key2".to_string()).await.unwrap(), None);
        assert_eq!(cache.get(&"key3".to_string()).await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_combined_statistics() {
        let (cache, _temp_dir) = create_test_tiered_cache().await;

        // Insert and access some values
        cache
            .insert("key1".to_string(), "value1".to_string())
            .await
            .unwrap();
        cache
            .insert("key2".to_string(), "value2".to_string())
            .await
            .unwrap();

        // Perform some gets (should all be memory hits)
        assert_eq!(
            cache.get(&"key1".to_string()).await.unwrap(),
            Some("value1".to_string())
        );
        assert_eq!(
            cache.get(&"key2".to_string()).await.unwrap(),
            Some("value2".to_string())
        );
        assert_eq!(cache.get(&"key3".to_string()).await.unwrap(), None); // miss

        let stats = cache.stats().await.unwrap();
        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.total_hits, 2);
        assert_eq!(stats.total_misses, 1);
        assert!((stats.combined_hit_rate - 66.67).abs() < 0.1); // Approximately 66.67%
    }

    #[tokio::test]
    async fn test_maintenance() {
        let temp_dir = TempDir::new().unwrap();
        let cache = TieredCache::new(
            5,
            Duration::from_millis(100), // Short TTL for testing
            temp_dir.path(),
            Duration::from_millis(100),
            1024 * 1024,
        )
        .await
        .unwrap();

        // Insert some values
        cache
            .insert("key1".to_string(), "value1".to_string())
            .await
            .unwrap();
        cache
            .insert("key2".to_string(), "value2".to_string())
            .await
            .unwrap();

        // Wait for expiration
        sleep(Duration::from_millis(150)).await;

        // Add a fresh entry
        cache
            .insert("key3".to_string(), "value3".to_string())
            .await
            .unwrap();

        // Run maintenance
        let report = cache.maintenance().await.unwrap();

        // Should have cleaned up expired entries
        assert!(report.memory_expired > 0 || report.disk_expired > 0);

        // Fresh entry should still be accessible
        assert_eq!(
            cache.get(&"key3".to_string()).await.unwrap(),
            Some("value3".to_string())
        );

        // Expired entries should be gone
        assert_eq!(cache.get(&"key1".to_string()).await.unwrap(), None);
        assert_eq!(cache.get(&"key2".to_string()).await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let (cache, _temp_dir) = create_test_tiered_cache().await;
        let mut handles = vec![];

        // Spawn multiple tasks that insert and read concurrently
        for i in 0..10 {
            let cache_clone = cache.clone();
            let handle = tokio::spawn(async move {
                let key = format!("key{i}");
                let value = format!("value{i}");

                cache_clone
                    .insert(key.clone(), value.clone())
                    .await
                    .unwrap();
                assert_eq!(cache_clone.get(&key).await.unwrap(), Some(value));
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify all values are accessible
        for i in 0..10 {
            let key = format!("key{i}");
            let value = format!("value{i}");
            assert_eq!(cache.get(&key).await.unwrap(), Some(value));
        }
    }
}
