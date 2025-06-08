use crate::{
    client::DocsClient,
    error_handling::{handle_http_response, parse_json_response, build_docs_url}
};
use chrono::{DateTime, Utc};
use rustacean_docs_cache::memory::MemoryCache;
use rustacean_docs_core::{
    error::Error,
    models::docs::{CrateRelease, RecentReleasesRequest, RecentReleasesResponse},
};
use serde::Deserialize;
use std::{hash::Hash, sync::Arc, time::Duration};
use tracing::{debug, error, trace};

/// Raw response from crates.io API for recent crates
#[derive(Debug, Deserialize)]
struct CratesIoRecentResponse {
    crates: Vec<CratesIoRecentCrate>,
    meta: CratesIoMeta,
}

/// Individual crate data from crates.io recent updates API
#[derive(Debug, Deserialize)]
struct CratesIoRecentCrate {
    #[serde(rename = "id")]
    name: String,
    #[serde(rename = "newest_version")]
    version: String,
    description: Option<String>,
    updated_at: String,
    #[serde(rename = "downloads")]
    _total_downloads: u64,
    #[serde(rename = "recent_downloads")]
    _recent_downloads: Option<u64>,
}

/// Metadata from crates.io API response
#[derive(Debug, Deserialize)]
struct CratesIoMeta {
    total: usize,
}

/// Cache key for releases requests
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReleasesCacheKey {
    limit: usize,
}

impl ReleasesCacheKey {
    fn new(request: &RecentReleasesRequest) -> Self {
        Self {
            limit: request.limit(),
        }
    }
}

/// Releases service for fetching recent crate releases
pub struct ReleasesService {
    client: DocsClient,
    cache: Arc<MemoryCache<ReleasesCacheKey, RecentReleasesResponse>>,
}

impl ReleasesService {
    pub fn new(client: DocsClient) -> Self {
        // Create a cache with 100 capacity and 30 minutes TTL (shorter since releases change frequently)
        let cache = Arc::new(MemoryCache::new(
            100,
            Duration::from_secs(1800), // 30 minutes
        ));

        Self { client, cache }
    }

    /// Fetch recent releases using crates.io API
    pub async fn get_recent_releases(
        &self,
        request: &RecentReleasesRequest,
    ) -> Result<RecentReleasesResponse, Error> {
        let cache_key = ReleasesCacheKey::new(request);

        // Try to get from cache first
        if let Some(cached_response) = self.cache.get(&cache_key).await {
            trace!(
                limit = request.limit(),
                "Recent releases cache hit"
            );
            return Ok(cached_response);
        }

        trace!(
            limit = request.limit(),
            "Recent releases cache miss, fetching from API"
        );

        let releases = self.fetch_releases_from_api(request).await?;
        let response = RecentReleasesResponse { releases };

        // Store in cache for future requests
        self.cache.insert(cache_key, response.clone()).await;

        debug!(
            limit = request.limit(),
            total_releases = response.releases.len(),
            "Recent releases fetched and cached successfully"
        );

        Ok(response)
    }

    async fn fetch_releases_from_api(
        &self,
        request: &RecentReleasesRequest,
    ) -> Result<Vec<CrateRelease>, Error> {
        // Use crates.io API to get recently updated crates
        let url = format!(
            "https://crates.io/api/v1/crates?sort=recent-updates&per_page={}",
            request.limit()
        );

        debug!("Requesting recent releases from: {}", url);

        let response = self
            .client
            .inner_client()
            .get(&url)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to fetch recent releases from crates.io: {}", e);
                Error::Network(e)
            })?;

        let response = handle_http_response(response, "crates.io recent releases").await?;
        let crates_io_response: CratesIoRecentResponse = parse_json_response(response, "crates.io recent releases").await?;

        debug!(
            total_crates = crates_io_response.meta.total,
            returned_crates = crates_io_response.crates.len(),
            "Successfully fetched recent releases from crates.io"
        );

        // Transform the response to our internal format
        let releases = crates_io_response
            .crates
            .into_iter()
            .map(|crate_data| self.transform_crate_to_release(crate_data))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(releases)
    }

    fn transform_crate_to_release(
        &self,
        crate_data: CratesIoRecentCrate,
    ) -> Result<CrateRelease, Error> {
        // Parse the updated_at timestamp
        let published_at = DateTime::parse_from_rfc3339(&crate_data.updated_at)
            .map_err(|e| {
                error!(
                    "Failed to parse timestamp '{}': {}",
                    crate_data.updated_at, e
                );
                Error::internal(format!("Invalid timestamp format: {e}"))
            })?
            .with_timezone(&Utc);

        // Generate docs.rs URL for this crate and version
        let docs_url = build_docs_url(&crate_data.name, &crate_data.version)?;

        Ok(CrateRelease {
            name: crate_data.name,
            version: crate_data.version,
            description: crate_data.description,
            published_at,
            docs_url: Some(docs_url),
        })
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> rustacean_docs_core::CacheLayerStats {
        self.cache.stats().await
    }

    /// Clear the entire cache
    pub async fn clear_cache(&self) -> usize {
        self.cache.clear().await
    }

    /// Clean up expired cache entries
    pub async fn cleanup_expired(&self) -> usize {
        self.cache.cleanup_expired().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_client() -> DocsClient {
        DocsClient::test_client().expect("Failed to create test client")
    }

    #[test]
    fn test_releases_cache_key() {
        let req1 = RecentReleasesRequest::new();
        let req2 = RecentReleasesRequest::with_limit(10);

        let key1 = ReleasesCacheKey::new(&req1);
        let key2 = ReleasesCacheKey::new(&req2);

        assert_eq!(key1.limit, 20); // Default limit
        assert_eq!(key2.limit, 10);
    }

    #[test]
    fn test_releases_service_creation() {
        let client = create_test_client();
        let _service = ReleasesService::new(client);

        // Basic verification that service can be created
        assert!(true);
    }

    #[test]
    fn test_transform_crate_to_release() {
        let client = create_test_client();
        let service = ReleasesService::new(client);

        let crate_data = CratesIoRecentCrate {
            name: "serde".to_string(),
            version: "1.0.195".to_string(),
            description: Some("A serialization framework".to_string()),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            _total_downloads: 1000000,
            _recent_downloads: Some(50000),
        };

        let result = service.transform_crate_to_release(crate_data);
        assert!(result.is_ok());

        let release = result.unwrap();
        assert_eq!(release.name, "serde");
        assert_eq!(release.version, "1.0.195");
        assert_eq!(
            release.description,
            Some("A serialization framework".to_string())
        );
        assert!(release.docs_url.is_some());
        assert_eq!(
            release.docs_url.unwrap().as_str(),
            "https://docs.rs/serde/1.0.195/serde/"
        );
    }

    #[test]
    fn test_transform_crate_with_invalid_timestamp() {
        let client = create_test_client();
        let service = ReleasesService::new(client);

        let crate_data = CratesIoRecentCrate {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            updated_at: "invalid-timestamp".to_string(),
            _total_downloads: 100,
            _recent_downloads: None,
        };

        let result = service.transform_crate_to_release(crate_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_transform_crate_with_minimal_data() {
        let client = create_test_client();
        let service = ReleasesService::new(client);

        let crate_data = CratesIoRecentCrate {
            name: "minimal".to_string(),
            version: "0.1.0".to_string(),
            description: None,
            updated_at: "2024-01-01T12:00:00Z".to_string(),
            _total_downloads: 1,
            _recent_downloads: None,
        };

        let result = service.transform_crate_to_release(crate_data);
        assert!(result.is_ok());

        let release = result.unwrap();
        assert_eq!(release.name, "minimal");
        assert_eq!(release.version, "0.1.0");
        assert_eq!(release.description, None);
        assert!(release.docs_url.is_some());
    }
}
