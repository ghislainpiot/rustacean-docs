use super::cache_keys::{CrateDocsCacheKey, ItemDocsCacheKey, RecentReleasesCacheKey};
use crate::{
    client::DocsClient,
    html_parser::{parse_crate_documentation, parse_item_documentation, parse_recent_releases},
};
use rustacean_docs_cache::{Cache, MemoryCache};
use rustacean_docs_core::{
    models::docs::{
        CrateDocsRequest, CrateDocsResponse, ItemDocsRequest, ItemDocsResponse,
        RecentReleasesRequest, RecentReleasesResponse,
    },
    Result,
};
use std::{sync::Arc, time::Duration};
use tracing::{debug, trace};

/// Documentation service that combines HTTP client with caching
pub struct DocsService {
    client: DocsClient,
    crate_docs_cache: Arc<MemoryCache<CrateDocsCacheKey, CrateDocsResponse>>,
    item_docs_cache: Arc<MemoryCache<ItemDocsCacheKey, ItemDocsResponse>>,
    releases_cache: Arc<MemoryCache<RecentReleasesCacheKey, RecentReleasesResponse>>,
}

impl DocsService {
    /// Create a new documentation service with cache
    pub fn new(client: DocsClient, cache_capacity: usize, cache_ttl: Duration) -> Self {
        let crate_docs_cache = Arc::new(MemoryCache::new(cache_capacity));
        let item_docs_cache = Arc::new(MemoryCache::new(cache_capacity));
        let releases_cache = Arc::new(MemoryCache::new(cache_capacity / 10));

        debug!(
            cache_capacity = cache_capacity,
            cache_ttl_secs = cache_ttl.as_secs(),
            "Created documentation service with cache"
        );

        Self {
            client,
            crate_docs_cache,
            item_docs_cache,
            releases_cache,
        }
    }

    /// Get comprehensive crate documentation with caching
    pub async fn get_crate_docs(&self, request: CrateDocsRequest) -> Result<CrateDocsResponse> {
        let cache_key = CrateDocsCacheKey::new(&request);

        // Try to get from cache first
        if let Ok(Some(cached_response)) = self.crate_docs_cache.get(&cache_key).await {
            trace!(
                crate_name = %request.crate_name,
                version = ?request.version,
                "Crate docs cache hit"
            );
            return Ok(cached_response);
        }

        trace!(
            crate_name = %request.crate_name,
            version = ?request.version,
            "Crate docs cache miss, fetching from docs.rs"
        );

        // Cache miss - fetch from docs.rs
        let version = request.version.as_deref().unwrap_or("latest");
        let path = format!(
            "/{}/{}/{}/",
            request.crate_name, version, request.crate_name
        );
        let html = self.client.get_text(&path).await?;
        let response = parse_crate_documentation(&html, &request.crate_name, &request.version)?;

        // Store in cache for future requests
        let _ = self
            .crate_docs_cache
            .insert(cache_key, response.clone())
            .await;

        debug!(
            crate_name = %response.name,
            version = %response.version,
            item_count = response.items.len(),
            "Crate documentation fetched and cached"
        );

        Ok(response)
    }

    /// Get specific item documentation with caching
    pub async fn get_item_docs(&self, request: ItemDocsRequest) -> Result<ItemDocsResponse> {
        let cache_key = ItemDocsCacheKey::new(&request);

        // Try to get from cache first
        if let Ok(Some(cached_response)) = self.item_docs_cache.get(&cache_key).await {
            trace!(
                crate_name = %request.crate_name,
                item_path = %request.item_path,
                version = ?request.version,
                "Item docs cache hit"
            );
            return Ok(cached_response);
        }

        trace!(
            crate_name = %request.crate_name,
            item_path = %request.item_path,
            version = ?request.version,
            "Item docs cache miss, fetching from docs.rs"
        );

        // Cache miss - fetch from docs.rs
        let version = request.version.as_deref().unwrap_or("latest");
        let url = format!("/{}/{}/{}", request.crate_name, version, request.item_path);
        let html = self.client.get_text(&url).await?;
        let response = parse_item_documentation(
            &html,
            &request.crate_name,
            &request.item_path,
            &request.version,
        )?;

        // Store in cache for future requests
        let _ = self
            .item_docs_cache
            .insert(cache_key, response.clone())
            .await;

        debug!(
            crate_name = %response.crate_name,
            item_name = %response.name,
            "Item documentation fetched and cached"
        );

        Ok(response)
    }

    /// Get recent releases with caching
    pub async fn get_recent_releases(
        &self,
        request: RecentReleasesRequest,
    ) -> Result<RecentReleasesResponse> {
        let cache_key = RecentReleasesCacheKey::new(&request);

        // Try to get from cache first
        if let Ok(Some(cached_response)) = self.releases_cache.get(&cache_key).await {
            trace!(limit = request.limit(), "Recent releases cache hit");
            return Ok(cached_response);
        }

        trace!(
            limit = request.limit(),
            "Recent releases cache miss, fetching from docs.rs"
        );

        // Cache miss - fetch from docs.rs
        let html = self.client.get_text("/").await?; // docs.rs homepage
        let releases = parse_recent_releases(&html, request.limit())?;
        let response = RecentReleasesResponse { releases };

        // Store in cache for future requests
        let _ = self
            .releases_cache
            .insert(cache_key, response.clone())
            .await;

        debug!(
            release_count = response.releases.len(),
            "Recent releases fetched and cached"
        );

        Ok(response)
    }

    /// Get cache statistics for all caches
    pub fn cache_stats(
        &self,
    ) -> (
        rustacean_docs_cache::CacheStats,
        rustacean_docs_cache::CacheStats,
        rustacean_docs_cache::CacheStats,
    ) {
        let crate_stats = self.crate_docs_cache.stats();
        let item_stats = self.item_docs_cache.stats();
        let releases_stats = self.releases_cache.stats();
        (crate_stats, item_stats, releases_stats)
    }

    /// Clear all documentation caches
    pub async fn clear_cache(&self) -> Result<()> {
        let _ = self.crate_docs_cache.clear().await;
        let _ = self.item_docs_cache.clear().await;
        let _ = self.releases_cache.clear().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn create_test_client() -> DocsClient {
        DocsClient::test_client().expect("Failed to create test client")
    }

    #[tokio::test]
    async fn test_docs_service_creation() {
        let client = create_test_client();
        let service = DocsService::new(client, 100, Duration::from_secs(3600));

        let (crate_stats, item_stats, releases_stats) = service.cache_stats();

        assert_eq!(crate_stats.size, 0);
        assert_eq!(crate_stats.capacity, 100);
        assert_eq!(item_stats.size, 0);
        assert_eq!(item_stats.capacity, 100);
        assert_eq!(releases_stats.size, 0);
        assert_eq!(releases_stats.capacity, 10); // releases cache is smaller
    }

    #[tokio::test]
    async fn test_docs_service_cache_operations() {
        let client = create_test_client();
        let service = DocsService::new(client, 10, Duration::from_secs(60));

        // Test cache clear
        service.clear_cache().await.unwrap();

        // Verify caches are empty by checking stats
        let (crate_stats, item_stats, releases_stats) = service.cache_stats();
        assert_eq!(crate_stats.size, 0);
        assert_eq!(item_stats.size, 0);
        assert_eq!(releases_stats.size, 0);
    }
}
