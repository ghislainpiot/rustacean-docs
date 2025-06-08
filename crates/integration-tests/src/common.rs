//! Common utilities for integration tests

use crate::*;
use chrono::Utc;
use url::Url;

type ServerCache = MemoryCache<String, Value>;

/// Create a test environment with standard settings
pub async fn create_test_environment() -> (DocsClient, Arc<RwLock<ServerCache>>) {
    let client = DocsClient::new().expect("Failed to create DocsClient");
    let cache = Arc::new(RwLock::new(MemoryCache::new(
        100,
        Duration::from_secs(3600),
    )));
    (client, cache)
}

/// Create a test environment with short TTL for expiration testing
pub async fn create_short_ttl_environment() -> (DocsClient, Arc<RwLock<ServerCache>>) {
    let client = DocsClient::new().expect("Failed to create DocsClient");
    let cache = Arc::new(RwLock::new(MemoryCache::new(
        10,
        Duration::from_millis(100),
    )));
    (client, cache)
}

/// Helper to create mock search result
pub fn create_mock_search_result(name: &str, version: &str) -> CrateSearchResult {
    CrateSearchResult {
        name: name.to_string(),
        version: version.to_string(),
        description: Some(format!("Description for {name}")),
        docs_url: Some(Url::parse(&format!("https://docs.rs/{name}")).unwrap()),
        download_count: Some(1000000),
        last_updated: Some(Utc::now()),
        repository: Some(Url::parse(&format!("https://github.com/rust-lang/{name}")).unwrap()),
        homepage: Some(Url::parse(&format!("https://{name}.rs")).unwrap()),
        keywords: vec!["test".to_string(), "mock".to_string()],
        categories: vec!["development-tools".to_string()],
    }
}

/// Helper to create comprehensive mock responses
pub fn create_mock_response(crate_name: &str, result_count: usize) -> Value {
    let results: Vec<Value> = (0..result_count)
        .map(|i| {
            let name = if i == 0 {
                crate_name.to_string()
            } else {
                format!("{crate_name}-{i}")
            };

            json!({
                "name": name,
                "version": format!("1.{}.0", i),
                "description": format!("Description for {} variant {}", crate_name, i),
                "docs_url": format!("https://docs.rs/{}", name),
                "download_count": 1000000 - (i * 10000),
                "last_updated": "2023-01-01T00:00:00Z",
                "repository": format!("https://github.com/rust-lang/{}", name),
                "homepage": format!("https://{}.rs", name),
                "keywords": ["async", "network", "io"],
                "categories": ["network-programming", "asynchronous"]
            })
        })
        .collect();

    json!({
        "results": results,
        "total": result_count * 10,
        "query": {
            "returned": result_count,
            "requested": result_count
        }
    })
}
