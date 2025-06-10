//! Documentation workflow integration tests
//!
//! Tests the complete documentation retrieval and caching workflow,
//! including crate docs, item docs, and version-specific requests.

use integration_tests::common::create_tiered_cache;
use rustacean_docs_cache::{Cache, TieredCache};
use rustacean_docs_client::DocsClient;
use rustacean_docs_mcp_server::tools::{
    crate_docs::CrateDocsTool, item_docs::ItemDocsTool, ToolHandler,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Instant;

type ServerCache = TieredCache<String, Value>;

/// Create test environment optimized for documentation caching
async fn create_docs_test_environment() -> (DocsClient, Arc<RwLock<ServerCache>>) {
    let client = DocsClient::new().expect("Failed to create DocsClient");
    let temp_dir =
        std::env::temp_dir().join(format!("rustacean_docs_test_{}", rand::random::<u64>()));
    // Documentation cache: 500 entries
    let cache = Arc::new(RwLock::new(create_tiered_cache(500, &temp_dir)));
    (client, cache)
}

/// Create mock crate documentation response
fn create_mock_crate_docs(crate_name: &str, version: &str) -> Value {
    json!({
        "name": crate_name,
        "version": version,
        "description": format!("Mock documentation for {} v{}", crate_name, version),
        "summary": {
            "total_items": 42,
            "modules": 5,
            "structs": 12,
            "traits": 8,
            "functions": 15,
            "macros": 2
        },
        "categories": {
            "core_types": [
                {
                    "name": "Config",
                    "path": "struct.Config.html",
                    "type": "struct",
                    "visibility": "pub",
                    "description": "Main configuration struct"
                },
                {
                    "name": "Error",
                    "path": "enum.Error.html",
                    "type": "enum",
                    "visibility": "pub",
                    "description": "Error types"
                }
            ],
            "traits": [
                {
                    "name": "Processor",
                    "path": "trait.Processor.html",
                    "type": "trait",
                    "visibility": "pub",
                    "description": "Processing trait"
                }
            ],
            "modules": [
                {
                    "name": "utils",
                    "path": "utils/index.html",
                    "type": "module",
                    "visibility": "pub",
                    "description": "Utility functions"
                }
            ]
        },
        "examples": [
            {
                "title": "Basic Usage",
                "code": format!("use {}::Config;\nlet config = Config::new();", crate_name)
            }
        ],
        "docs_url": format!("https://docs.rs/{}/{}", crate_name, version)
    })
}

/// Create mock item documentation response
fn create_mock_item_docs(crate_name: &str, item_path: &str) -> Value {
    let item_name = item_path.split('/').last().unwrap_or(item_path);
    json!({
        "crate_name": crate_name,
        "item_path": item_path,
        "signature": format!("pub struct {}", item_name),
        "description": format!("Documentation for {} in {}", item_name, crate_name),
        "examples": [
            {
                "title": "Example usage",
                "code": format!("let instance = {}::new();", item_name)
            }
        ],
        "fields": [
            {
                "name": "field1",
                "type": "String",
                "description": "First field"
            },
            {
                "name": "field2",
                "type": "Option<u32>",
                "description": "Optional second field"
            }
        ],
        "methods": [
            {
                "name": "new",
                "signature": "pub fn new() -> Self",
                "description": "Creates a new instance"
            }
        ],
        "docs_url": format!("https://docs.rs/{}/latest/{}", crate_name, item_path)
    })
}

#[tokio::test]
async fn test_crate_docs_integration() {
    let (client, cache) = create_docs_test_environment().await;
    let client = Arc::new(client);
    let tool = CrateDocsTool::new();

    println!("Testing crate documentation integration");

    // Test basic crate docs request
    let docs_params = json!({
        "crate_name": "integration-test-crate"
    });

    // Pre-populate cache with mock response
    let cache_key = "crate_docs:integration-test-crate:latest";
    let mock_response = create_mock_crate_docs("integration-test-crate", "1.0.0");

    {
        let cache_guard = cache.write().await;
        cache_guard
            .insert(cache_key.to_string(), mock_response.clone())
            .await
            .expect("Failed to insert into cache");
    }

    // Execute tool and verify response
    let start_time = Instant::now();
    let result = tool.execute(docs_params.clone(), &client, &cache).await;
    let duration = start_time.elapsed();

    assert!(result.is_ok(), "Crate docs request should succeed");
    let response = result.unwrap();
    assert_eq!(response, mock_response, "Should return cached response");

    // Verify it was a cache hit (should be fast)
    assert!(duration.as_millis() < 50, "Cache hit should be fast");

    // Test with explicit version
    let versioned_params = json!({
        "crate_name": "integration-test-crate",
        "version": "1.0.0"
    });

    let versioned_cache_key = "crate_docs:integration-test-crate:1.0.0";
    let versioned_response = create_mock_crate_docs("integration-test-crate", "1.0.0");

    {
        let cache_guard = cache.write().await;
        cache_guard
            .insert(versioned_cache_key.to_string(), versioned_response.clone())
            .await
            .expect("Failed to insert into cache");
    }

    let versioned_result = tool.execute(versioned_params, &client, &cache).await;
    assert!(
        versioned_result.is_ok(),
        "Versioned crate docs should succeed"
    );
    assert_eq!(versioned_result.unwrap(), versioned_response);

    // Verify cache statistics
    let cache_stats = {
        let cache_guard = cache.read().await;
        cache_guard.stats()
    };

    assert!(cache_stats.hits >= 2, "Should have multiple cache hits");
    assert_eq!(
        cache_stats.size, 4,
        "Should have 2 cached entries (stored in both memory and disk)"
    );
}

#[tokio::test]
async fn test_item_docs_integration() {
    let (client, cache) = create_docs_test_environment().await;
    let client = Arc::new(client);
    let tool = ItemDocsTool::new();

    println!("Testing item documentation integration");

    // Test basic item docs request
    let item_params = json!({
        "crate_name": "test-crate",
        "item_path": "struct.TestStruct.html"
    });

    // Pre-populate cache
    let cache_key = "item_docs:test-crate:struct.TestStruct.html:latest";
    let mock_response = create_mock_item_docs("test-crate", "struct.TestStruct.html");

    {
        let cache_guard = cache.write().await;
        cache_guard
            .insert(cache_key.to_string(), mock_response.clone())
            .await
            .expect("Failed to insert into cache");
    }

    // Execute and verify
    let result = tool.execute(item_params, &client, &cache).await;
    assert!(result.is_ok(), "Item docs request should succeed");
    assert_eq!(result.unwrap(), mock_response);

    // Test with different item types
    let test_items = vec![
        ("enum.ErrorType.html", "enum"),
        ("trait.Processor.html", "trait"),
        ("fn.helper_function.html", "function"),
        ("struct.Config.html", "struct"),
    ];

    for (item_path, item_type) in test_items {
        let params = json!({
            "crate_name": "test-crate",
            "item_path": item_path
        });

        let cache_key = format!("item_docs:test-crate:{}:latest", item_path);
        let response = create_mock_item_docs("test-crate", item_path);

        {
            let cache_guard = cache.write().await;
            cache_guard
                .insert(cache_key, response.clone())
                .await
                .expect("Failed to insert into cache");
        }

        let result = tool.execute(params, &client, &cache).await;
        assert!(result.is_ok(), "Item docs for {} should succeed", item_type);
        assert_eq!(result.unwrap(), response);
    }

    // Verify cache contains all items
    let final_stats = {
        let cache_guard = cache.read().await;
        cache_guard.stats()
    };

    assert!(final_stats.size >= 5, "Should have cached multiple items");
}

#[tokio::test]
async fn test_version_specific_docs() {
    let (client, cache) = create_docs_test_environment().await;
    let client = Arc::new(client);
    let crate_tool = CrateDocsTool::new();
    let item_tool = ItemDocsTool::new();

    println!("Testing version-specific documentation requests");

    let crate_name = "version-test-crate";
    let versions = vec!["1.0.0", "1.1.0", "2.0.0", "latest"];

    // Test crate docs with different versions
    for version in &versions {
        let params = json!({
            "crate_name": crate_name,
            "version": version
        });

        let cache_key = format!("crate_docs:{}:{}", crate_name, version);
        let mock_response = create_mock_crate_docs(crate_name, version);

        {
            let cache_guard = cache.write().await;
            cache_guard
                .insert(cache_key, mock_response.clone())
                .await
                .expect("Failed to insert into cache");
        }

        let result = crate_tool.execute(params, &client, &cache).await;
        assert!(
            result.is_ok(),
            "Crate docs for version {} should succeed",
            version
        );

        let response = result.unwrap();
        assert_eq!(response["version"], *version, "Should have correct version");
    }

    // Test item docs with different versions
    let item_path = "struct.VersionedStruct.html";
    for version in &versions {
        let params = json!({
            "crate_name": crate_name,
            "item_path": item_path,
            "version": version
        });

        let cache_key = format!("item_docs:{}:{}:{}", crate_name, item_path, version);
        let mock_response = create_mock_item_docs(crate_name, item_path);

        {
            let cache_guard = cache.write().await;
            cache_guard
                .insert(cache_key, mock_response.clone())
                .await
                .expect("Failed to insert into cache");
        }

        let result = item_tool.execute(params, &client, &cache).await;
        assert!(
            result.is_ok(),
            "Item docs for version {} should succeed",
            version
        );
    }

    // Verify each version creates separate cache entries
    let cache_stats = {
        let cache_guard = cache.read().await;
        cache_guard.stats()
    };

    assert_eq!(
        cache_stats.size,
        versions.len() * 2 * 2, // Both crate and item docs for each version, stored in both memory and disk
        "Should have separate cache entries for each version"
    );
}

#[tokio::test]
async fn test_docs_cache_behavior() {
    let (client, cache) = create_docs_test_environment().await;
    let client = Arc::new(client);
    let crate_tool = CrateDocsTool::new();

    println!("Testing documentation cache behavior");

    let crate_name = "cache-behavior-test";
    let params = json!({
        "crate_name": crate_name
    });

    // First request - cache miss scenario
    let cache_key = format!("crate_docs:{}:latest", crate_name);
    let mock_response = create_mock_crate_docs(crate_name, "1.0.0");

    {
        let cache_guard = cache.write().await;
        cache_guard
            .insert(cache_key.clone(), mock_response.clone())
            .await
            .expect("Failed to insert into cache");
    }

    // Multiple requests should all hit cache
    let request_count = 5;
    for i in 0..request_count {
        let start_time = Instant::now();
        let result = crate_tool.execute(params.clone(), &client, &cache).await;
        let duration = start_time.elapsed();

        assert!(result.is_ok(), "Request {} should succeed", i);
        assert_eq!(
            result.unwrap(),
            mock_response,
            "Should return cached response"
        );
        assert!(duration.as_millis() < 10, "Cache hit should be very fast");
    }

    // Verify cache hit statistics
    let cache_stats = {
        let cache_guard = cache.read().await;
        cache_guard.stats()
    };

    assert!(
        cache_stats.hits >= request_count as u64,
        "Should have multiple cache hits"
    );
    assert_eq!(
        cache_stats.size, 2,
        "Should only have one cached entry (stored in both memory and disk)"
    );

    // Test cache key uniqueness with different parameters
    let different_version_params = json!({
        "crate_name": crate_name,
        "version": "2.0.0"
    });

    let version_cache_key = format!("crate_docs:{}:2.0.0", crate_name);
    let version_response = create_mock_crate_docs(crate_name, "2.0.0");

    {
        let cache_guard = cache.write().await;
        cache_guard
            .insert(version_cache_key, version_response.clone())
            .await
            .expect("Failed to insert into cache");
    }

    let version_result = crate_tool
        .execute(different_version_params, &client, &cache)
        .await;
    assert!(version_result.is_ok(), "Different version should succeed");
    assert_eq!(version_result.unwrap(), version_response);

    // Should now have 2 cache entries
    let final_stats = {
        let cache_guard = cache.read().await;
        cache_guard.stats()
    };

    assert_eq!(
        final_stats.size, 4,
        "Should have 2 different cache entries (stored in both memory and disk)"
    );
}

#[tokio::test]
async fn test_complete_documentation_workflow() {
    let (client, cache) = create_docs_test_environment().await;
    let client = Arc::new(client);
    let crate_tool = CrateDocsTool::new();
    let item_tool = ItemDocsTool::new();

    println!("Testing complete documentation workflow");

    let crate_name = "workflow-test-crate";

    // Step 1: Get crate documentation overview
    let crate_params = json!({
        "crate_name": crate_name
    });

    let crate_cache_key = format!("crate_docs:{}:latest", crate_name);
    let crate_response = create_mock_crate_docs(crate_name, "1.0.0");

    {
        let cache_guard = cache.write().await;
        cache_guard
            .insert(crate_cache_key, crate_response.clone())
            .await
            .expect("Failed to insert into cache");
    }

    let crate_result = crate_tool.execute(crate_params, &client, &cache).await;
    assert!(crate_result.is_ok(), "Crate docs should succeed");

    let crate_docs = crate_result.unwrap();
    assert_eq!(crate_docs["name"], crate_name);
    assert!(
        crate_docs["categories"].is_object(),
        "Should have categories"
    );

    // Step 2: Navigate to specific items from the crate docs
    let core_types = crate_docs["categories"]["core_types"].as_array().unwrap();
    for item in core_types {
        let item_path = item["path"].as_str().unwrap();
        let item_params = json!({
            "crate_name": crate_name,
            "item_path": item_path
        });

        let item_cache_key = format!("item_docs:{}:{}:latest", crate_name, item_path);
        let item_response = create_mock_item_docs(crate_name, item_path);

        {
            let cache_guard = cache.write().await;
            cache_guard
                .insert(item_cache_key, item_response.clone())
                .await
                .expect("Failed to insert into cache");
        }

        let item_result = item_tool.execute(item_params, &client, &cache).await;
        assert!(
            item_result.is_ok(),
            "Item docs should succeed for {}",
            item_path
        );

        let item_docs = item_result.unwrap();
        assert_eq!(item_docs["crate_name"], crate_name);
        assert_eq!(item_docs["item_path"], item_path);
    }

    // Step 3: Test workflow with version constraints
    let versioned_crate_params = json!({
        "crate_name": crate_name,
        "version": "1.0.0"
    });

    let versioned_cache_key = format!("crate_docs:{}:1.0.0", crate_name);
    let versioned_response = create_mock_crate_docs(crate_name, "1.0.0");

    {
        let cache_guard = cache.write().await;
        cache_guard
            .insert(versioned_cache_key, versioned_response.clone())
            .await
            .expect("Failed to insert into cache");
    }

    let versioned_result = crate_tool
        .execute(versioned_crate_params, &client, &cache)
        .await;
    assert!(
        versioned_result.is_ok(),
        "Versioned crate docs should succeed"
    );

    // Step 4: Verify complete workflow cache utilization
    let workflow_stats = {
        let cache_guard = cache.read().await;
        cache_guard.stats()
    };

    println!("Workflow cache stats: {:?}", workflow_stats);
    assert!(
        workflow_stats.size >= 3,
        "Should have cached multiple documentation pieces"
    );
    assert!(
        workflow_stats.hits >= 3,
        "Should have multiple cache hits during workflow"
    );

    let hit_rate =
        workflow_stats.hits as f64 / (workflow_stats.hits + workflow_stats.misses) as f64;
    assert!(
        hit_rate > 0.8,
        "Documentation workflow should have high cache hit rate"
    );
}

#[tokio::test]
async fn test_docs_error_handling_integration() {
    let (client, cache) = create_docs_test_environment().await;
    let client = Arc::new(client);
    let crate_tool = CrateDocsTool::new();
    let item_tool = ItemDocsTool::new();

    println!("Testing documentation error handling");

    // Test invalid parameters for crate docs
    let invalid_crate_cases = vec![
        (json!({}), "missing crate_name"),
        (json!({"crate_name": ""}), "empty crate_name"),
        (
            json!({"crate_name": "test", "version": ""}),
            "empty version",
        ),
        (json!({"crate_name": 123}), "non-string crate_name"),
    ];

    for (params, description) in invalid_crate_cases {
        let result = crate_tool.execute(params, &client, &cache).await;
        assert!(result.is_err(), "Should fail for: {}", description);
    }

    // Test invalid parameters for item docs
    let invalid_item_cases = vec![
        (json!({"crate_name": "test"}), "missing item_path"),
        (json!({"item_path": "test"}), "missing crate_name"),
        (
            json!({"crate_name": "", "item_path": "test"}),
            "empty crate_name",
        ),
        (
            json!({"crate_name": "test", "item_path": ""}),
            "empty item_path",
        ),
    ];

    for (params, description) in invalid_item_cases {
        let result = item_tool.execute(params, &client, &cache).await;
        assert!(result.is_err(), "Should fail for: {}", description);
    }

    // Verify cache remains functional after errors
    let valid_params = json!({
        "crate_name": "error-recovery-test"
    });

    let cache_key = "crate_docs:error-recovery-test:latest";
    let mock_response = create_mock_crate_docs("error-recovery-test", "1.0.0");

    {
        let cache_guard = cache.write().await;
        cache_guard
            .insert(cache_key.to_string(), mock_response.clone())
            .await
            .expect("Failed to insert into cache");
    }

    let result = crate_tool.execute(valid_params, &client, &cache).await;
    assert!(result.is_ok(), "Valid request should work after errors");
    assert_eq!(result.unwrap(), mock_response);
}

#[tokio::test]
async fn test_docs_performance_characteristics() {
    let (client, cache) = create_docs_test_environment().await;
    let client = Arc::new(client);
    let crate_tool = CrateDocsTool::new();

    println!("Testing documentation performance characteristics");

    // Pre-populate cache with many crate docs
    let crate_count = 30;
    for i in 0..crate_count {
        let crate_name = format!("perf-test-crate-{}", i);
        let cache_key = format!("crate_docs:{}:latest", crate_name);
        let mock_response = create_mock_crate_docs(&crate_name, "1.0.0");

        {
            let cache_guard = cache.write().await;
            cache_guard
                .insert(cache_key, mock_response)
                .await
                .expect("Failed to insert into cache");
        }
    }

    // Measure performance of documentation lookups
    let start_time = Instant::now();

    for i in 0..crate_count {
        let crate_name = format!("perf-test-crate-{}", i);
        let params = json!({
            "crate_name": crate_name
        });

        let lookup_start = Instant::now();
        let result = crate_tool.execute(params, &client, &cache).await;
        let lookup_duration = lookup_start.elapsed();

        assert!(result.is_ok(), "Performance test {} should succeed", i);
        assert!(
            lookup_duration.as_millis() < 5,
            "Each lookup should be very fast"
        );
    }

    let total_duration = start_time.elapsed();
    let avg_duration = total_duration / crate_count;

    println!(
        "Total time for {} doc lookups: {:?}",
        crate_count, total_duration
    );
    println!("Average time per lookup: {:?}", avg_duration);

    // Performance assertions for documentation access
    assert!(
        avg_duration.as_millis() < 3,
        "Average documentation lookup should be under 3ms"
    );
    assert!(
        total_duration.as_millis() < 100,
        "Total time should be under 100ms for {} lookups",
        crate_count
    );

    // Verify cache efficiency
    let perf_stats = {
        let cache_guard = cache.read().await;
        cache_guard.stats()
    };

    assert_eq!(perf_stats.size, crate_count as usize * 2);
    assert!(perf_stats.hits >= crate_count as u64);

    let hit_rate = perf_stats.hits as f64 / (perf_stats.hits + perf_stats.misses) as f64;
    assert!(
        hit_rate > 0.95,
        "Documentation cache should have very high hit rate"
    );
}
