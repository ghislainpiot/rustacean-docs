use lru::LruCache;
use rustacean_docs_core::CacheLayerStats;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, trace};

/// Entry stored in the memory cache with value and creation timestamp
#[derive(Debug, Clone)]
struct CacheEntry<V> {
    value: V,
    created_at: Instant,
}

impl<V> CacheEntry<V> {
    fn new(value: V) -> Self {
        Self {
            value,
            created_at: Instant::now(),
        }
    }

    fn is_expired(&self, ttl: Duration) -> bool {
        self.created_at.elapsed() > ttl
    }
}

/// Statistics for tracking cache performance
#[derive(Debug, Clone, Default)]
struct CacheStatistics {
    hits: u64,
    misses: u64,
    evictions: u64,
    expirations: u64,
    requests: u64,
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

/// In-memory LRU cache with TTL support and statistics tracking
pub struct MemoryCache<K, V> {
    cache: RwLock<LruCache<K, CacheEntry<V>>>,
    ttl: Duration,
    capacity: usize,
    stats: RwLock<CacheStatistics>,
}

impl<K, V> MemoryCache<K, V>
where
    K: std::hash::Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    /// Create a new memory cache with specified capacity and TTL
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        debug!(
            "Creating memory cache with capacity {} and TTL {:?}",
            capacity, ttl
        );

        Self {
            cache: RwLock::new(LruCache::new(NonZeroUsize::new(capacity).unwrap())),
            ttl,
            capacity,
            stats: RwLock::new(CacheStatistics::default()),
        }
    }

    /// Get a value from the cache, checking for expiration
    pub async fn get(&self, key: &K) -> Option<V> {
        let mut stats = self.stats.write().await;
        stats.requests += 1;

        let mut cache = self.cache.write().await;

        if let Some(entry) = cache.get(key) {
            if entry.is_expired(self.ttl) {
                // Entry is expired, remove it
                trace!("Cache entry expired for key");
                cache.pop(key);
                stats.expirations += 1;
                stats.misses += 1;
                None
            } else {
                // Entry is valid
                trace!("Cache hit for key");
                stats.hits += 1;
                Some(entry.value.clone())
            }
        } else {
            // Cache miss
            trace!("Cache miss for key");
            stats.misses += 1;
            None
        }
    }

    /// Insert a value into the cache
    pub async fn insert(&self, key: K, value: V) -> Option<V> {
        let mut cache = self.cache.write().await;
        let entry = CacheEntry::new(value);

        let evicted = cache.put(key, entry).map(|old_entry| old_entry.value);

        if evicted.is_some() {
            let mut stats = self.stats.write().await;
            stats.evictions += 1;
            trace!("Cache eviction occurred due to capacity limit");
        }

        evicted
    }

    /// Remove a specific key from the cache
    pub async fn remove(&self, key: &K) -> Option<V> {
        let mut cache = self.cache.write().await;
        cache.pop(key).map(|entry| entry.value)
    }

    /// Clear all entries from the cache
    pub async fn clear(&self) -> usize {
        let mut cache = self.cache.write().await;
        let count = cache.len();
        cache.clear();

        debug!("Cleared {} items from memory cache", count);
        count
    }

    /// Get current cache statistics
    pub async fn stats(&self) -> CacheLayerStats {
        let stats = self.stats.read().await;
        let cache = self.cache.read().await;

        CacheLayerStats {
            size: cache.len(),
            capacity: self.capacity,
            requests: stats.requests,
            hits: stats.hits,
            misses: stats.misses,
            hit_rate: stats.hit_rate(),
            bytes_used: None, // Memory cache doesn't track bytes
            bytes_capacity: None,
        }
    }

    /// Check if the cache contains a non-expired entry for the key
    pub async fn contains(&self, key: &K) -> bool {
        let cache = self.cache.read().await;

        if let Some(entry) = cache.peek(key) {
            !entry.is_expired(self.ttl)
        } else {
            false
        }
    }

    /// Get the number of items currently in the cache
    pub async fn len(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }

    /// Check if the cache is empty
    pub async fn is_empty(&self) -> bool {
        let cache = self.cache.read().await;
        cache.is_empty()
    }

    /// Clean up expired entries from the cache
    pub async fn cleanup_expired(&self) -> usize {
        let mut cache = self.cache.write().await;
        let mut stats = self.stats.write().await;

        let mut expired_keys = Vec::new();

        // Collect expired keys
        for (key, entry) in cache.iter() {
            if entry.is_expired(self.ttl) {
                expired_keys.push(key.clone());
            }
        }

        // Remove expired entries
        let count = expired_keys.len();
        for key in expired_keys {
            cache.pop(&key);
        }

        stats.expirations += count as u64;

        if count > 0 {
            debug!("Cleaned up {} expired entries from memory cache", count);
        }

        count
    }

    /// Get cache capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get cache TTL
    pub fn ttl(&self) -> Duration {
        self.ttl
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::{sleep, timeout};

    #[tokio::test]
    async fn test_new_cache() {
        let cache: MemoryCache<String, String> = MemoryCache::new(10, Duration::from_secs(60));

        assert_eq!(cache.capacity(), 10);
        assert_eq!(cache.ttl(), Duration::from_secs(60));
        assert!(cache.is_empty().await);
        assert_eq!(cache.len().await, 0);
    }

    #[tokio::test]
    async fn test_basic_operations() {
        let cache = MemoryCache::new(5, Duration::from_secs(60));

        // Test insert and get
        assert_eq!(
            cache.insert("key1".to_string(), "value1".to_string()).await,
            None
        );
        assert_eq!(
            cache.get(&"key1".to_string()).await,
            Some("value1".to_string())
        );
        assert_eq!(cache.len().await, 1);

        // Test contains
        assert!(cache.contains(&"key1".to_string()).await);
        assert!(!cache.contains(&"nonexistent".to_string()).await);

        // Test remove
        assert_eq!(
            cache.remove(&"key1".to_string()).await,
            Some("value1".to_string())
        );
        assert_eq!(cache.get(&"key1".to_string()).await, None);
        assert!(cache.is_empty().await);
    }

    #[tokio::test]
    async fn test_lru_eviction() {
        let cache = MemoryCache::new(3, Duration::from_secs(60));

        // Fill cache to capacity
        cache.insert("key1".to_string(), "value1".to_string()).await;
        cache.insert("key2".to_string(), "value2".to_string()).await;
        cache.insert("key3".to_string(), "value3".to_string()).await;
        assert_eq!(cache.len().await, 3);

        // Access key1 to make it most recently used
        assert_eq!(
            cache.get(&"key1".to_string()).await,
            Some("value1".to_string())
        );

        // Insert key4, should evict key2 (least recently used)
        cache.insert("key4".to_string(), "value4".to_string()).await;
        assert_eq!(cache.len().await, 3);

        // key2 should be evicted, others should remain
        assert_eq!(cache.get(&"key2".to_string()).await, None);
        assert_eq!(
            cache.get(&"key1".to_string()).await,
            Some("value1".to_string())
        );
        assert_eq!(
            cache.get(&"key3".to_string()).await,
            Some("value3".to_string())
        );
        assert_eq!(
            cache.get(&"key4".to_string()).await,
            Some("value4".to_string())
        );
    }

    #[tokio::test]
    async fn test_ttl_expiration() {
        let cache = MemoryCache::new(5, Duration::from_millis(100));

        // Insert a value
        cache.insert("key1".to_string(), "value1".to_string()).await;
        assert_eq!(
            cache.get(&"key1".to_string()).await,
            Some("value1".to_string())
        );

        // Wait for TTL to expire
        sleep(Duration::from_millis(150)).await;

        // Value should be expired and automatically removed
        assert_eq!(cache.get(&"key1".to_string()).await, None);
        assert!(cache.is_empty().await);
    }

    #[tokio::test]
    async fn test_ttl_with_contains() {
        let cache = MemoryCache::new(5, Duration::from_millis(100));

        cache.insert("key1".to_string(), "value1".to_string()).await;
        assert!(cache.contains(&"key1".to_string()).await);

        // Wait for expiration
        sleep(Duration::from_millis(150)).await;

        // Should not contain expired key
        assert!(!cache.contains(&"key1".to_string()).await);
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let cache = MemoryCache::new(5, Duration::from_millis(100));

        // Insert multiple values
        cache.insert("key1".to_string(), "value1".to_string()).await;
        cache.insert("key2".to_string(), "value2".to_string()).await;
        cache.insert("key3".to_string(), "value3".to_string()).await;
        assert_eq!(cache.len().await, 3);

        // Wait for all entries to expire
        sleep(Duration::from_millis(150)).await;

        // Add a new entry that won't be expired
        cache.insert("key4".to_string(), "value4".to_string()).await;

        // Manual cleanup - should remove the 3 expired entries
        let expired_count = cache.cleanup_expired().await;
        assert_eq!(expired_count, 3);
        assert_eq!(cache.len().await, 1);
        assert_eq!(
            cache.get(&"key4".to_string()).await,
            Some("value4".to_string())
        );
    }

    #[tokio::test]
    async fn test_clear() {
        let cache = MemoryCache::new(5, Duration::from_secs(60));

        // Insert multiple values
        cache.insert("key1".to_string(), "value1".to_string()).await;
        cache.insert("key2".to_string(), "value2".to_string()).await;
        cache.insert("key3".to_string(), "value3".to_string()).await;
        assert_eq!(cache.len().await, 3);

        // Clear all
        let cleared_count = cache.clear().await;
        assert_eq!(cleared_count, 3);
        assert!(cache.is_empty().await);
    }

    #[tokio::test]
    async fn test_statistics() {
        let cache = MemoryCache::new(3, Duration::from_secs(60));

        // Get initial stats
        let stats = cache.stats().await;
        assert_eq!(stats.size, 0);
        assert_eq!(stats.capacity, 3);
        assert_eq!(stats.requests, 0);
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.hit_rate, 0.0);

        // Insert and access some values
        cache.insert("key1".to_string(), "value1".to_string()).await;
        cache.insert("key2".to_string(), "value2".to_string()).await;

        // Perform some gets (hits and misses)
        assert_eq!(
            cache.get(&"key1".to_string()).await,
            Some("value1".to_string())
        ); // hit
        assert_eq!(
            cache.get(&"key2".to_string()).await,
            Some("value2".to_string())
        ); // hit
        assert_eq!(cache.get(&"key3".to_string()).await, None); // miss
        assert_eq!(
            cache.get(&"key1".to_string()).await,
            Some("value1".to_string())
        ); // hit

        let stats = cache.stats().await;
        assert_eq!(stats.size, 2);
        assert_eq!(stats.capacity, 3);
        assert_eq!(stats.requests, 4);
        assert_eq!(stats.hits, 3);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hit_rate, 75.0);
    }

    #[tokio::test]
    async fn test_statistics_with_evictions() {
        let cache = MemoryCache::new(2, Duration::from_secs(60));

        // Fill cache and cause eviction
        cache.insert("key1".to_string(), "value1".to_string()).await;
        cache.insert("key2".to_string(), "value2".to_string()).await;
        cache.insert("key3".to_string(), "value3".to_string()).await; // Should evict key1

        let stats = cache.stats().await;
        assert_eq!(stats.size, 2);

        // Note: Eviction counting happens in the insert method, but we can't access
        // internal stats directly. We can test the behavior indirectly.
        assert_eq!(cache.get(&"key1".to_string()).await, None); // Evicted
        assert_eq!(
            cache.get(&"key2".to_string()).await,
            Some("value2".to_string())
        );
        assert_eq!(
            cache.get(&"key3".to_string()).await,
            Some("value3".to_string())
        );
    }

    #[tokio::test]
    async fn test_statistics_with_expirations() {
        let cache = MemoryCache::new(5, Duration::from_millis(50));

        // Insert some values
        cache.insert("key1".to_string(), "value1".to_string()).await;
        cache.insert("key2".to_string(), "value2".to_string()).await;

        // Wait for expiration
        sleep(Duration::from_millis(100)).await;

        // Access expired items (should count as expirations and misses)
        assert_eq!(cache.get(&"key1".to_string()).await, None);
        assert_eq!(cache.get(&"key2".to_string()).await, None);

        let stats = cache.stats().await;
        assert_eq!(stats.size, 0);
        assert_eq!(stats.requests, 2);
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 2);
        assert_eq!(stats.hit_rate, 0.0);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let cache = std::sync::Arc::new(MemoryCache::new(10, Duration::from_secs(60)));
        let mut handles = vec![];

        // Spawn multiple tasks that insert and read concurrently
        for i in 0..10 {
            let cache_clone = cache.clone();
            let handle = tokio::spawn(async move {
                let key = format!("key{}", i);
                let value = format!("value{}", i);

                cache_clone.insert(key.clone(), value.clone()).await;
                assert_eq!(cache_clone.get(&key).await, Some(value));
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

        assert_eq!(cache.len().await, 10);
    }

    #[tokio::test]
    async fn test_update_existing_key() {
        let cache = MemoryCache::new(5, Duration::from_secs(60));

        // Insert initial value
        assert_eq!(
            cache.insert("key1".to_string(), "value1".to_string()).await,
            None
        );
        assert_eq!(
            cache.get(&"key1".to_string()).await,
            Some("value1".to_string())
        );

        // Update with new value (should return old value)
        assert_eq!(
            cache.insert("key1".to_string(), "value2".to_string()).await,
            Some("value1".to_string())
        );
        assert_eq!(
            cache.get(&"key1".to_string()).await,
            Some("value2".to_string())
        );
        assert_eq!(cache.len().await, 1);
    }

    #[tokio::test]
    async fn test_zero_capacity_panic() {
        // This should panic due to NonZeroUsize requirement
        let result = std::panic::catch_unwind(|| {
            let _cache: MemoryCache<String, String> = MemoryCache::new(0, Duration::from_secs(60));
        });
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mixed_operations() {
        let cache = MemoryCache::new(3, Duration::from_millis(200));

        // Insert some values
        cache.insert("key1".to_string(), "value1".to_string()).await;
        cache.insert("key2".to_string(), "value2".to_string()).await;

        // Wait half TTL
        sleep(Duration::from_millis(100)).await;

        // Insert more (should not expire yet)
        cache.insert("key3".to_string(), "value3".to_string()).await;

        // Access existing keys (reset their position in LRU)
        assert_eq!(
            cache.get(&"key1".to_string()).await,
            Some("value1".to_string())
        );

        // Insert one more to trigger eviction (key2 should be evicted)
        cache.insert("key4".to_string(), "value4".to_string()).await;

        // Check current state
        assert_eq!(cache.len().await, 3);
        assert_eq!(cache.get(&"key2".to_string()).await, None); // Evicted
        assert_eq!(
            cache.get(&"key1".to_string()).await,
            Some("value1".to_string())
        );
        assert_eq!(
            cache.get(&"key3".to_string()).await,
            Some("value3".to_string())
        );
        assert_eq!(
            cache.get(&"key4".to_string()).await,
            Some("value4".to_string())
        );

        // Wait for first entries to expire
        sleep(Duration::from_millis(200)).await;

        // key1 should now be expired, but key3 and key4 might not be
        assert_eq!(cache.get(&"key1".to_string()).await, None);
    }
}
