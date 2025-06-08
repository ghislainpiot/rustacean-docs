//! End-to-end workflow tests for the complete system
//!
//! These tests verify the full search workflow with cache integration,
//! simulating real-world usage patterns and edge cases.

use rustacean_docs_cache::TieredCache;
use rustacean_docs_client::DocsClient;
use rustacean_docs_mcp_server::tools::{search::SearchTool, ToolHandler};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::Instant;

type ServerCache = TieredCache<String, Value>;

/// Create test environment with realistic cache settings
async fn create_realistic_test_environment() -> (DocsClient, Arc<RwLock<ServerCache>>) {
    let client = DocsClient::new().expect("Failed to create DocsClient");
    let temp_dir =
        std::env::temp_dir().join(format!("rustacean_docs_test_{}", rand::random::<u64>()));
    // Use realistic cache settings: 1000 entries, 1 hour TTL
    let cache = Arc::new(RwLock::new(
        TieredCache::new(
            1000,
            Duration::from_secs(3600),
            temp_dir,
            Duration::from_secs(7200), // 2 hours disk TTL
            100 * 1024 * 1024,         // 100MB disk cache
        )
        .await
        .expect("Failed to create TieredCache"),
    ));
    (client, cache)
}

/// Helper to create comprehensive mock responses
fn create_comprehensive_mock_response(crate_name: &str, result_count: usize) -> Value {
    let results: Vec<Value> = (0..result_count)
        .map(|i| {
            let name = if i == 0 {
                crate_name.to_string()
            } else {
                format!("{}-{}", crate_name, i)
            };
            json!({
                "name": name.clone(),
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
        "total": result_count * 10, // Simulate more results available
        "query": {
            "returned": result_count,
            "requested": result_count
        }
    })
}

#[tokio::test]
async fn test_complete_search_workflow_with_cache() {
    let (client, cache) = create_realistic_test_environment().await;
    let client = Arc::new(client);
    let tool = SearchTool::new();

    // Phase 1: Cold cache scenario (cache miss)
    println!("Phase 1: Testing cache miss scenario");

    let search_params = json!({
        "query": "workflow-test",
        "limit": 10
    });

    // First request should be a cache miss (may fail due to network)
    let start_time = Instant::now();
    let first_result = tool.execute(search_params.clone(), &client, &cache).await;
    let first_duration = start_time.elapsed();

    // Check cache state after first request
    let cache_stats_after_first = {
        let cache_guard = cache.read().await;
        cache_guard
            .stats()
            .await
            .expect("Failed to get cache stats")
    };

    match first_result {
        Ok(_) => {
            println!("First request succeeded (network available)");
            // If successful, should have cached the result
            assert!(
                cache_stats_after_first.total_requests > 0,
                "Should have recorded cache request"
            );
        }
        Err(_) => {
            println!("First request failed (expected if no network)");
            // Even on failure, cache should have been checked
            assert!(
                cache_stats_after_first.total_requests > 0,
                "Should have attempted cache lookup"
            );
        }
    }

    // Phase 2: Pre-populate cache for consistent testing
    println!("Phase 2: Pre-populating cache for consistent testing");

    let cache_key = "search:workflow-test:10";
    let mock_response = create_comprehensive_mock_response("workflow-test", 5);

    {
        let cache_guard = cache.write().await;
        cache_guard
            .insert(cache_key.to_string(), mock_response.clone())
            .await
            .expect("Failed to insert into cache");
    }

    // Phase 3: Cache hit scenario
    println!("Phase 3: Testing cache hit scenario");

    let start_time = Instant::now();
    let second_result = tool.execute(search_params.clone(), &client, &cache).await;
    let second_duration = start_time.elapsed();

    assert!(second_result.is_ok(), "Cached request should succeed");
    let response = second_result.unwrap();
    assert_eq!(response, mock_response, "Should return cached response");

    // Cache hit should be faster than potential network request
    println!(
        "Cache hit duration: {:?}, First request duration: {:?}",
        second_duration, first_duration
    );

    // Verify cache statistics
    let cache_stats_after_hit = {
        let cache_guard = cache.read().await;
        cache_guard
            .stats()
            .await
            .expect("Failed to get cache stats")
    };

    assert!(
        cache_stats_after_hit.total_hits > cache_stats_after_first.total_hits,
        "Should have recorded cache hit"
    );

    // Phase 4: Multiple requests to test cache consistency
    println!("Phase 4: Testing multiple cache hits");

    for i in 0..5 {
        let result = tool.execute(search_params.clone(), &client, &cache).await;
        assert!(result.is_ok(), "Cache hit {} should succeed", i);

        let response = result.unwrap();
        assert_eq!(
            response, mock_response,
            "All cache hits should return same response"
        );
    }

    // Final cache statistics
    let final_cache_stats = {
        let cache_guard = cache.read().await;
        cache_guard
            .stats()
            .await
            .expect("Failed to get cache stats")
    };

    assert!(
        final_cache_stats.total_hits >= 6,
        "Should have multiple cache hits"
    );
    println!("Final cache stats: {:?}", final_cache_stats);
}

#[tokio::test]
async fn test_multiple_queries_workflow() {
    let (client, cache) = create_realistic_test_environment().await;
    let client = Arc::new(client);
    let tool = SearchTool::new();

    // Test different search queries with various parameters
    let test_queries = vec![
        ("tokio", 10),
        ("serde", 20),
        ("reqwest", 5),
        ("async-trait", 15),
        ("clap", 25),
    ];

    // Pre-populate cache with responses for all queries
    for (query, limit) in &test_queries {
        let cache_key = format!("search:{}:{}", query, limit);
        let mock_response = create_comprehensive_mock_response(query, *limit);

        {
            let cache_guard = cache.write().await;
            cache_guard
                .insert(cache_key, mock_response)
                .await
                .expect("Failed to insert into cache");
        }
    }

    // Execute all queries and verify responses
    for (query, limit) in &test_queries {
        let search_params = json!({
            "query": query,
            "limit": limit
        });

        let result = tool.execute(search_params, &client, &cache).await;
        assert!(result.is_ok(), "Query '{}' should succeed", query);

        let response = result.unwrap();
        assert!(response["results"].is_array(), "Should have results array");

        let results = response["results"].as_array().unwrap();
        assert_eq!(
            results.len(),
            *limit,
            "Should return requested number of results"
        );

        // Verify first result matches the query
        if !results.is_empty() {
            let first_result = &results[0];
            assert_eq!(
                first_result["name"], *query,
                "First result should match query"
            );
        }
    }

    // Verify cache contains all queries
    let final_stats = {
        let cache_guard = cache.read().await;
        cache_guard
            .stats()
            .await
            .expect("Failed to get cache stats")
    };

    assert_eq!(
        final_stats.memory.size + final_stats.disk.size,
        test_queries.len() * 2,
        "Cache should contain all unique queries (stored in both memory and disk)"
    );
    assert!(
        final_stats.total_hits >= test_queries.len() as u64,
        "Should have cache hits for all queries"
    );
}

#[tokio::test]
async fn test_workflow_with_parameter_variations() {
    let (client, cache) = create_realistic_test_environment().await;
    let client = Arc::new(client);
    let tool = SearchTool::new();

    // Test same query with different limits (should create different cache entries)
    let base_query = "param-test";
    let limits = vec![5, 10, 20, 50];

    // Pre-populate cache for all variations
    for limit in &limits {
        let cache_key = format!("search:{}:{}", base_query, limit);
        let mock_response = create_comprehensive_mock_response(base_query, *limit);

        {
            let cache_guard = cache.write().await;
            cache_guard
                .insert(cache_key, mock_response)
                .await
                .expect("Failed to insert into cache");
        }
    }

    // Test each parameter variation
    for limit in &limits {
        let search_params = json!({
            "query": base_query,
            "limit": limit
        });

        let result = tool.execute(search_params, &client, &cache).await;
        assert!(result.is_ok(), "Query with limit {} should succeed", limit);

        let response = result.unwrap();
        let results = response["results"].as_array().unwrap();
        assert_eq!(results.len(), *limit, "Should return {} results", limit);
    }

    // Also test default limit behavior
    let search_params_no_limit = json!({
        "query": "default-limit-test"
    });

    // Pre-populate cache for default limit (10)
    let default_cache_key = "search:default-limit-test:10";
    let default_response = create_comprehensive_mock_response("default-limit-test", 10);

    {
        let cache_guard = cache.write().await;
        cache_guard
            .insert(default_cache_key.to_string(), default_response.clone())
            .await
            .expect("Failed to insert into cache");
    }

    let result = tool.execute(search_params_no_limit, &client, &cache).await;
    assert!(result.is_ok(), "Query without limit should succeed");

    let response = result.unwrap();
    assert_eq!(response, default_response, "Should use default limit of 10");
}

#[tokio::test]
async fn test_workflow_error_scenarios() {
    let (client, cache) = create_realistic_test_environment().await;
    let client = Arc::new(client);
    let tool = SearchTool::new();

    // Test various error conditions in workflow context
    let error_cases = vec![
        (json!({"query": ""}), "empty query"),
        (json!({"query": "  "}), "whitespace query"),
        (json!({"query": "test", "limit": 0}), "zero limit"),
        (json!({"query": "test", "limit": 200}), "excessive limit"),
        (json!({"limit": 10}), "missing query"),
        (json!({"query": 123}), "non-string query"),
        (
            json!({"query": "test", "limit": "invalid"}),
            "non-numeric limit",
        ),
    ];

    for (params, description) in error_cases {
        let result = tool.execute(params, &client, &cache).await;
        assert!(result.is_err(), "Should fail for: {}", description);

        // Verify error doesn't corrupt cache
        let cache_stats = {
            let cache_guard = cache.read().await;
            cache_guard
                .stats()
                .await
                .expect("Failed to get cache stats")
        };

        // Cache should remain functional
        assert!(
            cache_stats.memory.capacity > 0,
            "Cache should remain functional after error"
        );
    }

    // Verify cache still works after errors
    let cache_key = "search:post-error-test:10";
    let mock_response = create_comprehensive_mock_response("post-error-test", 3);

    {
        let cache_guard = cache.write().await;
        cache_guard
            .insert(cache_key.to_string(), mock_response.clone())
            .await
            .expect("Failed to insert into cache");
    }

    let valid_params = json!({
        "query": "post-error-test",
        "limit": 10
    });

    let result = tool.execute(valid_params, &client, &cache).await;
    assert!(result.is_ok(), "Valid request should work after errors");
    assert_eq!(
        result.unwrap(),
        mock_response,
        "Should return cached response"
    );
}

#[tokio::test]
async fn test_workflow_performance_characteristics() {
    let (client, cache) = create_realistic_test_environment().await;
    let client = Arc::new(client);
    let tool = SearchTool::new();

    // Pre-populate cache with multiple entries
    let query_count = 20;
    for i in 0..query_count {
        let cache_key = format!("search:perf-test-{}:10", i);
        let mock_response = create_comprehensive_mock_response(&format!("perf-test-{}", i), 10);

        {
            let cache_guard = cache.write().await;
            cache_guard
                .insert(cache_key, mock_response)
                .await
                .expect("Failed to insert into cache");
        }
    }

    // Measure performance of cache hits
    let start_time = Instant::now();

    for i in 0..query_count {
        let search_params = json!({
            "query": format!("perf-test-{}", i),
            "limit": 10
        });

        let result = tool.execute(search_params, &client, &cache).await;
        assert!(
            result.is_ok(),
            "Performance test query {} should succeed",
            i
        );
    }

    let total_duration = start_time.elapsed();
    let avg_duration = total_duration / query_count;

    println!(
        "Total time for {} cache hits: {:?}",
        query_count, total_duration
    );
    println!("Average time per cache hit: {:?}", avg_duration);

    // Performance assertions
    assert!(
        avg_duration.as_millis() < 10,
        "Average cache hit should be under 10ms, got {}ms",
        avg_duration.as_millis()
    );
    assert!(
        total_duration.as_millis() < 200,
        "Total time for {} requests should be under 200ms, got {}ms",
        query_count,
        total_duration.as_millis()
    );

    // Verify cache statistics
    let final_stats = {
        let cache_guard = cache.read().await;
        cache_guard
            .stats()
            .await
            .expect("Failed to get cache stats")
    };

    assert_eq!(
        final_stats.memory.size + final_stats.disk.size,
        query_count as usize * 2,
        "All entries should be cached (stored in both memory and disk)"
    );
    assert!(
        final_stats.total_hits >= query_count as u64,
        "Should have many cache hits"
    );

    let hit_rate = final_stats.total_hits as f64 / final_stats.total_requests as f64;
    assert!(
        hit_rate > 0.8,
        "Hit rate should be high (>80%), got {:.2}",
        hit_rate
    );
}

#[tokio::test]
async fn test_workflow_cache_capacity_management() {
    let client = DocsClient::new().expect("Failed to create DocsClient");
    // Create cache with very small capacity for testing eviction
    let temp_dir = std::env::temp_dir().join(format!(
        "rustacean_docs_test_small_{}",
        rand::random::<u64>()
    ));
    let small_cache = Arc::new(RwLock::new(
        TieredCache::new(
            5, // Only 5 entries
            Duration::from_secs(3600),
            temp_dir,
            Duration::from_secs(7200),
            5 * 1024 * 1024, // 5MB disk cache
        )
        .await
        .expect("Failed to create TieredCache"),
    ));
    let client = Arc::new(client);
    let tool = SearchTool::new();

    // Fill cache beyond capacity
    for i in 0..8 {
        let search_params = json!({
            "query": format!("capacity-test-{}", i),
            "limit": 10
        });

        // Pre-populate cache
        let cache_key = format!("search:capacity-test-{}:10", i);
        let mock_response = create_comprehensive_mock_response(&format!("capacity-test-{}", i), 5);

        {
            let cache_guard = small_cache.write().await;
            cache_guard
                .insert(cache_key, mock_response)
                .await
                .expect("Failed to insert into cache");
        }

        // Execute search to ensure tool can handle it
        let result = tool.execute(search_params, &client, &small_cache).await;
        assert!(
            result.is_ok(),
            "Search {} should succeed even with cache eviction",
            i
        );
    }

    // Verify cache maintained capacity limit
    let final_stats = {
        let cache_guard = small_cache.read().await;
        cache_guard
            .stats()
            .await
            .expect("Failed to get cache stats")
    };

    // Memory cache should respect capacity limit (5), but disk cache may have more entries
    // Memory: at most 5 entries, Disk: could have up to 8 entries (no count-based eviction)
    assert!(
        final_stats.memory.size <= 5,
        "Memory cache should not exceed capacity of 5, got {}",
        final_stats.memory.size
    );
    assert!(
        final_stats.disk.size <= 8,
        "Disk cache should have at most 8 entries, got {}",
        final_stats.disk.size
    );
    // Total will be more than 5 because disk cache has different eviction strategy
    assert!(
        final_stats.memory.size + final_stats.disk.size >= 5,
        "Should have at least 5 total entries across both layers"
    );
    // When cache is at capacity, older entries should have been evicted

    // Verify LRU behavior - recent entries should still be accessible
    let recent_params = json!({
        "query": "capacity-test-7",
        "limit": 10
    });

    let result = tool.execute(recent_params, &client, &small_cache).await;
    assert!(result.is_ok(), "Most recent entry should still be cached");
}

#[tokio::test]
async fn test_full_system_integration() {
    let (client, cache) = create_realistic_test_environment().await;
    let client = Arc::new(client);
    let tool = SearchTool::new();

    // Simulate real usage pattern: popular crates search
    let popular_crates = vec![
        "serde",
        "tokio",
        "reqwest",
        "clap",
        "anyhow",
        "thiserror",
        "uuid",
        "chrono",
        "regex",
        "log",
    ];

    println!(
        "Starting full system integration test with {} popular crates",
        popular_crates.len()
    );

    // Phase 1: Initial searches (cache misses expected)
    let mut initial_results = Vec::new();
    for crate_name in &popular_crates {
        let search_params = json!({
            "query": crate_name,
            "limit": 15
        });

        let result = tool.execute(search_params, &client, &cache).await;
        initial_results.push((crate_name, result));
    }

    // Phase 2: Pre-populate cache for consistent testing
    for crate_name in &popular_crates {
        let cache_key = format!("search:{}:15", crate_name);
        let mock_response = create_comprehensive_mock_response(crate_name, 15);

        {
            let cache_guard = cache.write().await;
            cache_guard
                .insert(cache_key, mock_response)
                .await
                .expect("Failed to insert into cache");
        }
    }

    // Phase 3: Repeated searches (should all be cache hits)
    for crate_name in &popular_crates {
        let search_params = json!({
            "query": crate_name,
            "limit": 15
        });

        let result = tool.execute(search_params, &client, &cache).await;
        assert!(
            result.is_ok(),
            "Cached search for '{}' should succeed",
            crate_name
        );

        let response = result.unwrap();
        assert!(response["results"].is_array(), "Should have results");
        assert_eq!(
            response["results"].as_array().unwrap().len(),
            15,
            "Should return 15 results for '{}'",
            crate_name
        );
    }

    // Phase 4: Mixed parameter searches
    for (i, crate_name) in popular_crates.iter().enumerate() {
        let limit = 5 + (i * 2); // Varying limits: 5, 7, 9, 11, etc.
        let search_params = json!({
            "query": crate_name,
            "limit": limit
        });

        // Pre-populate for this variation
        let cache_key = format!("search:{}:{}", crate_name, limit);
        let mock_response = create_comprehensive_mock_response(crate_name, limit);

        {
            let cache_guard = cache.write().await;
            cache_guard
                .insert(cache_key, mock_response.clone())
                .await
                .expect("Failed to insert into cache");
        }

        let result = tool.execute(search_params, &client, &cache).await;
        assert!(result.is_ok(), "Mixed parameter search should succeed");
        assert_eq!(
            result.unwrap(),
            mock_response,
            "Should get correct cached response"
        );
    }

    // Final verification
    let final_stats = {
        let cache_guard = cache.read().await;
        cache_guard
            .stats()
            .await
            .expect("Failed to get cache stats")
    };

    println!("Final system stats: {:?}", final_stats);
    assert!(
        final_stats.memory.size + final_stats.disk.size >= popular_crates.len(),
        "Should have cached many entries"
    );
    assert!(
        final_stats.total_hits >= (popular_crates.len() * 2) as u64,
        "Should have many cache hits from repeated searches"
    );

    let hit_rate = final_stats.total_hits as f64 / final_stats.total_requests as f64;
    println!("Final hit rate: {:.2}%", hit_rate * 100.0);
    assert!(hit_rate > 0.5, "Hit rate should be reasonable (>50%)");
}
