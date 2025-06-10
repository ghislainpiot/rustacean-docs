use crate::{Cache, CacheStats};
use async_trait::async_trait;
use std::fmt::{self, Debug};

/// Write strategy for tiered cache
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteStrategy {
    /// Write to all cache layers
    WriteThrough,
    /// Write only to the first layer
    WriteBack,
}

/// A tiered cache that combines multiple cache layers
pub struct TieredCache<K, V> {
    layers: Vec<Box<dyn Cache<Key = K, Value = V, Error = anyhow::Error>>>,
    write_strategy: WriteStrategy,
}

impl<K, V> TieredCache<K, V>
where
    K: Send + Sync,
    V: Send + Sync,
{
    /// Create a new tiered cache with the given layers and write strategy
    pub fn new(
        layers: Vec<Box<dyn Cache<Key = K, Value = V, Error = anyhow::Error>>>,
        write_strategy: WriteStrategy,
    ) -> Self {
        Self {
            layers,
            write_strategy,
        }
    }

    /// Create a builder for constructing a tiered cache
    pub fn builder() -> TieredCacheBuilder<K, V> {
        TieredCacheBuilder {
            layers: Vec::new(),
            write_strategy: WriteStrategy::WriteThrough,
        }
    }
}

impl<K, V> Debug for TieredCache<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TieredCache")
            .field("num_layers", &self.layers.len())
            .field("write_strategy", &self.write_strategy)
            .finish()
    }
}

#[async_trait]
impl<K, V> Cache for TieredCache<K, V>
where
    K: Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    type Key = K;
    type Value = V;
    type Error = anyhow::Error;

    async fn get(&self, key: &Self::Key) -> Result<Option<Self::Value>, Self::Error> {
        for (index, layer) in self.layers.iter().enumerate() {
            match layer.get(key).await {
                Ok(Some(value)) => {
                    // Promote to higher priority layers
                    for i in 0..index {
                        // Ignore errors during promotion
                        let _ = self.layers[i].insert(key.clone(), value.clone()).await;
                    }
                    return Ok(Some(value));
                }
                Ok(None) => continue,
                Err(e) => {
                    // Log error but continue to next layer
                    tracing::warn!("Error reading from cache layer {}: {}", index, e);
                    continue;
                }
            }
        }
        Ok(None)
    }

    async fn insert(&self, key: Self::Key, value: Self::Value) -> Result<(), Self::Error> {
        match self.write_strategy {
            WriteStrategy::WriteThrough => {
                // Write to all layers
                for layer in &self.layers {
                    let key_clone = key.clone();
                    let value_clone = value.clone();
                    
                    match layer.insert(key_clone, value_clone).await {
                        Ok(_) => {},
                        Err(e) => {
                            tracing::warn!("Failed to write to cache layer: {}", e);
                        }
                    }
                }
                Ok(())
            }
            WriteStrategy::WriteBack => {
                // Write only to the first layer
                if let Some(first_layer) = self.layers.first() {
                    first_layer.insert(key, value).await
                } else {
                    Ok(())
                }
            }
        }
    }

    async fn remove(&self, key: &Self::Key) -> Result<(), Self::Error> {
        // Remove from all layers
        for layer in &self.layers {
            match layer.remove(key).await {
                Ok(_) => {},
                Err(e) => {
                    tracing::warn!("Failed to remove from cache layer: {}", e);
                }
            }
        }
        Ok(())
    }

    async fn clear(&self) -> Result<(), Self::Error> {
        // Clear all layers
        for layer in &self.layers {
            match layer.clear().await {
                Ok(_) => {},
                Err(e) => {
                    tracing::warn!("Failed to clear cache layer: {}", e);
                }
            }
        }
        Ok(())
    }

    fn stats(&self) -> CacheStats {
        // Combine stats from all layers
        let mut combined = CacheStats::default();
        
        for layer in &self.layers {
            let layer_stats = layer.stats();
            combined.hits += layer_stats.hits;
            combined.misses += layer_stats.misses;
            combined.size += layer_stats.size;
            combined.capacity = combined.capacity.saturating_add(layer_stats.capacity);
        }
        
        combined
    }
}

/// Builder for constructing a TieredCache
pub struct TieredCacheBuilder<K, V> {
    layers: Vec<Box<dyn Cache<Key = K, Value = V, Error = anyhow::Error>>>,
    write_strategy: WriteStrategy,
}

impl<K, V> TieredCacheBuilder<K, V>
where
    K: Send + Sync,
    V: Send + Sync,
{
    /// Add a cache layer
    pub fn add_layer(
        mut self,
        layer: Box<dyn Cache<Key = K, Value = V, Error = anyhow::Error>>,
    ) -> Self {
        self.layers.push(layer);
        self
    }

    /// Set the write strategy
    pub fn write_strategy(mut self, strategy: WriteStrategy) -> Self {
        self.write_strategy = strategy;
        self
    }

    /// Build the tiered cache
    pub fn build(self) -> TieredCache<K, V> {
        TieredCache {
            layers: self.layers,
            write_strategy: self.write_strategy,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DiskCache, MemoryCache};
    use std::convert::Infallible;
    use tempfile::TempDir;

    // Helper to convert MemoryCache error type to anyhow::Error
    struct MemoryCacheWrapper<K, V>(MemoryCache<K, V>);

    #[async_trait]
    impl<K, V> Cache for MemoryCacheWrapper<K, V>
    where
        K: std::hash::Hash + Eq + Clone + Send + Sync,
        V: Clone + Send + Sync,
    {
        type Key = K;
        type Value = V;
        type Error = anyhow::Error;

        async fn get(&self, key: &Self::Key) -> Result<Option<Self::Value>, Self::Error> {
            self.0.get(key).await.map_err(|_: Infallible| unreachable!())
        }

        async fn insert(&self, key: Self::Key, value: Self::Value) -> Result<(), Self::Error> {
            self.0.insert(key, value).await.map_err(|_: Infallible| unreachable!())
        }

        async fn remove(&self, key: &Self::Key) -> Result<(), Self::Error> {
            self.0.remove(key).await.map_err(|_: Infallible| unreachable!())
        }

        async fn clear(&self) -> Result<(), Self::Error> {
            self.0.clear().await.map_err(|_: Infallible| unreachable!())
        }

        fn stats(&self) -> CacheStats {
            self.0.stats()
        }
    }

    #[tokio::test]
    async fn test_tiered_cache_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        
        let memory = MemoryCacheWrapper(MemoryCache::<String, String>::new(10));
        let disk = DiskCache::<String, String>::new(temp_dir.path());

        let cache = TieredCache::builder()
            .add_layer(Box::new(memory))
            .add_layer(Box::new(disk))
            .write_strategy(WriteStrategy::WriteThrough)
            .build();

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
    }

    #[tokio::test]
    async fn test_tiered_cache_promotion() {
        let temp_dir = TempDir::new().unwrap();
        
        let memory = MemoryCacheWrapper(MemoryCache::<String, String>::new(10));
        let disk = DiskCache::<String, String>::new(temp_dir.path());

        // Insert only to disk cache
        disk.insert("promoted".to_string(), "value".to_string())
            .await
            .unwrap();

        let cache = TieredCache::builder()
            .add_layer(Box::new(memory))
            .add_layer(Box::new(disk))
            .build();

        // First get should promote to memory
        let result = cache.get(&"promoted".to_string()).await.unwrap();
        assert_eq!(result, Some("value".to_string()));

        // Check stats to verify promotion
        let stats = cache.stats();
        assert!(stats.hits > 0);
    }

    #[tokio::test]
    async fn test_tiered_cache_write_strategies() {
        let temp_dir = TempDir::new().unwrap();
        
        // Test WriteBack strategy
        {
            let memory = MemoryCacheWrapper(MemoryCache::<String, String>::new(10));
            let disk = DiskCache::<String, String>::new(temp_dir.path());

            let cache = TieredCache::builder()
                .add_layer(Box::new(memory))
                .add_layer(Box::new(disk))
                .write_strategy(WriteStrategy::WriteBack)
                .build();

            cache
                .insert("writeback".to_string(), "value".to_string())
                .await
                .unwrap();

            // Should only be in memory layer
            let _memory_only = MemoryCacheWrapper(MemoryCache::<String, String>::new(10));
            let disk_only = DiskCache::<String, String>::new(temp_dir.path());
            
            // Disk should not have the value with WriteBack
            assert_eq!(disk_only.get(&"writeback".to_string()).await.unwrap(), None);
        }
    }
}