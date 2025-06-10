use async_trait::async_trait;
use std::fmt::Debug;

pub mod disk;
pub mod memory;
pub mod tiered;

pub use disk::DiskCache;
pub use memory::MemoryCache;
pub use tiered::{TieredCache, WriteStrategy};

/// Simplified cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub size: usize,
    pub capacity: usize,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

/// Unified cache trait that all cache implementations follow
#[async_trait]
pub trait Cache: Send + Sync {
    type Key: Send + Sync;
    type Value: Send + Sync;
    type Error: Send + Sync + 'static;

    /// Get a value from the cache
    async fn get(&self, key: &Self::Key) -> Result<Option<Self::Value>, Self::Error>;

    /// Insert a value into the cache
    async fn insert(&self, key: Self::Key, value: Self::Value) -> Result<(), Self::Error>;

    /// Remove a value from the cache
    async fn remove(&self, key: &Self::Key) -> Result<(), Self::Error>;

    /// Clear all values from the cache
    async fn clear(&self) -> Result<(), Self::Error>;

    /// Get cache statistics (non-async for simplicity)
    fn stats(&self) -> CacheStats;
}