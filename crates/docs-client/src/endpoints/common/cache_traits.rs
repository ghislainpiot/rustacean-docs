use rustacean_docs_cache::MemoryCache;
use std::{hash::Hash, sync::Arc, time::Duration};

/// Trait for cache key types that can be used across different endpoints
pub trait CacheKey: Clone + PartialEq + Eq + Hash + Send + Sync + 'static {}

/// Service cache configuration
#[derive(Debug, Clone)]
pub struct ServiceCacheConfig {
    pub capacity: usize,
    pub ttl: Duration,
}

impl Default for ServiceCacheConfig {
    fn default() -> Self {
        Self {
            capacity: 1000,
            ttl: Duration::from_secs(3600), // 1 hour
        }
    }
}

/// Create a new service cache with the given configuration
pub fn create_service_cache<K, V>(config: ServiceCacheConfig) -> Arc<MemoryCache<K, V>>
where
    K: CacheKey,
    V: Clone + Send + Sync + 'static,
{
    Arc::new(MemoryCache::new(config.capacity))
}

// Implement CacheKey for common types
impl<T> CacheKey for T where T: Clone + PartialEq + Eq + Hash + Send + Sync + 'static {}

#[cfg(test)]
mod tests {
    use super::*;
    use rustacean_docs_cache::Cache;

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct TestKey {
        id: String,
    }

    #[derive(Debug, Clone)]
    struct TestValue {
        data: String,
    }

    #[tokio::test]
    async fn test_service_cache_operations() {
        let config = ServiceCacheConfig::default();
        let cache = create_service_cache::<TestKey, TestValue>(config);

        let key = TestKey {
            id: "test".to_string(),
        };
        let value = TestValue {
            data: "test_data".to_string(),
        };

        // Test insert and get
        cache.insert(key.clone(), value.clone()).await.unwrap();
        let retrieved = cache.get(&key).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().data, "test_data");

        // Test stats
        let stats = cache.stats();
        assert_eq!(stats.size, 1);

        // Test clear
        cache.clear().await.unwrap();
        let stats = cache.stats();
        assert_eq!(stats.size, 0);
    }

    #[test]
    fn test_service_cache_config_default() {
        let config = ServiceCacheConfig::default();
        assert_eq!(config.capacity, 1000);
        assert_eq!(config.ttl, Duration::from_secs(3600));
    }
}
