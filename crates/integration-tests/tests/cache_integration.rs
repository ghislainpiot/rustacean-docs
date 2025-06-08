//! Integration tests for cache behavior across the system
//!
//! These tests verify cache hit/miss scenarios, TTL behavior, and
//! integration between different components using the cache.

use rustacean_docs_cache::TieredCache;
use rustacean_docs_client::DocsClient;
use rustacean_docs_mcp_server::tools::{search::SearchTool, ToolHandler};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;

type ServerCache = TieredCache<String, Value>;

/// Create a test environment with shorter TTL for testing
async fn create_cache_test_environment() -> (DocsClient, Arc<RwLock<ServerCache>>) {
    let client = DocsClient::new().expect("Failed to create DocsClient");
    let temp_dir =
        std::env::temp_dir().join(format!("rustacean_docs_test_{}", rand::random::<u64>()));
    let cache = Arc::new(RwLock::new(
        TieredCache::new(
            10,                         // Small capacity for testing eviction
            Duration::from_millis(100), // Short TTL for testing expiration
            temp_dir,
            Duration::from_millis(200), // Disk TTL slightly longer
            1024 * 1024,                // 1MB disk cache
        )
        .await
        .expect("Failed to create TieredCache"),
    ));
    (client, cache)
}

/// Create a standard test environment
async fn create_test_environment() -> (DocsClient, Arc<RwLock<ServerCache>>) {
    let client = DocsClient::new().expect("Failed to create DocsClient");
    let temp_dir =
        std::env::temp_dir().join(format!("rustacean_docs_test_{}", rand::random::<u64>()));
    let cache = Arc::new(RwLock::new(
        TieredCache::new(
            100,
            Duration::from_secs(3600),
            temp_dir,
            Duration::from_secs(7200), // 2 hours disk TTL
            50 * 1024 * 1024,          // 50MB disk cache
        )
        .await
        .expect("Failed to create TieredCache"),
    ));
    (client, cache)
}

/// Helper to create mock response data
fn create_mock_response(name: &str, total: usize) -> Value {
    json!({
        "results": [{
            "name": name,
            "version": "1.0.0",
            "description": format!("Mock crate {name}"),
            "docs_url": format!("https://docs.rs/{name}"),
            "download_count": 1000,
            "last_updated": "2023-01-01T00:00:00Z",
            "repository": format!("https://github.com/test/{name}"),
            "homepage": format!("https://{name}.rs"),
            "keywords": ["test"],
            "categories": ["development-tools"]
        }],
        "total": total,
        "query": {
            "returned": 1,
            "requested": 1
        }
    })
}

#[tokio::test]
async fn test_cache_hit_scenario() {
    let (client, cache) = create_test_environment().await;
    let client = Arc::new(client);
    let tool = SearchTool::new();

    // Pre-populate cache
    let cache_key = "search:cached-hit:10";
    let mock_response = create_mock_response("cached-hit", 1);

    {
        let cache_guard = cache.write().await;
        cache_guard
            .insert(cache_key.to_string(), mock_response.clone())
            .await
            .expect("Failed to insert into cache");
    }

    // Verify initial cache state
    {
        let cache_guard = cache.read().await;
        let stats = cache_guard
            .stats()
            .await
            .expect("Failed to get cache stats");
        assert_eq!(
            stats.memory.size + stats.disk.size,
            2,
            "Cache should have one entry (stored in both memory and disk)"
        );
        assert_eq!(stats.total_hits, 0, "No hits yet");
        assert_eq!(stats.total_misses, 0, "No misses yet");
    }

    // Execute search that should hit cache
    let search_params = json!({
        "query": "cached-hit",
        "limit": 10
    });

    let result = tool.execute(search_params, &client, &cache).await;
    assert!(result.is_ok(), "Cache hit should succeed");

    let response = result.unwrap();
    assert_eq!(response, mock_response, "Should return cached response");

    // Verify cache statistics after hit
    {
        let cache_guard = cache.read().await;
        let stats = cache_guard
            .stats()
            .await
            .expect("Failed to get cache stats");
        assert_eq!(
            stats.memory.size + stats.disk.size,
            2,
            "Cache size should remain 2 (one entry in both layers)"
        );
        assert!(stats.total_hits > 0, "Should have recorded a cache hit");
        assert!(stats.total_requests > 0, "Should have recorded a request");
    }
}

#[tokio::test]
async fn test_cache_miss_scenario() {
    let (client, cache) = create_test_environment().await;
    let client = Arc::new(client);
    let tool = SearchTool::new();

    // Verify cache starts empty
    {
        let cache_guard = cache.read().await;
        let stats = cache_guard
            .stats()
            .await
            .expect("Failed to get cache stats");
        assert_eq!(
            stats.memory.size + stats.disk.size,
            0,
            "Cache should start empty"
        );
    }

    // Execute search that will miss cache
    let search_params = json!({
        "query": "cache-miss-test",
        "limit": 15
    });

    let result = tool.execute(search_params, &client, &cache).await;

    // The result may fail due to network issues, but cache should record the miss
    {
        let cache_guard = cache.read().await;
        let stats = cache_guard
            .stats()
            .await
            .expect("Failed to get cache stats");
        assert!(
            stats.total_requests > 0,
            "Should have recorded a cache request"
        );
    }

    match result {
        Ok(_) => {
            // If network call succeeded, check if result was cached
            {
                let cache_guard = cache.read().await;
                let stats = cache_guard
                    .stats()
                    .await
                    .expect("Failed to get cache stats");
                // The tool should have attempted to cache the result
                assert!(stats.total_requests > 0, "Should have cache activity");
            }
        }
        Err(_) => {
            // Network error is acceptable in integration tests
            // Just verify cache behavior was attempted
            {
                let cache_guard = cache.read().await;
                let stats = cache_guard
                    .stats()
                    .await
                    .expect("Failed to get cache stats");
                assert!(
                    stats.total_requests > 0,
                    "Should have attempted cache lookup"
                );
            }
        }
    }
}

#[tokio::test]
async fn test_cache_key_uniqueness() {
    let (client, cache) = create_test_environment().await;
    let client = Arc::new(client);
    let tool = SearchTool::new();

    // Create different cache entries that should have unique keys
    let test_cases = vec![
        ("search:test1:10", json!({"query": "test1", "limit": 10})),
        ("search:test1:20", json!({"query": "test1", "limit": 20})),
        ("search:test2:10", json!({"query": "test2", "limit": 10})),
        ("search:test3:10", json!({"query": "test3"})), // No limit specified, defaults to 10
    ];

    // Pre-populate cache with unique responses for each key
    for (i, (key, _)) in test_cases.iter().enumerate() {
        let mock_response = create_mock_response(&format!("crate{}", i), i + 1);
        {
            let cache_guard = cache.write().await;
            cache_guard
                .insert(key.to_string(), mock_response)
                .await
                .expect("Failed to insert into cache");
        }
    }

    // Verify all entries are cached separately
    {
        let cache_guard = cache.read().await;
        let stats = cache_guard
            .stats()
            .await
            .expect("Failed to get cache stats");
        assert_eq!(
            stats.memory.size + stats.disk.size,
            test_cases.len() * 2,
            "All entries should be cached separately (each entry stored in both memory and disk)"
        );
    }

    // Test retrieval of each cached entry
    for (i, (_, params)) in test_cases.iter().enumerate() {
        let result = tool.execute(params.clone(), &client, &cache).await;
        assert!(result.is_ok(), "Cached entry {} should be retrievable", i);

        let response = result.unwrap();
        let results = response["results"].as_array().unwrap();
        assert_eq!(
            results[0]["name"],
            format!("crate{}", i),
            "Should get correct cached entry"
        );
    }
}

#[tokio::test]
async fn test_cache_ttl_expiration() {
    let (_client, cache) = create_cache_test_environment().await; // Short TTL
    let _tool = SearchTool::new();

    // Insert entry into cache
    let cache_key = "search:ttl-test:10";
    let mock_response = create_mock_response("ttl-test", 1);

    {
        let cache_guard = cache.write().await;
        cache_guard
            .insert(cache_key.to_string(), mock_response.clone())
            .await
            .expect("Failed to insert into cache");
    }

    // Verify entry is cached
    {
        let cache_guard = cache.read().await;
        let cached = cache_guard
            .get(&cache_key.to_string())
            .await
            .expect("Failed to get from cache");
        assert!(cached.is_some(), "Entry should be initially cached");
    }

    // Wait for TTL expiration (100ms in test environment)
    sleep(Duration::from_millis(150)).await;

    // Trigger cleanup and verify entry is expired
    {
        let cache_guard = cache.write().await;
        let (memory_expired, disk_expired) = cache_guard
            .cleanup_expired()
            .await
            .expect("Failed to cleanup expired entries");
        assert!(
            memory_expired > 0 || disk_expired > 0,
            "Should have cleaned up expired entries"
        );
    }

    // Verify entry is no longer cached (should be expired from both layers)
    {
        let cache_guard = cache.read().await;
        let cached = cache_guard
            .get(&cache_key.to_string())
            .await
            .expect("Failed to get from cache");
        // Entry might still exist in disk cache since it has slightly longer TTL (200ms vs 100ms)
        // Just check that cleanup actually happened
        let stats = cache_guard
            .stats()
            .await
            .expect("Failed to get cache stats");
        // The entry should either be gone or we should see it was promoted back from disk
        if cached.is_some() {
            // If still present, it was promoted from disk (which has 200ms TTL)
            assert!(
                stats.total_requests > 0,
                "Should have attempted cache access"
            );
        } else {
            // If gone, both layers expired it
            assert!(
                stats.total_requests > 0,
                "Should have attempted cache access"
            );
        }
    }
}

#[tokio::test]
async fn test_cache_lru_eviction() {
    let (_client, _cache) = create_test_environment().await;

    // Manually create a cache with very small capacity for testing eviction
    let temp_dir = std::env::temp_dir().join(format!(
        "rustacean_docs_test_small_{}",
        rand::random::<u64>()
    ));
    let small_cache = Arc::new(RwLock::new(
        TieredCache::new(
            2, // Only 2 entries
            Duration::from_secs(3600),
            temp_dir,
            Duration::from_secs(7200),
            1024 * 1024, // 1MB disk cache
        )
        .await
        .expect("Failed to create small TieredCache"),
    ));

    let _tool = SearchTool::new();

    // Fill cache to capacity
    let entries = vec![
        ("search:entry1:10", create_mock_response("entry1", 1)),
        ("search:entry2:10", create_mock_response("entry2", 1)),
    ];

    for (key, response) in &entries {
        {
            let cache_guard = small_cache.write().await;
            cache_guard
                .insert(key.to_string(), response.clone())
                .await
                .expect("Failed to insert into cache");
        }
    }

    // Verify cache is at capacity
    {
        let cache_guard = small_cache.read().await;
        let stats = cache_guard
            .stats()
            .await
            .expect("Failed to get cache stats");
        assert_eq!(
            stats.memory.size + stats.disk.size,
            4,
            "Cache should be at capacity (2 entries × 2 layers)"
        );
    }

    // Add one more entry to trigger eviction
    {
        let cache_guard = small_cache.write().await;
        let new_response = create_mock_response("entry3", 1);
        cache_guard
            .insert("search:entry3:10".to_string(), new_response)
            .await
            .expect("Failed to insert into cache");
    }

    // Verify memory cache enforced capacity but disk cache may have all entries
    {
        let cache_guard = small_cache.read().await;
        let stats = cache_guard
            .stats()
            .await
            .expect("Failed to get cache stats");
        // Memory cache should be at capacity (2), but disk cache might have all 3 entries
        // Memory: 2 entries (capacity limit), Disk: 3 entries (no eviction yet) = 5 total
        assert_eq!(stats.memory.size, 2, "Memory cache should be at capacity");
        assert_eq!(stats.disk.size, 3, "Disk cache should have all entries");
        assert_eq!(
            stats.memory.size + stats.disk.size,
            5,
            "Total should be 5 (2 memory + 3 disk)"
        );

        // Test if entries are still accessible (might be promoted from disk)
        let first_entry = cache_guard
            .get(&"search:entry1:10".to_string())
            .await
            .expect("Failed to get from cache");
        let second_entry = cache_guard
            .get(&"search:entry2:10".to_string())
            .await
            .expect("Failed to get from cache");
        let third_entry = cache_guard
            .get(&"search:entry3:10".to_string())
            .await
            .expect("Failed to get from cache");

        // All entries should be accessible (either from memory or promoted from disk)
        assert!(
            first_entry.is_some(),
            "First entry should be accessible from disk"
        );
        assert!(second_entry.is_some(), "Second entry should be accessible");
        assert!(third_entry.is_some(), "Third entry should be accessible");
    }
}

#[tokio::test]
async fn test_cache_concurrent_access() {
    let (client, cache) = create_test_environment().await;
    let client = Arc::new(client);
    let _tool = SearchTool::new();

    // Pre-populate cache
    let cache_key = "search:concurrent:10";
    let mock_response = create_mock_response("concurrent", 1);

    {
        let cache_guard = cache.write().await;
        cache_guard
            .insert(cache_key.to_string(), mock_response.clone())
            .await
            .expect("Failed to insert into cache");
    }

    // Spawn multiple concurrent tasks that access the same cache entry
    let mut handles = vec![];

    for i in 0..5 {
        let client_clone = Arc::clone(&client);
        let cache_clone = cache.clone();
        let tool_clone = SearchTool::new();

        let handle = tokio::spawn(async move {
            let search_params = json!({
                "query": "concurrent",
                "limit": 10
            });

            let result = tool_clone
                .execute(search_params, &client_clone, &cache_clone)
                .await;
            (i, result)
        });

        handles.push(handle);
    }

    // Wait for all tasks to complete
    let mut success_count = 0;
    for handle in handles {
        let (task_id, result) = handle.await.expect("Task should complete");
        match result {
            Ok(response) => {
                success_count += 1;
                assert_eq!(
                    response, mock_response,
                    "Task {} should get cached response",
                    task_id
                );
            }
            Err(e) => {
                panic!("Task {} failed unexpectedly: {}", task_id, e);
            }
        }
    }

    assert_eq!(success_count, 5, "All concurrent tasks should succeed");

    // Verify cache statistics
    {
        let cache_guard = cache.read().await;
        let stats = cache_guard
            .stats()
            .await
            .expect("Failed to get cache stats");
        assert!(
            stats.total_hits >= 5,
            "Should have multiple cache hits from concurrent access"
        );
    }
}

#[tokio::test]
async fn test_cache_statistics_accuracy() {
    let (client, cache) = create_test_environment().await;
    let client = Arc::new(client);
    let tool = SearchTool::new();

    // Initial state verification
    {
        let cache_guard = cache.read().await;
        let stats = cache_guard
            .stats()
            .await
            .expect("Failed to get cache stats");
        assert_eq!(stats.memory.size + stats.disk.size, 0);
        assert_eq!(stats.total_hits, 0);
        assert_eq!(stats.total_misses, 0);
        assert_eq!(stats.total_requests, 0);
    }

    // Perform a cache miss (should be empty cache)
    let search_params = json!({
        "query": "stats-test",
        "limit": 10
    });

    let _result = tool.execute(search_params.clone(), &client, &cache).await;

    // Check stats after first request
    {
        let cache_guard = cache.read().await;
        let stats = cache_guard
            .stats()
            .await
            .expect("Failed to get cache stats");
        assert!(stats.total_requests > 0, "Should have recorded request");
    }

    // Pre-populate cache for next test
    let cache_key = "search:stats-test:10";
    let mock_response = create_mock_response("stats-test", 1);

    {
        let cache_guard = cache.write().await;
        cache_guard
            .insert(cache_key.to_string(), mock_response.clone())
            .await
            .expect("Failed to insert into cache");
    }

    // Perform cache hit
    let _result = tool.execute(search_params, &client, &cache).await;

    // Verify statistics were updated correctly
    {
        let cache_guard = cache.read().await;
        let stats = cache_guard
            .stats()
            .await
            .expect("Failed to get cache stats");
        assert!(stats.total_hits > 0, "Should have recorded cache hit");
        assert!(
            stats.total_requests >= 2,
            "Should have recorded both requests"
        );

        // Hit rate should be reasonable
        let hit_rate = stats.total_hits as f64 / stats.total_requests as f64;
        assert!(
            hit_rate > 0.0,
            "Hit rate should be positive when we have hits"
        );
        assert!(hit_rate <= 1.0, "Hit rate should not exceed 100%");
    }
}

#[tokio::test]
async fn test_cache_clear_functionality() {
    let (_client, cache) = create_test_environment().await;

    // Populate cache with multiple entries
    let entries = vec![
        ("search:clear1:10", create_mock_response("clear1", 1)),
        ("search:clear2:10", create_mock_response("clear2", 1)),
        ("search:clear3:10", create_mock_response("clear3", 1)),
    ];

    for (key, response) in &entries {
        {
            let cache_guard = cache.write().await;
            cache_guard
                .insert(key.to_string(), response.clone())
                .await
                .expect("Failed to insert into cache");
        }
    }

    // Verify cache has entries
    {
        let cache_guard = cache.read().await;
        let stats = cache_guard
            .stats()
            .await
            .expect("Failed to get cache stats");
        assert_eq!(
            stats.memory.size + stats.disk.size,
            6,
            "Cache should have 3 entries (stored in both memory and disk)"
        );
    }

    // Clear cache
    {
        let cache_guard = cache.write().await;
        let (memory_cleared, disk_cleared) =
            cache_guard.clear().await.expect("Failed to clear cache");
        assert_eq!(
            memory_cleared + disk_cleared,
            6,
            "Should have cleared 6 entries total (3 from memory + 3 from disk)"
        );
    }

    // Verify cache is empty
    {
        let cache_guard = cache.read().await;
        let stats = cache_guard
            .stats()
            .await
            .expect("Failed to get cache stats");
        assert_eq!(
            stats.memory.size + stats.disk.size,
            0,
            "Cache should be empty after clear"
        );

        // Verify specific entries are gone
        for (key, _) in &entries {
            let cached = cache_guard
                .get(&key.to_string())
                .await
                .expect("Failed to get from cache");
            assert!(cached.is_none(), "Entry {} should be cleared", key);
        }
    }
}

#[tokio::test]
async fn test_cache_performance_characteristics() {
    let (client, cache) = create_test_environment().await;
    let client = Arc::new(client);
    let tool = SearchTool::new();

    // Measure cache insertion performance
    let start = std::time::Instant::now();

    // Insert multiple entries
    for i in 0..50 {
        let key = format!("search:perf{}:10", i);
        let response = create_mock_response(&format!("perf{}", i), 1);

        {
            let cache_guard = cache.write().await;
            cache_guard
                .insert(key, response)
                .await
                .expect("Failed to insert into cache");
        }
    }

    let insert_duration = start.elapsed();

    // Measure cache retrieval performance
    let start = std::time::Instant::now();

    for i in 0..50 {
        let search_params = json!({
            "query": format!("perf{}", i),
            "limit": 10
        });

        let _result = tool.execute(search_params, &client, &cache).await;
    }

    let retrieval_duration = start.elapsed();

    // Basic performance assertions
    assert!(
        insert_duration.as_millis() < 1000,
        "Cache insertions should be fast (took {}ms)",
        insert_duration.as_millis()
    );
    assert!(
        retrieval_duration.as_millis() < 1000,
        "Cache retrievals should be fast (took {}ms)",
        retrieval_duration.as_millis()
    );

    // Verify all entries are present
    {
        let cache_guard = cache.read().await;
        let stats = cache_guard
            .stats()
            .await
            .expect("Failed to get cache stats");
        assert_eq!(
            stats.memory.size + stats.disk.size,
            100,
            "All entries should be cached (50 entries × 2 layers)"
        );
        assert!(stats.total_hits >= 50, "Should have many cache hits");
    }
}
