//! Integration tests for MCP protocol compliance
//!
//! These tests verify that our tools correctly implement the MCP protocol
//! specifications and provide the expected interfaces.

use integration_tests::common::create_tiered_cache;
use rustacean_docs_cache::{Cache, TieredCache};
use rustacean_docs_client::DocsClient;
use rustacean_docs_mcp_server::tools::{search::SearchTool, ToolHandler};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;

type ServerCache = TieredCache<String, Value>;

/// Test that tools implement the required ToolHandler trait
#[tokio::test]
async fn test_tool_handler_trait_implementation() {
    let search_tool = SearchTool::new();

    // Verify that SearchTool implements ToolHandler
    let tool_handler: &dyn ToolHandler = &search_tool;

    // Test that all required methods are available
    let description = tool_handler.description();
    assert!(!description.is_empty(), "Tool should have a description");

    let schema = tool_handler.parameters_schema();
    assert!(schema.is_object(), "Tool should provide a parameter schema");
}

#[tokio::test]
async fn test_tool_parameter_schema_compliance() {
    let search_tool = SearchTool::new();
    let schema = search_tool.parameters_schema();

    // Test JSON Schema compliance
    assert!(schema.is_object(), "Schema should be a JSON object");

    // Required fields for JSON Schema
    assert!(
        schema.get("type").is_some(),
        "Schema should have 'type' field"
    );
    assert_eq!(schema["type"], "object", "Schema type should be 'object'");

    assert!(
        schema.get("properties").is_some(),
        "Schema should have 'properties' field"
    );
    assert!(
        schema["properties"].is_object(),
        "Properties should be an object"
    );

    // MCP-specific requirements
    let properties = &schema["properties"];

    // Test query parameter
    assert!(
        properties.get("query").is_some(),
        "Should have 'query' parameter"
    );
    let query_prop = &properties["query"];
    assert_eq!(query_prop["type"], "string", "Query should be string type");
    assert!(
        query_prop.get("description").is_some(),
        "Query should have description"
    );

    // Test limit parameter
    assert!(
        properties.get("limit").is_some(),
        "Should have 'limit' parameter"
    );
    let limit_prop = &properties["limit"];
    assert_eq!(
        limit_prop["type"], "integer",
        "Limit should be integer type"
    );
    assert!(
        limit_prop.get("description").is_some(),
        "Limit should have description"
    );

    // Test required fields
    assert!(
        schema.get("required").is_some(),
        "Schema should specify required fields"
    );
    let required = schema["required"].as_array().unwrap();
    assert!(
        required.contains(&json!("query")),
        "Query should be required"
    );

    // Test constraints
    assert!(
        query_prop.get("minLength").is_some(),
        "Query should have minLength constraint"
    );
    assert!(
        limit_prop.get("minimum").is_some(),
        "Limit should have minimum constraint"
    );
    assert!(
        limit_prop.get("maximum").is_some(),
        "Limit should have maximum constraint"
    );
}

#[tokio::test]
async fn test_tool_description_quality() {
    let search_tool = SearchTool::new();
    let description = search_tool.description();

    // Basic description requirements
    assert!(!description.is_empty(), "Description should not be empty");
    assert!(
        description.len() > 20,
        "Description should be meaningful (>20 chars)"
    );
    assert!(
        description.len() < 500,
        "Description should be concise (<500 chars)"
    );

    // Content requirements for search tool
    let desc_lower = description.to_lowercase();
    assert!(desc_lower.contains("search"), "Should mention 'search'");
    assert!(desc_lower.contains("crate"), "Should mention 'crate'");
    assert!(desc_lower.contains("rust"), "Should mention 'rust'");

    // Should not contain implementation details
    assert!(
        !desc_lower.contains("http"),
        "Should not expose HTTP details"
    );
    assert!(
        !desc_lower.contains("cache"),
        "Should not expose cache details"
    );
    assert!(
        !desc_lower.contains("reqwest"),
        "Should not expose library details"
    );
}

#[tokio::test]
async fn test_tool_parameter_validation() {
    let (client, cache) = create_test_environment().await;
    let client = Arc::new(client);
    let search_tool = SearchTool::new();

    // Test valid parameters
    let valid_params = json!({
        "query": "tokio",
        "limit": 20
    });

    // This may fail due to network, but parameter validation should pass
    let result = search_tool.execute(valid_params, &client, &cache).await;
    match result {
        Ok(_) => {
            // Success is good
        }
        Err(e) => {
            let error_msg = e.to_string();
            // Should not be a parameter validation error
            assert!(
                !error_msg.contains("invalid input"),
                "Valid parameters should not cause validation error: {}",
                error_msg
            );
        }
    }

    // Test parameter validation errors
    let invalid_cases = vec![
        (json!({}), "missing query"),
        (json!({"query": ""}), "empty query"),
        (json!({"query": "test", "limit": 0}), "zero limit"),
        (json!({"query": "test", "limit": 200}), "excessive limit"),
        (json!({"query": 123}), "non-string query"),
        (
            json!({"query": "test", "limit": "not-a-number"}),
            "non-numeric limit",
        ),
    ];

    for (params, description) in invalid_cases {
        let result = search_tool.execute(params, &client, &cache).await;
        assert!(
            result.is_err(),
            "Should reject invalid parameters: {}",
            description
        );
    }
}

#[tokio::test]
async fn test_tool_response_format() {
    let (client, cache) = create_test_environment().await;
    let client = Arc::new(client);
    let search_tool = SearchTool::new();

    // Pre-populate cache with a known response
    let cache_key = "search_crate:format-test:10";
    let mock_response = json!({
        "results": [
            {
                "name": "format-test",
                "version": "1.0.0",
                "description": "Test crate for format validation",
                "docs_url": "https://docs.rs/format-test",
                "download_count": 1000,
                "last_updated": "2023-01-01T00:00:00Z",
                "repository": "https://github.com/test/format-test",
                "homepage": "https://format-test.rs",
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
        let _ = cache_guard
            .insert(cache_key.to_string(), mock_response.clone())
            .await;
    }

    let params = json!({
        "query": "format-test",
        "limit": 10
    });

    let result = search_tool.execute(params, &client, &cache).await;
    assert!(result.is_ok(), "Cached response should succeed");

    let response = result.unwrap();

    // Test response structure compliance
    assert!(response.is_object(), "Response should be JSON object");

    // Required top-level fields
    assert!(
        response.get("results").is_some(),
        "Response should have 'results' field"
    );
    assert!(
        response.get("total").is_some(),
        "Response should have 'total' field"
    );
    assert!(
        response.get("query").is_some(),
        "Response should have 'query' field"
    );

    // Test results array structure
    let results = response["results"].as_array().unwrap();
    assert_eq!(results.len(), 1, "Should have one result");

    let result_item = &results[0];
    let required_fields = vec!["name", "version", "description", "docs_url"];
    for field in required_fields {
        assert!(
            result_item.get(field).is_some(),
            "Result should have '{}' field",
            field
        );
    }

    // Test data types
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

    // Test numeric fields
    if result_item["download_count"].is_number() {
        let _download_count = result_item["download_count"].as_u64().unwrap();
        // u64 is always non-negative
    }

    // Test total field
    assert!(response["total"].is_number(), "Total should be numeric");
    let total = response["total"].as_u64().unwrap();
    assert!(
        total >= results.len() as u64,
        "Total should be >= results length"
    );
}

#[tokio::test]
async fn test_tool_error_handling() {
    let (client, cache) = create_test_environment().await;
    let client = Arc::new(client);
    let search_tool = SearchTool::new();

    // Test malformed JSON
    let malformed_params = json!({
        "not_query": "test"
    });

    let result = search_tool.execute(malformed_params, &client, &cache).await;
    assert!(result.is_err(), "Malformed parameters should cause error");

    // Verify error can be converted to string (for MCP protocol)
    let error = result.unwrap_err();
    let error_string = error.to_string();
    assert!(
        !error_string.is_empty(),
        "Error should have meaningful message"
    );
    assert!(
        error_string.len() < 1000,
        "Error message should be reasonably sized"
    );
}

#[tokio::test]
async fn test_tool_concurrency_safety() {
    let (client, cache) = create_test_environment().await;
    let client = Arc::new(client);

    // Pre-populate cache to ensure consistent responses
    let cache_key = "search_crate:concurrency:10";
    let mock_response = json!({
        "results": [{
            "name": "concurrency-test",
            "version": "1.0.0",
            "description": "Test for concurrency",
            "docs_url": "https://docs.rs/concurrency-test",
            "download_count": 1000,
            "last_updated": "2023-01-01T00:00:00Z",
            "repository": "https://github.com/test/concurrency-test",
            "homepage": "https://concurrency-test.rs",
            "keywords": ["test"],
            "categories": ["development-tools"]
        }],
        "total": 1,
        "query": {"returned": 1, "requested": 1}
    });

    {
        let cache_guard = cache.write().await;
        let _ = cache_guard
            .insert(cache_key.to_string(), mock_response.clone())
            .await;
    }

    // Test concurrent tool execution
    let mut handles = vec![];

    for i in 0..10 {
        let client_clone = client.clone();
        let cache_clone = cache.clone();
        let tool = SearchTool::new();

        let handle = tokio::spawn(async move {
            let params = json!({
                "query": "concurrency",
                "limit": 10
            });

            let result = tool.execute(params, &client_clone, &cache_clone).await;
            (i, result)
        });

        handles.push(handle);
    }

    // Collect all results
    let mut successful_results = 0;
    for handle in handles {
        let (task_id, result) = handle.await.expect("Task should complete");

        match result {
            Ok(response) => {
                successful_results += 1;

                // Verify response consistency
                assert_eq!(
                    response, mock_response,
                    "Task {} should get consistent response",
                    task_id
                );
            }
            Err(e) => {
                panic!("Task {} failed: {}", task_id, e);
            }
        }
    }

    assert_eq!(
        successful_results, 10,
        "All concurrent executions should succeed"
    );
}

#[tokio::test]
async fn test_tool_state_independence() {
    let (client, cache) = create_test_environment().await;
    let client = Arc::new(client);

    // Create multiple tool instances
    let tool1 = SearchTool::new();
    let tool2 = SearchTool::new();

    // Verify tools are independent (no shared state)
    let params1 = json!({"query": "tool1", "limit": 10});
    let params2 = json!({"query": "tool2", "limit": 20});

    // Execute on both tools concurrently
    let (result1, result2) = tokio::join!(
        tool1.execute(params1, &client, &cache),
        tool2.execute(params2, &client, &cache)
    );

    // Both should handle their parameters independently
    // (Results may fail due to network, but parameter handling should work)
    if let Err(e1) = result1 {
        assert!(
            !e1.to_string().contains("tool2"),
            "Tool1 should not be affected by tool2"
        );
    }

    if let Err(e2) = result2 {
        assert!(
            !e2.to_string().contains("tool1"),
            "Tool2 should not be affected by tool1"
        );
    }
}

#[tokio::test]
async fn test_tool_schema_additionalproperties() {
    let search_tool = SearchTool::new();
    let schema = search_tool.parameters_schema();

    // Check additionalProperties setting
    assert!(
        schema.get("additionalProperties").is_some(),
        "Schema should specify additionalProperties policy"
    );
    assert_eq!(
        schema["additionalProperties"], false,
        "Should not allow additional properties for strict validation"
    );
}

#[tokio::test]
async fn test_tool_json_schema_validation() {
    let search_tool = SearchTool::new();
    let schema = search_tool.parameters_schema();

    // Test that the schema itself is valid JSON
    let schema_string =
        serde_json::to_string(&schema).expect("Schema should be serializable to JSON");

    let _parsed_back: Value =
        serde_json::from_str(&schema_string).expect("Schema should be valid JSON when serialized");

    // Test schema structure integrity
    assert!(schema.is_object(), "Schema root should be object");

    let properties = schema["properties"].as_object().unwrap();

    for (prop_name, prop_def) in properties {
        assert!(
            prop_def.is_object(),
            "Property '{}' definition should be object",
            prop_name
        );
        assert!(
            prop_def["type"].is_string(),
            "Property '{}' should have type",
            prop_name
        );

        if let Some(desc) = prop_def.get("description") {
            assert!(
                desc.is_string(),
                "Property '{}' description should be string",
                prop_name
            );
            let desc_str = desc.as_str().unwrap();
            assert!(
                !desc_str.is_empty(),
                "Property '{}' description should not be empty",
                prop_name
            );
        }
    }
}

async fn create_test_environment() -> (DocsClient, Arc<RwLock<ServerCache>>) {
    let client = DocsClient::new().expect("Failed to create DocsClient");
    let temp_dir =
        std::env::temp_dir().join(format!("rustacean_docs_test_{}", rand::random::<u64>()));
    let cache = Arc::new(RwLock::new(create_tiered_cache(100, &temp_dir)));
    (client, cache)
}
