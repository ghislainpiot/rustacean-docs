use crate::{
    client::DocsClient,
    error_handling::{handle_http_response, parse_json_response, build_docs_url}
};
use chrono::{DateTime, Utc};
use rustacean_docs_cache::memory::MemoryCache;
use rustacean_docs_core::{
    error::ErrorContext,
    models::search::{CrateSearchResult, SearchRequest, SearchResponse},
    Result, DEFAULT_VERSION,
};
use serde::{Deserialize, Serialize};
use std::{hash::Hash, sync::Arc, time::Duration};
use tracing::{debug, trace};
use url::Url;

/// Raw response from crates.io search API
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CratesIoSearchResponse {
    crates: Vec<CratesIoCrate>,
    meta: CratesIoMeta,
}

/// Metadata about the search response
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CratesIoMeta {
    total: usize,
    #[serde(rename = "next_page")]
    next_page: Option<String>,
    #[serde(rename = "prev_page")]
    prev_page: Option<String>,
}

/// Cache key for search requests
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SearchCacheKey {
    query: String,
    limit: usize,
}

impl SearchCacheKey {
    fn new(request: &SearchRequest) -> Self {
        Self {
            query: request.query.clone(),
            limit: request.limit(),
        }
    }
}

/// Search service that combines HTTP client with caching
pub struct SearchService {
    client: DocsClient,
    cache: Arc<MemoryCache<SearchCacheKey, SearchResponse>>,
}

impl SearchService {
    /// Create a new search service with cache
    pub fn new(client: DocsClient, cache_capacity: usize, cache_ttl: Duration) -> Self {
        let cache = Arc::new(MemoryCache::new(cache_capacity, cache_ttl));

        debug!(
            cache_capacity = cache_capacity,
            cache_ttl_secs = cache_ttl.as_secs(),
            "Created search service with cache"
        );

        Self { client, cache }
    }

    /// Search for crates with caching
    pub async fn search_crates(&self, request: SearchRequest) -> Result<SearchResponse> {
        let cache_key = SearchCacheKey::new(&request);

        // Try to get from cache first
        if let Some(cached_response) = self.cache.get(&cache_key).await {
            trace!(
                query = %request.query,
                limit = request.limit(),
                "Search cache hit"
            );
            return Ok(cached_response);
        }

        trace!(
            query = %request.query,
            limit = request.limit(),
            "Search cache miss, fetching from API"
        );

        // Cache miss - fetch from API
        let response = self.client.search_crates(request).await?;

        // Store in cache for future requests
        self.cache.insert(cache_key, response.clone()).await;

        debug!(
            query = %response.results.first().map(|r| &r.name).unwrap_or(&"none".to_string()),
            total_results = response.total.unwrap_or(0),
            returned_results = response.results.len(),
            "Search completed and cached"
        );

        Ok(response)
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> rustacean_docs_core::models::metadata::CacheLayerStats {
        self.cache.stats().await
    }

    /// Clear the search cache
    pub async fn clear_cache(&self) -> usize {
        self.cache.clear().await
    }

    /// Clean up expired cache entries
    pub async fn cleanup_expired(&self) -> usize {
        self.cache.cleanup_expired().await
    }
}

/// Individual crate data from crates.io API
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CratesIoCrate {
    #[serde(rename = "id")]
    id: String,
    #[serde(rename = "name")]
    name: String,
    #[serde(rename = "newest_version")]
    newest_version: String,
    #[serde(rename = "max_version")]
    max_version: String,
    #[serde(rename = "max_stable_version")]
    max_stable_version: Option<String>,
    #[serde(rename = "default_version")]
    default_version: String,
    #[serde(rename = "description")]
    description: Option<String>,
    #[serde(rename = "downloads")]
    downloads: Option<u64>,
    #[serde(rename = "recent_downloads")]
    recent_downloads: Option<u64>,
    #[serde(rename = "updated_at")]
    updated_at: Option<DateTime<Utc>>,
    #[serde(rename = "created_at")]
    created_at: Option<DateTime<Utc>>,
    #[serde(rename = "repository")]
    repository: Option<String>,
    #[serde(rename = "homepage")]
    homepage: Option<String>,
    #[serde(rename = "documentation")]
    documentation: Option<String>,
    #[serde(rename = "keywords")]
    keywords: Option<Vec<String>>,
    #[serde(rename = "categories")]
    categories: Option<Vec<String>>,
    #[serde(rename = "badges")]
    badges: Vec<serde_json::Value>,
    #[serde(rename = "links")]
    links: serde_json::Value,
    #[serde(rename = "exact_match")]
    exact_match: bool,
    #[serde(rename = "num_versions")]
    num_versions: usize,
    #[serde(rename = "yanked")]
    yanked: bool,
    #[serde(rename = "versions")]
    versions: Option<serde_json::Value>,
}

impl DocsClient {
    /// Search for Rust crates using the crates.io API
    pub async fn search_crates(&self, request: SearchRequest) -> Result<SearchResponse> {
        let limit = request.limit();
        let query = urlencoding::encode(&request.query);

        // Build the search URL for crates.io API
        let path = format!("/api/v1/crates?q={query}&per_page={limit}");

        trace!(
            query = %request.query,
            limit = limit,
            path = %path,
            "Searching crates via crates.io API"
        );

        // Make the API request using crates.io base URL
        let crates_io_url = "https://crates.io";
        let full_url = format!("{crates_io_url}{path}");

        let response = self
            .inner_client()
            .get(&full_url)
            .send()
            .await
            .context("Failed to send search request to crates.io")?;

        let response = handle_http_response(response, "crates.io search").await?;
        let crates_io_response: CratesIoSearchResponse = parse_json_response(response, "crates.io search results").await?;

        debug!(
            query = %request.query,
            total_results = crates_io_response.meta.total,
            returned_results = crates_io_response.crates.len(),
            "Search completed successfully"
        );

        // Transform the crates.io response to our internal format
        let search_results = transform_search_results(crates_io_response.crates)?;

        let response = SearchResponse::with_total(search_results, crates_io_response.meta.total);

        Ok(response)
    }
}

/// Transform crates.io API response to our internal search result format
fn transform_search_results(crates: Vec<CratesIoCrate>) -> Result<Vec<CrateSearchResult>> {
    let mut results = Vec::with_capacity(crates.len());

    for crate_data in crates {
        let result = transform_crate_data(crate_data)?;
        results.push(result);
    }

    Ok(results)
}

/// Transform a single crate from crates.io format to our internal format
fn transform_crate_data(crate_data: CratesIoCrate) -> Result<CrateSearchResult> {
    // Parse repository URL if present
    let repository = match crate_data.repository {
        Some(ref repo_str) if !repo_str.is_empty() => match Url::parse(repo_str) {
            Ok(url) => Some(url),
            Err(e) => {
                trace!(
                    crate_name = %crate_data.name,
                    repository = %repo_str,
                    error = %e,
                    "Failed to parse repository URL, skipping"
                );
                None
            }
        },
        _ => None,
    };

    // Parse homepage URL if present
    let homepage = match crate_data.homepage {
        Some(ref home_str) if !home_str.is_empty() => match Url::parse(home_str) {
            Ok(url) => Some(url),
            Err(e) => {
                trace!(
                    crate_name = %crate_data.name,
                    homepage = %home_str,
                    error = %e,
                    "Failed to parse homepage URL, skipping"
                );
                None
            }
        },
        _ => None,
    };

    // Generate docs.rs URL with version
    let docs_url = Some(build_docs_url(&crate_data.name, DEFAULT_VERSION)?);

    Ok(CrateSearchResult {
        name: crate_data.name,
        version: crate_data.newest_version,
        description: crate_data.description,
        docs_url,
        download_count: crate_data.downloads,
        last_updated: crate_data.updated_at,
        repository,
        homepage,
        keywords: crate_data.keywords.unwrap_or_default(),
        categories: crate_data.categories.unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;
    use std::time::Duration;

    fn create_test_crate(name: &str, version: &str) -> CratesIoCrate {
        CratesIoCrate {
            id: name.to_string(),
            name: name.to_string(),
            newest_version: version.to_string(),
            max_version: version.to_string(),
            max_stable_version: Some(version.to_string()),
            default_version: version.to_string(),
            description: None,
            downloads: None,
            recent_downloads: None,
            updated_at: Some(Utc::now()),
            created_at: Some(Utc::now()),
            repository: None,
            homepage: None,
            documentation: None,
            keywords: None,
            categories: None,
            badges: vec![],
            links: json!({}),
            exact_match: false,
            num_versions: 1,
            versions: None,
            yanked: false,
        }
    }

    #[test]
    fn test_transform_crate_data_complete() {
        let mut crate_data = create_test_crate("tokio", "1.0.0");
        crate_data.description = Some("An event-driven, non-blocking I/O platform".to_string());
        crate_data.downloads = Some(50000000);
        crate_data.repository = Some("https://github.com/tokio-rs/tokio".to_string());
        crate_data.homepage = Some("https://tokio.rs".to_string());
        crate_data.keywords = Some(vec!["async".to_string(), "io".to_string()]);
        crate_data.categories = Some(vec!["asynchronous".to_string()]);

        let result = transform_crate_data(crate_data.clone()).unwrap();

        assert_eq!(result.name, "tokio");
        assert_eq!(result.version, "1.0.0");
        assert_eq!(
            result.description,
            Some("An event-driven, non-blocking I/O platform".to_string())
        );
        assert_eq!(result.download_count, Some(50000000));
        assert!(result.docs_url.is_some());
        assert_eq!(
            result.docs_url.unwrap().as_str(),
            "https://docs.rs/tokio/latest/tokio/"
        );
        assert!(result.repository.is_some());
        assert!(result.homepage.is_some());
        assert_eq!(result.keywords, vec!["async", "io"]);
        assert_eq!(result.categories, vec!["asynchronous"]);
    }

    #[test]
    fn test_transform_crate_data_minimal() {
        let crate_data = create_test_crate("minimal", "0.1.0");

        let result = transform_crate_data(crate_data).unwrap();

        assert_eq!(result.name, "minimal");
        assert_eq!(result.version, "0.1.0");
        assert_eq!(result.description, None);
        assert_eq!(result.download_count, None);
        assert!(result.docs_url.is_some());
        assert_eq!(
            result.docs_url.unwrap().as_str(),
            "https://docs.rs/minimal/latest/minimal/"
        );
        assert_eq!(result.repository, None);
        assert_eq!(result.homepage, None);
        assert!(result.keywords.is_empty());
        assert!(result.categories.is_empty());
    }

    #[test]
    fn test_transform_crate_data_invalid_urls() {
        let mut crate_data = create_test_crate("badurls", "0.1.0");
        crate_data.repository = Some("not-a-valid-url".to_string());
        crate_data.homepage = Some("also-not-valid".to_string());

        let result = transform_crate_data(crate_data).unwrap();

        assert_eq!(result.name, "badurls");
        assert_eq!(result.repository, None); // Should be None due to invalid URL
        assert_eq!(result.homepage, None); // Should be None due to invalid URL
        assert!(result.docs_url.is_some()); // docs.rs URL should still work
    }

    #[test]
    fn test_transform_crate_data_empty_urls() {
        let mut crate_data = create_test_crate("emptyurls", "0.1.0");
        crate_data.repository = Some("".to_string());
        crate_data.homepage = Some("".to_string());

        let result = transform_crate_data(crate_data).unwrap();

        assert_eq!(result.name, "emptyurls");
        assert_eq!(result.repository, None); // Should be None due to empty string
        assert_eq!(result.homepage, None); // Should be None due to empty string
    }

    #[test]
    fn test_transform_search_results() {
        let mut crate1 = create_test_crate("crate1", "1.0.0");
        crate1.description = Some("First crate".to_string());
        crate1.downloads = Some(1000);

        let mut crate2 = create_test_crate("crate2", "2.0.0");
        crate2.description = Some("Second crate".to_string());
        crate2.downloads = Some(2000);

        let crates = vec![crate1, crate2];

        let results = transform_search_results(crates).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "crate1");
        assert_eq!(results[1].name, "crate2");
        assert_eq!(results[0].download_count, Some(1000));
        assert_eq!(results[1].download_count, Some(2000));
    }

    #[test]
    fn test_crates_io_response_deserialization() {
        let json = json!({
            "crates": [
                {
                    "id": "serde",
                    "name": "serde",
                    "newest_version": "1.0.136",
                    "max_version": "1.0.136",
                    "max_stable_version": "1.0.136",
                    "default_version": "1.0.136",
                    "description": "A generic serialization/deserialization framework",
                    "downloads": 123456789,
                    "recent_downloads": 1000000,
                    "updated_at": "2023-01-01T00:00:00Z",
                    "created_at": "2020-01-01T00:00:00Z",
                    "repository": "https://github.com/serde-rs/serde",
                    "homepage": "https://serde.rs",
                    "documentation": "https://docs.rs/serde",
                    "keywords": ["serde", "serialization"],
                    "categories": ["encoding"],
                    "badges": [],
                    "links": {},
                    "exact_match": true,
                    "num_versions": 100,
                    "yanked": false,
                    "versions": null
                }
            ],
            "meta": {
                "total": 1
            }
        });

        let response: CratesIoSearchResponse = serde_json::from_value(json).unwrap();

        assert_eq!(response.crates.len(), 1);
        assert_eq!(response.meta.total, 1);

        let crate_data = &response.crates[0];
        assert_eq!(crate_data.name, "serde");
        assert_eq!(crate_data.newest_version, "1.0.136");
        assert_eq!(
            crate_data.description,
            Some("A generic serialization/deserialization framework".to_string())
        );
        assert_eq!(crate_data.downloads, Some(123456789));
        assert_eq!(
            crate_data.repository,
            Some("https://github.com/serde-rs/serde".to_string())
        );
        assert_eq!(crate_data.homepage, Some("https://serde.rs".to_string()));
        assert_eq!(
            crate_data.keywords,
            Some(vec!["serde".to_string(), "serialization".to_string()])
        );
        assert_eq!(crate_data.categories, Some(vec!["encoding".to_string()]));
    }

    #[test]
    fn test_search_cache_key() {
        let request1 = SearchRequest::new("tokio");
        let request2 = SearchRequest::with_limit("tokio", 10);
        let request3 = SearchRequest::with_limit("tokio", 20);
        let request4 = SearchRequest::new("serde");

        let key1 = SearchCacheKey::new(&request1);
        let key2 = SearchCacheKey::new(&request2);
        let key3 = SearchCacheKey::new(&request3);
        let key4 = SearchCacheKey::new(&request4);

        // Same query and limit should be equal
        assert_eq!(key1, key2);

        // Different limit should not be equal
        assert_ne!(key1, key3);

        // Different query should not be equal
        assert_ne!(key1, key4);

        // Keys should be hashable
        use std::collections::HashMap;
        let mut map = HashMap::new();
        map.insert(key1.clone(), "value1");
        map.insert(key3, "value3");
        map.insert(key4, "value4");

        assert_eq!(map.get(&key1), Some(&"value1"));
        assert_eq!(map.get(&key2), Some(&"value1")); // key2 equals key1
    }

    #[tokio::test]
    async fn test_search_service_creation() {
        let client = DocsClient::new().unwrap();
        let service = SearchService::new(client, 100, Duration::from_secs(300));

        let stats = service.cache_stats().await;
        assert_eq!(stats.size, 0);
        assert_eq!(stats.capacity, 100);
        assert_eq!(stats.requests, 0);
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
    }

    #[tokio::test]
    async fn test_search_service_cache_operations() {
        let client = DocsClient::new().unwrap();
        let service = SearchService::new(client, 10, Duration::from_secs(60));

        // Test cache clear
        let cleared = service.clear_cache().await;
        assert_eq!(cleared, 0); // Empty cache

        // Test cleanup expired
        let expired = service.cleanup_expired().await;
        assert_eq!(expired, 0); // No expired entries

        let stats = service.cache_stats().await;
        assert_eq!(stats.size, 0);
    }

    // Integration tests with mock server
    #[cfg(feature = "integration-tests")]
    mod integration_tests {
        use super::*;
        use mockito::Server;
        use rustacean_docs_core::models::search::SearchRequest;

        #[tokio::test]
        async fn test_search_crates_success() {
            let mut server = Server::new_async().await;

            let mock_response = json!({
                "crates": [
                    {
                        "name": "tokio",
                        "newest_version": "1.0.0",
                        "description": "An event-driven, non-blocking I/O platform",
                        "downloads": 50000000,
                        "updated_at": "2023-01-01T00:00:00Z",
                        "repository": "https://github.com/tokio-rs/tokio",
                        "homepage": "https://tokio.rs",
                        "keywords": ["async", "io"],
                        "categories": ["asynchronous"]
                    }
                ],
                "meta": {
                    "total": 1
                }
            });

            let mock = server
                .mock("GET", "/api/v1/crates?q=tokio&per_page=10")
                .with_status(200)
                .with_header("content-type", "application/json")
                .with_body(mock_response.to_string())
                .create_async()
                .await;

            // Create a client with test configuration
            let client = DocsClient::test_client().unwrap();

            // Override the search method to use mock server
            let request = SearchRequest::new("tokio");
            let query = urlencoding::encode(&request.query);
            let limit = request.limit();
            let path = format!("/api/v1/crates?q={query}&per_page={limit}");
            let full_url = format!("{}{path}", server.url());

            let response = client.inner_client().get(&full_url).send().await.unwrap();

            let crates_io_response: CratesIoSearchResponse = response.json().await.unwrap();
            let search_results = transform_search_results(crates_io_response.crates).unwrap();
            let final_response =
                SearchResponse::with_total(search_results, crates_io_response.meta.total);

            assert_eq!(final_response.results.len(), 1);
            assert_eq!(final_response.total, Some(1));

            let result = &final_response.results[0];
            assert_eq!(result.name, "tokio");
            assert_eq!(result.version, "1.0.0");
            assert_eq!(
                result.description,
                Some("An event-driven, non-blocking I/O platform".to_string())
            );

            mock.assert_async().await;
        }

        #[tokio::test]
        async fn test_search_crates_empty_results() {
            let mut server = Server::new_async().await;

            let mock_response = json!({
                "crates": [],
                "meta": {
                    "total": 0
                }
            });

            let mock = server
                .mock("GET", "/api/v1/crates?q=nonexistent&per_page=10")
                .with_status(200)
                .with_header("content-type", "application/json")
                .with_body(mock_response.to_string())
                .create_async()
                .await;

            let client = DocsClient::test_client().unwrap();

            // Test with empty results
            let request = SearchRequest::new("nonexistent");
            let query = urlencoding::encode(&request.query);
            let limit = request.limit();
            let path = format!("/api/v1/crates?q={query}&per_page={limit}");
            let full_url = format!("{}{path}", server.url());

            let response = client.inner_client().get(&full_url).send().await.unwrap();

            let crates_io_response: CratesIoSearchResponse = response.json().await.unwrap();
            let search_results = transform_search_results(crates_io_response.crates).unwrap();
            let final_response =
                SearchResponse::with_total(search_results, crates_io_response.meta.total);

            assert_eq!(final_response.results.len(), 0);
            assert_eq!(final_response.total, Some(0));

            mock.assert_async().await;
        }
    }
}
