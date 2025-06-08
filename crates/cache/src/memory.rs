use lru::LruCache;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

#[allow(dead_code)]
pub struct MemoryCache<K, V> {
    cache: LruCache<K, (V, Instant)>,
    ttl: Duration,
}

impl<K, V> MemoryCache<K, V>
where
    K: std::hash::Hash + Eq,
{
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        Self {
            cache: LruCache::new(NonZeroUsize::new(capacity).unwrap()),
            ttl,
        }
    }
}
