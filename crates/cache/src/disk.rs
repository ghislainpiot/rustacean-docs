use anyhow::{Context, Result};
use rustacean_docs_core::CacheLayerStats;
use serde::{de::DeserializeOwned, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, trace, warn};

/// Entry stored in the disk cache with value and creation timestamp
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct DiskCacheEntry<V> {
    value: V,
    created_at: u64, // Unix timestamp in milliseconds
}

impl<V> DiskCacheEntry<V> {
    fn new(value: V) -> Result<Self> {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("Failed to get current timestamp")?
            .as_millis() as u64;

        Ok(Self { value, created_at })
    }

    fn is_expired(&self, ttl: Duration) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        // Check if current time is beyond creation time + TTL (both in milliseconds)
        now > self.created_at + ttl.as_millis() as u64
    }
}

/// Statistics for tracking disk cache performance
#[derive(Debug, Clone, Default)]
struct CacheStatistics {
    hits: u64,
    misses: u64,
    writes: u64,
    deletes: u64,
    requests: u64,
    errors: u64,
}

impl CacheStatistics {
    fn hit_rate(&self) -> f64 {
        if self.requests == 0 {
            0.0
        } else {
            (self.hits as f64 / self.requests as f64) * 100.0
        }
    }
}

/// Persistent disk cache with TTL support and size management
pub struct DiskCache {
    cache_dir: PathBuf,
    ttl: Duration,
    max_size_bytes: u64,
    stats: RwLock<CacheStatistics>,
}

impl DiskCache {
    /// Create a new disk cache with specified directory, TTL, and max size
    pub async fn new<P: AsRef<Path>>(
        cache_dir: P,
        ttl: Duration,
        max_size_bytes: u64,
    ) -> Result<Self> {
        let cache_dir = cache_dir.as_ref().to_path_buf();

        // Ensure cache directory exists
        tokio::fs::create_dir_all(&cache_dir)
            .await
            .context("Failed to create cache directory")?;

        debug!(
            "Creating disk cache at {:?} with TTL {:?} and max size {} bytes",
            cache_dir, ttl, max_size_bytes
        );

        let cache = Self {
            cache_dir,
            ttl,
            max_size_bytes,
            stats: RwLock::new(CacheStatistics::default()),
        };

        // Perform initial cleanup
        cache.cleanup_expired().await?;
        cache.enforce_size_limit().await?;

        Ok(cache)
    }

    /// Get a value from the disk cache, checking for expiration
    pub async fn get<V>(&self, key: &str) -> Result<Option<V>>
    where
        V: DeserializeOwned,
    {
        let mut stats = self.stats.write().await;
        stats.requests += 1;
        drop(stats);

        trace!("Attempting to get key '{}' from disk cache", key);

        match cacache::read(&self.cache_dir, key).await {
            Ok(data) => {
                match serde_json::from_slice::<DiskCacheEntry<V>>(&data) {
                    Ok(entry) => {
                        if entry.is_expired(self.ttl) {
                            // Entry is expired, remove it
                            trace!("Disk cache entry expired for key '{}'", key);
                            let _ = self.remove_entry(key).await; // Don't propagate remove errors

                            let mut stats = self.stats.write().await;
                            stats.misses += 1;
                            Ok(None)
                        } else {
                            // Entry is valid
                            trace!("Disk cache hit for key '{}'", key);
                            let mut stats = self.stats.write().await;
                            stats.hits += 1;
                            Ok(Some(entry.value))
                        }
                    }
                    Err(e) => {
                        warn!("Failed to deserialize cache entry for key '{}': {}", key, e);
                        let _ = self.remove_entry(key).await; // Remove corrupted entry

                        let mut stats = self.stats.write().await;
                        stats.misses += 1;
                        stats.errors += 1;
                        Ok(None)
                    }
                }
            }
            Err(e) => {
                // cacache errors don't have kind() method, check the error string
                let error_str = format!("{e}");
                if error_str.contains("not found") || error_str.contains("No such file") {
                    trace!("Disk cache miss for key '{}'", key);
                    let mut stats = self.stats.write().await;
                    stats.misses += 1;
                    Ok(None)
                } else {
                    warn!("Error reading from disk cache for key '{}': {}", key, e);
                    let mut stats = self.stats.write().await;
                    stats.misses += 1;
                    stats.errors += 1;
                    Ok(None)
                }
            }
        }
    }

    /// Insert a value into the disk cache
    pub async fn insert<V>(&self, key: String, value: V) -> Result<()>
    where
        V: Serialize,
    {
        trace!("Inserting key '{}' into disk cache", key);

        let entry = DiskCacheEntry::new(value).context("Failed to create cache entry")?;

        let data = serde_json::to_vec(&entry).context("Failed to serialize cache entry")?;

        match cacache::write(&self.cache_dir, &key, data).await {
            Ok(_) => {
                trace!("Successfully wrote key '{}' to disk cache", key);
                let mut stats = self.stats.write().await;
                stats.writes += 1;
                Ok(())
            }
            Err(e) => {
                warn!("Failed to write data for key '{}': {}", key, e);
                let mut stats = self.stats.write().await;
                stats.errors += 1;
                Err(anyhow::anyhow!("Failed to write cache data: {}", e))
            }
        }
    }

    /// Remove a specific key from the cache
    pub async fn remove(&self, key: &str) -> Result<bool> {
        let removed = self.remove_entry(key).await?;
        if removed {
            let mut stats = self.stats.write().await;
            stats.deletes += 1;
        }
        Ok(removed)
    }

    /// Internal method to remove an entry without updating delete stats
    async fn remove_entry(&self, key: &str) -> Result<bool> {
        match cacache::remove(&self.cache_dir, key).await {
            Ok(_) => {
                trace!("Removed key '{}' from disk cache", key);
                Ok(true)
            }
            Err(e) => {
                let error_str = format!("{e}");
                if error_str.contains("not found") || error_str.contains("No such file") {
                    trace!("Key '{}' not found in disk cache for removal", key);
                    Ok(false)
                } else {
                    warn!("Error removing key '{}' from disk cache: {}", key, e);
                    let mut stats = self.stats.write().await;
                    stats.errors += 1;
                    Err(anyhow::anyhow!("Failed to remove cache entry: {}", e))
                }
            }
        }
    }

    /// Clear all entries from the cache
    pub async fn clear(&self) -> Result<usize> {
        debug!("Clearing all entries from disk cache");

        match cacache::clear(&self.cache_dir).await {
            Ok(_) => {
                // Unfortunately, cacache doesn't return the count of removed items
                // We'll estimate by checking the stats before and after
                let stats = self.stats.read().await;
                let estimated_count = (stats.writes - stats.deletes) as usize;
                drop(stats);

                // Reset stats since we cleared everything
                let mut stats = self.stats.write().await;
                stats.deletes += estimated_count as u64;

                debug!(
                    "Cleared approximately {} items from disk cache",
                    estimated_count
                );
                Ok(estimated_count)
            }
            Err(e) => {
                warn!("Error clearing disk cache: {}", e);
                let mut stats = self.stats.write().await;
                stats.errors += 1;
                Err(anyhow::anyhow!("Failed to clear cache: {}", e))
            }
        }
    }

    /// Check if the cache contains a non-expired entry for the key
    pub async fn contains(&self, key: &str) -> bool {
        match cacache::read(&self.cache_dir, key).await {
            Ok(data) => {
                // Try to deserialize just to check expiration, but don't care about the actual value
                match serde_json::from_slice::<serde_json::Value>(&data) {
                    Ok(entry_json) => {
                        if let Some(created_at) =
                            entry_json.get("created_at").and_then(|v| v.as_u64())
                        {
                            let now = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .map(|d| d.as_millis() as u64)
                                .unwrap_or(0);

                            let is_valid = now <= created_at + self.ttl.as_millis() as u64;
                            if !is_valid {
                                // Entry is expired, remove it
                                let _ = self.remove_entry(key).await;
                            }
                            is_valid
                        } else {
                            false // Invalid entry format
                        }
                    }
                    Err(_) => false, // Invalid JSON
                }
            }
            Err(_) => false, // Entry doesn't exist or error reading
        }
    }

    /// Get current cache statistics
    pub async fn stats(&self) -> Result<CacheLayerStats> {
        let stats = self.stats.read().await;

        // Get cache size information
        let (size, bytes_used) = match self.get_cache_size().await {
            Ok((size, bytes)) => (size, Some(bytes)),
            Err(e) => {
                warn!("Failed to get cache size: {}", e);
                (0, None)
            }
        };

        Ok(CacheLayerStats {
            size,
            capacity: 0, // Disk cache doesn't have a fixed entry capacity
            requests: stats.requests,
            hits: stats.hits,
            misses: stats.misses,
            hit_rate: stats.hit_rate(),
            bytes_used,
            bytes_capacity: Some(self.max_size_bytes),
        })
    }

    /// Get the current size of the cache (entry count and bytes used)
    async fn get_cache_size(&self) -> Result<(usize, u64)> {
        let mut entry_count = 0;
        let mut total_bytes: usize = 0;

        // Use cacache::index::ls() which returns an iterator
        for metadata in cacache::index::ls(&self.cache_dir).flatten() {
            entry_count += 1;
            total_bytes += metadata.size;
        }

        Ok((entry_count, total_bytes as u64))
    }

    /// Clean up expired entries from the cache
    pub async fn cleanup_expired(&self) -> Result<usize> {
        debug!("Starting cleanup of expired entries from disk cache");

        let mut expired_keys = Vec::new();

        for metadata in cacache::index::ls(&self.cache_dir).flatten() {
            // Read and check if expired
            if let Ok(data) = cacache::read(&self.cache_dir, &metadata.key).await {
                if let Ok(cache_entry) = serde_json::from_slice::<serde_json::Value>(&data) {
                    if let Some(created_at) = cache_entry.get("created_at").and_then(|v| v.as_u64())
                    {
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map(|d| d.as_millis() as u64)
                            .unwrap_or(0);

                        if now > created_at + self.ttl.as_millis() as u64 {
                            expired_keys.push(metadata.key);
                        }
                    }
                }
            }
        }

        // Remove expired entries
        let count = expired_keys.len();
        for key in expired_keys {
            let _ = self.remove_entry(&key).await; // Don't fail on individual removals
        }

        if count > 0 {
            debug!("Cleaned up {} expired entries from disk cache", count);
        }

        Ok(count)
    }

    /// Enforce the size limit by removing least recently used entries
    pub async fn enforce_size_limit(&self) -> Result<usize> {
        let (_, current_bytes) = self.get_cache_size().await?;

        if current_bytes <= self.max_size_bytes {
            return Ok(0); // No cleanup needed
        }

        debug!(
            "Disk cache size ({} bytes) exceeds limit ({} bytes), enforcing size limit",
            current_bytes, self.max_size_bytes
        );

        // Get all entries sorted by modified time (oldest first)
        let mut entries = Vec::new();

        for metadata in cacache::index::ls(&self.cache_dir).flatten() {
            entries.push(metadata);
        }

        // Sort by time (oldest first) - cacache entries have time information
        entries.sort_by_key(|entry| entry.time);

        let mut removed_count = 0;
        let mut current_size = current_bytes;
        let target_size = (self.max_size_bytes as f64 * 0.8) as u64; // Remove to 80% of limit

        for entry in entries {
            if current_size <= target_size {
                break;
            }

            if self.remove_entry(&entry.key).await.is_ok() {
                current_size = current_size.saturating_sub(entry.size as u64);
                removed_count += 1;
            }
        }

        if removed_count > 0 {
            debug!(
                "Removed {} entries to enforce size limit, new size: {} bytes",
                removed_count, current_size
            );
        }

        Ok(removed_count)
    }

    /// Get cache directory path
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Get cache TTL
    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Get max size in bytes
    pub fn max_size_bytes(&self) -> u64 {
        self.max_size_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::time::{sleep, timeout};

    async fn create_test_cache() -> (std::sync::Arc<DiskCache>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let cache = DiskCache::new(
            temp_dir.path(),
            Duration::from_secs(60),
            1024 * 1024, // 1MB
        )
        .await
        .unwrap();
        (std::sync::Arc::new(cache), temp_dir)
    }

    #[tokio::test]
    async fn test_new_cache() {
        let temp_dir = TempDir::new().unwrap();
        let cache = DiskCache::new(temp_dir.path(), Duration::from_secs(60), 1024 * 1024)
            .await
            .unwrap();

        assert_eq!(cache.ttl(), Duration::from_secs(60));
        assert_eq!(cache.max_size_bytes(), 1024 * 1024);
        assert_eq!(cache.cache_dir(), temp_dir.path());
    }

    #[tokio::test]
    async fn test_basic_operations() {
        let (cache, _temp_dir) = create_test_cache().await;

        // Test insert and get
        cache
            .insert("key1".to_string(), "value1".to_string())
            .await
            .unwrap();
        assert_eq!(
            cache.get::<String>("key1").await.unwrap(),
            Some("value1".to_string())
        );

        // Test contains
        assert!(cache.contains("key1").await);
        assert!(!cache.contains("nonexistent").await);

        // Test remove
        assert!(cache.remove("key1").await.unwrap());
        assert_eq!(cache.get::<String>("key1").await.unwrap(), None);
        assert!(!cache.contains("key1").await);
    }

    #[tokio::test]
    async fn test_ttl_expiration() {
        let temp_dir = TempDir::new().unwrap();
        let cache = DiskCache::new(
            temp_dir.path(),
            Duration::from_millis(50), // Even shorter TTL
            1024 * 1024,
        )
        .await
        .unwrap();

        // Insert a value
        cache
            .insert("key1".to_string(), "value1".to_string())
            .await
            .unwrap();
        assert_eq!(
            cache.get::<String>("key1").await.unwrap(),
            Some("value1".to_string())
        );

        // Wait for TTL to expire (use a longer wait to be safe)
        sleep(Duration::from_millis(100)).await;

        // Value should be expired and automatically removed
        assert_eq!(cache.get::<String>("key1").await.unwrap(), None);
        assert!(!cache.contains("key1").await);
    }

    #[tokio::test]
    async fn test_ttl_with_contains() {
        let temp_dir = TempDir::new().unwrap();
        let cache = DiskCache::new(temp_dir.path(), Duration::from_millis(100), 1024 * 1024)
            .await
            .unwrap();

        cache
            .insert("key1".to_string(), "value1".to_string())
            .await
            .unwrap();
        assert!(cache.contains("key1").await);

        // Wait for expiration
        sleep(Duration::from_millis(150)).await;

        // Should not contain expired key
        assert!(!cache.contains("key1").await);
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let temp_dir = TempDir::new().unwrap();
        let cache = DiskCache::new(temp_dir.path(), Duration::from_millis(100), 1024 * 1024)
            .await
            .unwrap();

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

        // Wait for all entries to expire
        sleep(Duration::from_millis(150)).await;

        // Add a new entry that won't be expired
        cache
            .insert("key4".to_string(), "value4".to_string())
            .await
            .unwrap();

        // Manual cleanup - should remove the 3 expired entries
        let expired_count = cache.cleanup_expired().await.unwrap();
        assert_eq!(expired_count, 3);

        // Only key4 should remain
        assert_eq!(
            cache.get::<String>("key4").await.unwrap(),
            Some("value4".to_string())
        );
        assert_eq!(cache.get::<String>("key1").await.unwrap(), None);
        assert_eq!(cache.get::<String>("key2").await.unwrap(), None);
        assert_eq!(cache.get::<String>("key3").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_clear() {
        let (cache, _temp_dir) = create_test_cache().await;

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
        let cleared_count = cache.clear().await.unwrap();
        assert!(cleared_count > 0);

        // All entries should be gone
        assert_eq!(cache.get::<String>("key1").await.unwrap(), None);
        assert_eq!(cache.get::<String>("key2").await.unwrap(), None);
        assert_eq!(cache.get::<String>("key3").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_statistics() {
        let (cache, _temp_dir) = create_test_cache().await;

        // Get initial stats
        let stats = cache.stats().await.unwrap();
        assert_eq!(stats.requests, 0);
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.hit_rate, 0.0);

        // Insert and access some values
        cache
            .insert("key1".to_string(), "value1".to_string())
            .await
            .unwrap();
        cache
            .insert("key2".to_string(), "value2".to_string())
            .await
            .unwrap();

        // Perform some gets (hits and misses)
        assert_eq!(
            cache.get::<String>("key1").await.unwrap(),
            Some("value1".to_string())
        ); // hit
        assert_eq!(
            cache.get::<String>("key2").await.unwrap(),
            Some("value2".to_string())
        ); // hit
        assert_eq!(cache.get::<String>("key3").await.unwrap(), None); // miss
        assert_eq!(
            cache.get::<String>("key1").await.unwrap(),
            Some("value1".to_string())
        ); // hit

        let stats = cache.stats().await.unwrap();
        assert_eq!(stats.requests, 4);
        assert_eq!(stats.hits, 3);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hit_rate, 75.0);
    }

    #[tokio::test]
    async fn test_persistence() {
        let temp_dir = TempDir::new().unwrap();

        // Create cache, insert data, then drop it
        {
            let cache = DiskCache::new(temp_dir.path(), Duration::from_secs(60), 1024 * 1024)
                .await
                .unwrap();

            cache
                .insert("persistent_key".to_string(), "persistent_value".to_string())
                .await
                .unwrap();
        }

        // Create new cache instance with same directory
        let cache = DiskCache::new(temp_dir.path(), Duration::from_secs(60), 1024 * 1024)
            .await
            .unwrap();

        // Data should still be there
        assert_eq!(
            cache.get::<String>("persistent_key").await.unwrap(),
            Some("persistent_value".to_string())
        );
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let (cache, _temp_dir) = create_test_cache().await;
        let mut handles = vec![];

        // Spawn multiple tasks that insert and read concurrently
        for i in 0..10 {
            let cache_clone = cache.clone();
            let handle = tokio::spawn(async move {
                let key = format!("key{}", i);
                let value = format!("value{}", i);

                cache_clone
                    .insert(key.clone(), value.clone())
                    .await
                    .unwrap();
                assert_eq!(cache_clone.get::<String>(&key).await.unwrap(), Some(value));
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            timeout(Duration::from_secs(5), handle)
                .await
                .unwrap()
                .unwrap();
        }
    }

    #[tokio::test]
    async fn test_different_value_types() {
        let (cache, _temp_dir) = create_test_cache().await;

        // Test with different serializable types
        cache
            .insert("string_key".to_string(), "string_value".to_string())
            .await
            .unwrap();
        cache.insert("number_key".to_string(), 42i32).await.unwrap();
        cache.insert("bool_key".to_string(), true).await.unwrap();

        let complex_value = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        cache
            .insert("vec_key".to_string(), complex_value.clone())
            .await
            .unwrap();

        // Retrieve and verify
        assert_eq!(
            cache.get::<String>("string_key").await.unwrap(),
            Some("string_value".to_string())
        );
        assert_eq!(cache.get::<i32>("number_key").await.unwrap(), Some(42));
        assert_eq!(cache.get::<bool>("bool_key").await.unwrap(), Some(true));
        assert_eq!(
            cache.get::<Vec<String>>("vec_key").await.unwrap(),
            Some(complex_value)
        );
    }

    #[tokio::test]
    async fn test_update_existing_key() {
        let (cache, _temp_dir) = create_test_cache().await;

        // Insert initial value
        cache
            .insert("key1".to_string(), "value1".to_string())
            .await
            .unwrap();
        assert_eq!(
            cache.get::<String>("key1").await.unwrap(),
            Some("value1".to_string())
        );

        // Update with new value
        cache
            .insert("key1".to_string(), "value2".to_string())
            .await
            .unwrap();
        assert_eq!(
            cache.get::<String>("key1").await.unwrap(),
            Some("value2".to_string())
        );
    }

    #[tokio::test]
    async fn test_ttl_precision_levels() {
        // Test TTL precision across different time scales
        let ttl_scenarios = vec![
            (Duration::from_millis(50), "50ms"),
            (Duration::from_millis(100), "100ms"),
            (Duration::from_millis(500), "500ms"),
            (Duration::from_secs(1), "1s"),
            (Duration::from_secs(5), "5s"),
        ];

        for (ttl, description) in ttl_scenarios {
            let temp_dir = TempDir::new().unwrap();
            let cache = DiskCache::new(temp_dir.path(), ttl, 1024 * 1024)
                .await
                .unwrap();

            // Insert a value
            let key = format!("test_key_{}", description);
            let value = format!("test_value_{}", description);

            cache.insert(key.clone(), value.clone()).await.unwrap();

            // Verify it exists immediately
            assert_eq!(
                cache.get::<String>(&key).await.unwrap(),
                Some(value.clone()),
                "Value should exist immediately for TTL {}",
                description
            );
            assert!(
                cache.contains(&key).await,
                "Cache should contain key for TTL {}",
                description
            );

            // Wait for 75% of TTL and verify still exists
            let check_duration = Duration::from_millis((ttl.as_millis() as f64 * 0.75) as u64);
            sleep(check_duration).await;

            assert_eq!(
                cache.get::<String>(&key).await.unwrap(),
                Some(value),
                "Value should still exist at 75% TTL for {}",
                description
            );

            // Wait for TTL to expire with buffer
            let remaining_duration = ttl - check_duration + Duration::from_millis(50);
            sleep(remaining_duration).await;

            // Value should be expired and automatically removed
            assert_eq!(
                cache.get::<String>(&key).await.unwrap(),
                None,
                "Value should be expired for TTL {}",
                description
            );
            assert!(
                !cache.contains(&key).await,
                "Cache should not contain expired key for TTL {}",
                description
            );
        }
    }

    #[tokio::test]
    async fn test_ttl_edge_cases() {
        let temp_dir = TempDir::new().unwrap();
        let cache = DiskCache::new(temp_dir.path(), Duration::from_millis(100), 1024 * 1024)
            .await
            .unwrap();

        // Test rapid insertion and expiration
        for i in 0..5 {
            let key = format!("rapid_key_{}", i);
            let value = format!("rapid_value_{}", i);

            cache.insert(key.clone(), value.clone()).await.unwrap();

            // Sleep just enough to expire
            sleep(Duration::from_millis(120)).await;

            assert_eq!(
                cache.get::<String>(&key).await.unwrap(),
                None,
                "Rapid test key {} should be expired",
                i
            );
        }

        // Test mixed fresh and expired entries
        cache
            .insert("fresh".to_string(), "fresh_value".to_string())
            .await
            .unwrap();
        cache
            .insert("expire1".to_string(), "expire_value1".to_string())
            .await
            .unwrap();
        cache
            .insert("expire2".to_string(), "expire_value2".to_string())
            .await
            .unwrap();

        // Wait for first two to expire
        sleep(Duration::from_millis(120)).await;

        // Add fresh entry
        cache
            .insert("fresh2".to_string(), "fresh_value2".to_string())
            .await
            .unwrap();

        // Verify state
        assert_eq!(cache.get::<String>("fresh").await.unwrap(), None);
        assert_eq!(cache.get::<String>("expire1").await.unwrap(), None);
        assert_eq!(cache.get::<String>("expire2").await.unwrap(), None);
        assert_eq!(
            cache.get::<String>("fresh2").await.unwrap(),
            Some("fresh_value2".to_string())
        );
    }

    #[tokio::test]
    async fn test_cleanup_expired_comprehensive() {
        let temp_dir = TempDir::new().unwrap();
        let cache = DiskCache::new(temp_dir.path(), Duration::from_millis(100), 1024 * 1024)
            .await
            .unwrap();

        // Insert entries at different times
        cache
            .insert("early1".to_string(), "value1".to_string())
            .await
            .unwrap();
        cache
            .insert("early2".to_string(), "value2".to_string())
            .await
            .unwrap();

        // Wait for these to be close to expiration
        sleep(Duration::from_millis(80)).await;

        cache
            .insert("mid".to_string(), "mid_value".to_string())
            .await
            .unwrap();

        // Wait for early entries to expire
        sleep(Duration::from_millis(50)).await;

        cache
            .insert("late".to_string(), "late_value".to_string())
            .await
            .unwrap();

        // Manual cleanup should remove expired entries
        let expired_count = cache.cleanup_expired().await.unwrap();
        assert!(
            expired_count >= 2,
            "Should clean up at least 2 expired entries, got {}",
            expired_count
        );

        // Verify state after cleanup
        assert_eq!(cache.get::<String>("early1").await.unwrap(), None);
        assert_eq!(cache.get::<String>("early2").await.unwrap(), None);
        // Note: mid might also be expired by now due to timing
        assert_eq!(
            cache.get::<String>("late").await.unwrap(),
            Some("late_value".to_string())
        );
    }

    #[tokio::test]
    async fn test_ttl_boundary_conditions() {
        let temp_dir = TempDir::new().unwrap();
        let cache = DiskCache::new(temp_dir.path(), Duration::from_millis(100), 1024 * 1024)
            .await
            .unwrap();

        cache
            .insert("boundary".to_string(), "boundary_value".to_string())
            .await
            .unwrap();

        // Test at exactly TTL boundary (should be expired)
        sleep(Duration::from_millis(100)).await;

        // At exactly 100ms, entry should be expired
        assert_eq!(
            cache.get::<String>("boundary").await.unwrap(),
            None,
            "Entry should be expired at exact TTL boundary"
        );

        // Test just before expiration with safer timing
        cache
            .insert("boundary2".to_string(), "boundary_value2".to_string())
            .await
            .unwrap();
        sleep(Duration::from_millis(50)).await; // Well before expiration

        // Should still be valid well before expiration
        assert_eq!(
            cache.get::<String>("boundary2").await.unwrap(),
            Some("boundary_value2".to_string()),
            "Entry should be valid well before expiration"
        );

        // Wait for expiration with buffer
        sleep(Duration::from_millis(70)).await; // Total 120ms > 100ms TTL

        assert_eq!(
            cache.get::<String>("boundary2").await.unwrap(),
            None,
            "Entry should be expired after TTL"
        );
    }

    #[tokio::test]
    async fn test_concurrent_ttl_operations() {
        let temp_dir = TempDir::new().unwrap();
        let cache = std::sync::Arc::new(
            DiskCache::new(temp_dir.path(), Duration::from_millis(200), 1024 * 1024)
                .await
                .unwrap(),
        );

        let mut handles = vec![];

        // Spawn multiple tasks that insert and immediately check entries
        for i in 0..10 {
            let cache_clone = cache.clone();
            let handle = tokio::spawn(async move {
                let key = format!("concurrent_key_{}", i);
                let value = format!("concurrent_value_{}", i);

                // Insert
                cache_clone
                    .insert(key.clone(), value.clone())
                    .await
                    .unwrap();

                // Immediately verify
                assert_eq!(cache_clone.get::<String>(&key).await.unwrap(), Some(value));

                // Wait for expiration
                sleep(Duration::from_millis(250)).await;

                // Verify expired
                assert_eq!(cache_clone.get::<String>(&key).await.unwrap(), None);
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify cache is empty after all expirations
        let stats = cache.stats().await.unwrap();
        assert_eq!(
            stats.size, 0,
            "Cache should be empty after all entries expired"
        );
    }
}
