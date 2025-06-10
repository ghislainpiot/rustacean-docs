use rustacean_docs_core::models::docs::{CrateDocsRequest, ItemDocsRequest, RecentReleasesRequest};
use std::hash::Hash;

/// Cache key for crate documentation requests
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CrateDocsCacheKey {
    crate_name: String,
    version: Option<String>,
}

impl CrateDocsCacheKey {
    pub fn new(request: &CrateDocsRequest) -> Self {
        Self {
            crate_name: request.crate_name.clone(),
            version: request.version.clone(),
        }
    }
}

/// Cache key for item documentation requests
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ItemDocsCacheKey {
    crate_name: String,
    item_path: String,
    version: Option<String>,
}

impl ItemDocsCacheKey {
    pub fn new(request: &ItemDocsRequest) -> Self {
        Self {
            crate_name: request.crate_name.clone(),
            item_path: request.item_path.clone(),
            version: request.version.clone(),
        }
    }
}

/// Cache key for recent releases requests
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecentReleasesCacheKey {
    limit: usize,
}

impl RecentReleasesCacheKey {
    pub fn new(request: &RecentReleasesRequest) -> Self {
        Self {
            limit: request.limit(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crate_docs_cache_key() {
        let request1 = CrateDocsRequest::new("tokio");
        let request2 = CrateDocsRequest::with_version("tokio", "1.0.0");
        let request3 = CrateDocsRequest::new("serde");

        let key1 = CrateDocsCacheKey::new(&request1);
        let key2 = CrateDocsCacheKey::new(&request2);
        let key3 = CrateDocsCacheKey::new(&request3);

        assert_ne!(key1, key2); // Different versions
        assert_ne!(key1, key3); // Different crates

        // Keys should be hashable
        use std::collections::HashMap;
        let mut map = HashMap::new();
        map.insert(key1.clone(), "value1");
        map.insert(key2, "value2");
        map.insert(key3, "value3");

        assert_eq!(map.get(&key1), Some(&"value1"));
    }

    #[test]
    fn test_item_docs_cache_key() {
        let request1 = ItemDocsRequest::new("tokio", "spawn");
        let request2 = ItemDocsRequest::with_version("tokio", "spawn", "1.0.0");
        let request3 = ItemDocsRequest::new("tokio", "join");

        let key1 = ItemDocsCacheKey::new(&request1);
        let key2 = ItemDocsCacheKey::new(&request2);
        let key3 = ItemDocsCacheKey::new(&request3);

        assert_ne!(key1, key2); // Different versions
        assert_ne!(key1, key3); // Different items

        // Keys should be hashable
        use std::collections::HashMap;
        let mut map = HashMap::new();
        map.insert(key1.clone(), "value1");
        map.insert(key2, "value2");
        map.insert(key3, "value3");

        assert_eq!(map.get(&key1), Some(&"value1"));
    }

    #[test]
    fn test_recent_releases_cache_key() {
        let request1 = RecentReleasesRequest::new();
        let request2 = RecentReleasesRequest::with_limit(10);
        let request3 = RecentReleasesRequest::with_limit(20);

        let key1 = RecentReleasesCacheKey::new(&request1);
        let key2 = RecentReleasesCacheKey::new(&request2);
        let key3 = RecentReleasesCacheKey::new(&request3);

        assert_eq!(key1, key3); // Same limit (default is 20)
        assert_ne!(key1, key2); // Different limits (20 vs 10)
    }
}
