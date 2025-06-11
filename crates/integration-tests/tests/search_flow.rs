//! Integration tests for the search flow
//!
//! These tests verify the complete search workflow from MCP tool invocation
//! through client API calls to cache integration.

use chrono::Utc;
use integration_tests::common::create_tiered_cache;
use rustacean_docs_cache::{Cache, TieredCache};
use rustacean_docs_client::DocsClient;
use rustacean_docs_core::models::search::CrateSearchResult;
use rustacean_docs_mcp_server::tools::{search::SearchTool, ToolHandler};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use url::Url;

type ServerCache = TieredCache<String, Value>;

/// Helper function to create test environment
async fn create_test_environment() -> (DocsClient, Arc<RwLock<ServerCache>>) {
    let client = DocsClient::new().expect("Failed to create DocsClient");
    let temp_dir =
        std::env::temp_dir().join(format!("rustacean_docs_test_{}", rand::random::<u64>()));
    let cache = Arc::new(RwLock::new(create_tiered_cache(100, &temp_dir)));
    (client, cache)
}

/// Helper function to create a mock search result
fn _create_mock_search_result(name: &str, version: &str) -> CrateSearchResult {
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

#[tokio::test]
async fn test_search_tool_basic_workflow() {
    let (client, cache) = create_test_environment().await;
    let client = Arc::new(client);
    let tool = SearchTool::new();

    // Test basic search parameters
    let search_params = json!({
        "query": "tokio",
        "limit": 10
    });

    // Verify that the tool accepts the parameters structure
    let result = tool.execute(search_params, &client, &cache).await;

    // Note: This may fail with network error since we're not mocking HTTP calls
    // But it tests the parameter parsing and tool structure
    match result {
        Ok(response) => {
            // If successful, verify response structure
            assert!(response.is_object());
            assert!(response["results"].is_array());
            if response["total"].is_number() {
                assert!(response["total"].as_u64().is_some());
            }
        }
        Err(e) => {
            // Expected if no internet or docs.rs is down
            // Just verify it's a network error, not a parsing error
            let error_msg = e.to_string();
            assert!(
                error_msg.contains("Failed to search")
                    || error_msg.contains("connection")
                    || error_msg.contains("network")
                    || error_msg.contains("timeout"),
                "Unexpected error type: {error_msg}"
            );
        }
    }
}

#[tokio::test]
async fn test_search_tool_input_validation() {
    let (client, cache) = create_test_environment().await;
    let client = Arc::new(client);
    let tool = SearchTool::new();

    // Test empty query
    let empty_query = json!({
        "query": ""
    });
    let result = tool.execute(empty_query, &client, &cache).await;
    assert!(result.is_err(), "Empty query should be rejected");

    // Test invalid limit
    let invalid_limit = json!({
        "query": "tokio",
        "limit": 0
    });
    let result = tool.execute(invalid_limit, &client, &cache).await;
    assert!(result.is_err(), "Zero limit should be rejected");

    // Test too high limit
    let high_limit = json!({
        "query": "tokio",
        "limit": 200
    });
    let result = tool.execute(high_limit, &client, &cache).await;
    assert!(result.is_err(), "Excessive limit should be rejected");

    // Test malformed JSON
    let malformed = json!({
        "not_query": "value"
    });
    let result = tool.execute(malformed, &client, &cache).await;
    assert!(
        result.is_err(),
        "Missing required fields should be rejected"
    );
}

#[tokio::test]
async fn test_search_cache_integration() {
    let (client, cache) = create_test_environment().await;
    let client = Arc::new(client);
    let tool = SearchTool::new();

    // First, verify cache is empty
    {
        let cache_guard = cache.read().await;
        let stats = cache_guard.stats();
        assert_eq!(stats.size, 0, "Cache should start empty");
    }

    // Test cache key generation
    let search_params = json!({
        "query": "test-crate",
        "limit": 20
    });

    // The search will likely fail due to network, but we can test cache structure
    let _result = tool.execute(search_params.clone(), &client, &cache).await;

    // Test that we can manually interact with cache using the same key format
    let expected_cache_key = "search_crate:test-crate:20";
    let test_value = json!({"test": "cached_data"});

    {
        let cache_guard = cache.write().await;
        cache_guard
            .insert(expected_cache_key.to_string(), test_value.clone())
            .await
            .expect("Failed to insert into cache");
    }

    {
        let cache_guard = cache.read().await;
        let cached = cache_guard
            .get(&expected_cache_key.to_string())
            .await
            .expect("Failed to get from cache");
        assert!(
            cached.is_some(),
            "Manually inserted value should be retrievable"
        );
        assert_eq!(cached.unwrap(), test_value);
    }
}

#[tokio::test]
async fn test_search_tool_cache_hit_scenario() {
    let (client, cache) = create_test_environment().await;
    let client = Arc::new(client);
    let tool = SearchTool::new();

    // Pre-populate cache with a mock response
    let cache_key = "search_crate:cached-crate:10";
    let mock_response = json!({
        "results": [
            {
                "name": "cached-crate",
                "version": "1.0.0",
                "description": "A pre-cached crate for testing",
                "docs_url": "https://docs.rs/cached-crate",
                "download_count": 1000,
                "last_updated": "2023-01-01T00:00:00Z",
                "repository": "https://github.com/test/cached-crate",
                "homepage": "https://cached-crate.rs",
                "keywords": ["test"],
                "categories": ["development-tools"]
            }
        ],
        "total": 1,
        "query": {
            "returned": 1,
            "requested": 1
        }
    });

    {
        let cache_guard = cache.write().await;
        cache_guard
            .insert(cache_key.to_string(), mock_response.clone())
            .await
            .expect("Failed to insert into cache");
    }

    // Now search for the cached crate
    let search_params = json!({
        "query": "cached-crate",
        "limit": 10
    });

    let result = tool.execute(search_params, &client, &cache).await;
    assert!(result.is_ok(), "Cached search should succeed");

    let response = result.unwrap();
    assert_eq!(response, mock_response, "Should return cached response");

    // Verify cache statistics
    {
        let cache_guard = cache.read().await;
        let stats = cache_guard.stats();
        assert!(stats.hits > 0, "Should have cache hits");
    }
}

#[tokio::test]
async fn test_search_tool_cache_miss_scenario() {
    let (client, cache) = create_test_environment().await;
    let client = Arc::new(client);
    let tool = SearchTool::new();

    // Verify cache starts empty
    {
        let cache_guard = cache.read().await;
        let stats = cache_guard.stats();
        assert_eq!(stats.size, 0);
        assert_eq!(stats.misses, 0);
    }

    // Try to search for something not in cache
    let search_params = json!({
        "query": "uncached-crate",
        "limit": 5
    });

    let _result = tool.execute(search_params, &client, &cache).await;

    // Verify cache miss was recorded (regardless of whether the network call succeeded)
    {
        let cache_guard = cache.read().await;
        let stats = cache_guard.stats();
        assert!(
            stats.hits + stats.misses > 0,
            "Should have recorded cache requests"
        );
    }
}

#[tokio::test]
async fn test_search_tool_multiple_queries_different_cache_keys() {
    let (client, cache) = create_test_environment().await;
    let client = Arc::new(client);
    let tool = SearchTool::new();

    // Pre-populate cache with different query results
    let test_cases = vec![
        ("search_crate:query1:10", "query1", 10),
        ("search_crate:query1:20", "query1", 20),
        ("search_crate:query2:10", "query2", 10),
    ];

    for (cache_key, _query, _limit) in &test_cases {
        let mock_response = json!({
            "results": [],
            "total": 0,
            "query": {
                "returned": 0,
                "requested": 0
            }
        });

        {
            let cache_guard = cache.write().await;
            cache_guard
                .insert(cache_key.to_string(), mock_response)
                .await
                .expect("Failed to insert into cache");
        }
    }

    // Test that different parameters generate different cache lookups
    for (_, query, limit) in &test_cases {
        let search_params = json!({
            "query": query,
            "limit": limit
        });

        let result = tool.execute(search_params, &client, &cache).await;
        assert!(result.is_ok(), "Cached queries should succeed");
    }

    // Verify all entries are in cache
    {
        let cache_guard = cache.read().await;
        let stats = cache_guard.stats();
        assert_eq!(
            stats.size, 6,
            "Should have 6 total cached entries (3 in memory + 3 in disk)"
        );
    }
}

#[tokio::test]
async fn test_search_tool_json_response_structure() {
    let (client, cache) = create_test_environment().await;
    let client = Arc::new(client);
    let tool = SearchTool::new();

    // Pre-populate cache with a well-structured response
    let cache_key = "search_crate:structured-test:10";
    let expected_response = json!({
        "results": [
            {
                "name": "test-crate",
                "version": "1.0.0",
                "description": "A test crate",
                "docs_url": "https://docs.rs/test-crate",
                "download_count": 12345,
                "last_updated": "2023-01-01T00:00:00Z",
                "repository": "https://github.com/test/test-crate",
                "homepage": "https://test-crate.example",
                "keywords": ["test", "example"],
                "categories": ["development-tools"]
            }
        ],
        "total": 1,
        "query": {
            "returned": 1,
            "requested": 1
        }
    });

    {
        let cache_guard = cache.write().await;
        cache_guard
            .insert(cache_key.to_string(), expected_response.clone())
            .await
            .expect("Failed to insert into cache");
    }

    let search_params = json!({
        "query": "structured-test",
        "limit": 10
    });

    let result = tool.execute(search_params, &client, &cache).await;
    assert!(result.is_ok(), "Structured response test should succeed");

    let response = result.unwrap();

    // Verify response structure matches expected format
    assert!(response["results"].is_array(), "Results should be an array");
    assert!(response["total"].is_number(), "Total should be a number");
    assert!(
        response["query"].is_object(),
        "Query info should be an object"
    );

    let results = response["results"].as_array().unwrap();
    assert_eq!(results.len(), 1, "Should have one result");

    let result_item = &results[0];
    assert!(result_item["name"].is_string(), "Name should be string");
    assert!(
        result_item["version"].is_string(),
        "Version should be string"
    );
    assert!(
        result_item["docs_url"].is_string(),
        "Docs URL should be string"
    );
    assert!(
        result_item["keywords"].is_array(),
        "Keywords should be array"
    );
    assert!(
        result_item["categories"].is_array(),
        "Categories should be array"
    );
}

#[tokio::test]
async fn test_search_tool_schema_compliance() {
    let tool = SearchTool::new();
    let schema = tool.parameters_schema();

    // Verify the schema structure
    assert_eq!(schema["type"], "object", "Schema should be object type");
    assert!(schema["properties"].is_object(), "Should have properties");
    assert!(schema["required"].is_array(), "Should have required fields");

    let properties = &schema["properties"];
    assert!(
        properties["query"].is_object(),
        "Query property should exist"
    );
    assert!(
        properties["limit"].is_object(),
        "Limit property should exist"
    );

    let required = schema["required"].as_array().unwrap();
    assert!(
        required.contains(&json!("query")),
        "Query should be required"
    );

    // Verify property constraints
    let query_prop = &properties["query"];
    assert_eq!(query_prop["type"], "string", "Query should be string type");
    assert_eq!(query_prop["minLength"], 1, "Query should have min length");

    let limit_prop = &properties["limit"];
    assert_eq!(
        limit_prop["type"], "integer",
        "Limit should be integer type"
    );
    assert_eq!(limit_prop["minimum"], 1, "Limit should have minimum value");
    assert_eq!(
        limit_prop["maximum"], 100,
        "Limit should have maximum value"
    );
    assert_eq!(limit_prop["default"], 10, "Limit should have default value");
}
