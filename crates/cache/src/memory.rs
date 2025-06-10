use crate::{Cache, CacheStats};
use async_trait::async_trait;
use lru::LruCache;
use std::convert::Infallible;
use std::hash::Hash;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::RwLock;

/// In-memory LRU cache implementation
pub struct MemoryCache<K, V> {
    cache: Arc<RwLock<LruCache<K, V>>>,
    stats: Arc<RwLock<CacheStats>>,
}

impl<K, V> MemoryCache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Create a new memory cache with the specified capacity
    pub fn new(capacity: usize) -> Self {
        let cap = NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(1000).unwrap());
        Self {
            cache: Arc::new(RwLock::new(LruCache::new(cap))),
            stats: Arc::new(RwLock::new(CacheStats {
                capacity,
                ..Default::default()
            })),
        }
    }
}

#[async_trait]
impl<K, V> Cache for MemoryCache<K, V>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    type Key = K;
    type Value = V;
    type Error = Infallible;

    async fn get(&self, key: &Self::Key) -> Result<Option<Self::Value>, Self::Error> {
        let mut cache = self.cache.write().await;
        let mut stats = self.stats.write().await;

        match cache.get(key) {
            Some(value) => {
                stats.hits += 1;
                Ok(Some(value.clone()))
            }
            None => {
                stats.misses += 1;
                Ok(None)
            }
        }
    }

    async fn insert(&self, key: Self::Key, value: Self::Value) -> Result<(), Self::Error> {
        let mut cache = self.cache.write().await;
        let mut stats = self.stats.write().await;

        cache.put(key, value);
        stats.size = cache.len();

        Ok(())
    }

    async fn remove(&self, key: &Self::Key) -> Result<(), Self::Error> {
        let mut cache = self.cache.write().await;
        let mut stats = self.stats.write().await;

        cache.pop(key);
        stats.size = cache.len();

        Ok(())
    }

    async fn clear(&self) -> Result<(), Self::Error> {
        let mut cache = self.cache.write().await;
        let mut stats = self.stats.write().await;

        cache.clear();
        stats.size = 0;

        Ok(())
    }

    fn stats(&self) -> CacheStats {
        // Use try_read to avoid potential deadlock in stats() call
        self.stats.try_read().map(|s| s.clone()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_cache_basic_operations() {
        let cache = MemoryCache::<String, String>::new(10);

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
        assert_eq!(stats.size, 0);
    }

    #[tokio::test]
    async fn test_memory_cache_lru_eviction() {
        let cache = MemoryCache::<i32, i32>::new(3);

        // Fill cache
        for i in 0..3 {
            cache.insert(i, i * 10).await.unwrap();
        }

        // Access first item to make it recently used
        assert_eq!(cache.get(&0).await.unwrap(), Some(0));

        // Insert new item, should evict least recently used (1)
        cache.insert(3, 30).await.unwrap();

        // Check that 1 was evicted
        assert_eq!(cache.get(&1).await.unwrap(), None);
        assert_eq!(cache.get(&0).await.unwrap(), Some(0));
        assert_eq!(cache.get(&2).await.unwrap(), Some(20));
        assert_eq!(cache.get(&3).await.unwrap(), Some(30));

        let stats = cache.stats();
        assert_eq!(stats.size, 3);
    }

    #[tokio::test]
    async fn test_memory_cache_clear() {
        let cache = MemoryCache::<String, String>::new(10);

        // Add some items
        for i in 0..5 {
            cache
                .insert(format!("key{}", i), format!("value{}", i))
                .await
                .unwrap();
        }

        let stats = cache.stats();
        assert_eq!(stats.size, 5);

        // Clear cache
        cache.clear().await.unwrap();

        let stats = cache.stats();
        assert_eq!(stats.size, 0);

        // Verify items are gone
        assert_eq!(cache.get(&"key0".to_string()).await.unwrap(), None);
    }
}
