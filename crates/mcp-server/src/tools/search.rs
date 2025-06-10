use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, trace};

use rustacean_docs_cache::{Cache, TieredCache};
use rustacean_docs_client::DocsClient;
use rustacean_docs_core::{models::search::SearchRequest, Error};

use crate::tools::{ErrorHandler, ParameterValidator, ToolErrorContext, ToolHandler, ToolInput};

// Type alias for our specific cache implementation
type ServerCache = TieredCache<String, Value>;

/// Input parameters for the search_crate tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchToolInput {
    /// Search query - can be exact crate name or descriptive keywords
    pub query: String,
    /// Maximum number of results to return (default: 10, max recommended: 50)
    pub limit: Option<usize>,
}

impl ToolInput for SearchToolInput {
    fn validate(&self) -> Result<(), Error> {
        ParameterValidator::validate_query(&self.query, "search_crate")?;
        ParameterValidator::validate_limit(&self.limit, "search_crate", 100)?;
        Ok(())
    }

    fn cache_key(&self, tool_name: &str) -> String {
        format!("{}:{}:{}", tool_name, self.query, self.limit.unwrap_or(10))
    }
}

impl SearchToolInput {
    /// Convert to internal SearchRequest
    pub fn to_search_request(&self) -> SearchRequest {
        match self.limit {
            Some(limit) => SearchRequest::with_limit(&self.query, limit),
            None => SearchRequest::new(&self.query),
        }
    }
}

/// Search tool that searches for Rust crates by name or keywords
pub struct SearchTool;

impl SearchTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl ToolHandler for SearchTool {
    async fn execute(
        &self,
        params: Value,
        client: &Arc<DocsClient>,
        cache: &Arc<RwLock<ServerCache>>,
    ) -> Result<Value> {
        trace!("Executing search tool with params: {}", params);

        // Parse input parameters
        let input: SearchToolInput = serde_json::from_value(params.clone()).map_err(|e| {
            anyhow::anyhow!(
                "{}: {}",
                ErrorHandler::parameter_parsing_context("search_crate"),
                e
            )
        })?;

        // Validate input parameters
        input.validate().map_err(|e| anyhow::anyhow!("{}", e))?;

        debug!(
            query = %input.query,
            limit = input.limit,
            "Processing search request"
        );

        // Generate cache key
        let cache_key = input.cache_key("search_crate");

        // Try to get from server cache first
        {
            let cache_read = cache.read().await;
            if let Ok(Some(cached_value)) = cache_read.get(&cache_key).await {
                trace!(
                    query = %input.query,
                    limit = input.limit,
                    cache_key = %cache_key,
                    "Search cache hit"
                );
                return Ok(cached_value);
            }
        }

        trace!(
            query = %input.query,
            limit = input.limit,
            cache_key = %cache_key,
            "Search cache miss, fetching from API"
        );

        // Cache miss - fetch directly from client
        let search_request = input.to_search_request();
        let search_response = client
            .search_crates(search_request)
            .await
            .search_context(&input.query)?;

        debug!(
            query = %input.query,
            total_results = search_response.total.unwrap_or(0),
            returned_results = search_response.results.len(),
            "Search completed successfully"
        );

        // Serialize response to JSON for caching
        let json_value = serde_json::to_value(&search_response)?;

        // Store in server cache for future requests
        {
            let cache_write = cache.write().await;
            let _ = cache_write.insert(cache_key.clone(), json_value.clone()).await;
        }

        trace!(
            query = %input.query,
            cache_key = %cache_key,
            "Search result cached"
        );

        Ok(json_value)
    }

    fn description(&self) -> &str {
        "Search for Rust crates by name or keywords using the crates.io API. Returns detailed information including documentation URLs, download counts, and metadata."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query - can be exact crate name or descriptive keywords",
                    "minLength": 1
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results to return (default: 10, max: 100)",
                    "minimum": 1,
                    "maximum": 100,
                    "default": 10
                }
            },
            "required": ["query"],
            "additionalProperties": false
        })
    }
}

impl Default for SearchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_search_tool_input_validation() {
        // Valid input
        let valid_input = SearchToolInput {
            query: "tokio".to_string(),
            limit: Some(20),
        };
        assert!(valid_input.validate().is_ok());

        // Empty query
        let empty_query = SearchToolInput {
            query: "".to_string(),
            limit: None,
        };
        assert!(empty_query.validate().is_err());

        // Whitespace query
        let whitespace_query = SearchToolInput {
            query: "   ".to_string(),
            limit: None,
        };
        assert!(whitespace_query.validate().is_err());

        // Zero limit
        let zero_limit = SearchToolInput {
            query: "serde".to_string(),
            limit: Some(0),
        };
        assert!(zero_limit.validate().is_err());

        // Too high limit
        let high_limit = SearchToolInput {
            query: "serde".to_string(),
            limit: Some(200),
        };
        assert!(high_limit.validate().is_err());

        // Valid without limit
        let no_limit = SearchToolInput {
            query: "reqwest".to_string(),
            limit: None,
        };
        assert!(no_limit.validate().is_ok());
    }

    #[test]
    fn test_search_tool_input_to_search_request() {
        let input_with_limit = SearchToolInput {
            query: "tokio".to_string(),
            limit: Some(25),
        };
        let request = input_with_limit.to_search_request();
        assert_eq!(request.query, "tokio");
        assert_eq!(request.limit(), 25);

        let input_no_limit = SearchToolInput {
            query: "serde".to_string(),
            limit: None,
        };
        let request = input_no_limit.to_search_request();
        assert_eq!(request.query, "serde");
        assert_eq!(request.limit(), 10); // default
    }

    #[test]
    fn test_search_tool_cache_key() {
        let input1 = SearchToolInput {
            query: "tokio".to_string(),
            limit: Some(20),
        };
        let key1 = input1.cache_key("search");
        assert_eq!(key1, "search:tokio:20");

        let input2 = SearchToolInput {
            query: "serde".to_string(),
            limit: None,
        };
        let key2 = input2.cache_key("search");
        assert_eq!(key2, "search:serde:10");

        // Same query, different limit should have different keys
        let input3 = SearchToolInput {
            query: "tokio".to_string(),
            limit: Some(30),
        };
        let key3 = input3.cache_key("search");
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_search_tool_input_serialization() {
        let input = SearchToolInput {
            query: "async-trait".to_string(),
            limit: Some(15),
        };

        // Test serialization
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(json["query"], "async-trait");
        assert_eq!(json["limit"], 15);

        // Test deserialization
        let deserialized: SearchToolInput = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized.query, input.query);
        assert_eq!(deserialized.limit, input.limit);
    }

    #[test]
    fn test_search_tool_input_from_json() {
        // Test with limit
        let json_with_limit = json!({
            "query": "reqwest",
            "limit": 25
        });

        let input: SearchToolInput = serde_json::from_value(json_with_limit).unwrap();
        assert_eq!(input.query, "reqwest");
        assert_eq!(input.limit, Some(25));

        // Test without limit
        let json_no_limit = json!({
            "query": "tracing"
        });

        let input: SearchToolInput = serde_json::from_value(json_no_limit).unwrap();
        assert_eq!(input.query, "tracing");
        assert_eq!(input.limit, None);

        // Test invalid JSON structure
        let invalid_json = json!({
            "not_query": "test"
        });

        let result: Result<SearchToolInput, _> = serde_json::from_value(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_search_tool_description() {
        let tool = SearchTool::new();
        let description = tool.description();
        assert!(!description.is_empty());
        assert!(description.contains("Search"));
        assert!(description.contains("crates"));
    }

    #[test]
    fn test_search_tool_parameters_schema() {
        let tool = SearchTool::new();
        let schema = tool.parameters_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
        assert!(schema["properties"]["limit"].is_object());
        assert_eq!(schema["required"][0], "query");

        // Check query property
        let query_prop = &schema["properties"]["query"];
        assert_eq!(query_prop["type"], "string");
        assert_eq!(query_prop["minLength"], 1);

        // Check limit property
        let limit_prop = &schema["properties"]["limit"];
        assert_eq!(limit_prop["type"], "integer");
        assert_eq!(limit_prop["minimum"], 1);
        assert_eq!(limit_prop["maximum"], 100);
        assert_eq!(limit_prop["default"], 10);
    }

    // Mock tests to ensure the structure is correct
    #[tokio::test]
    async fn test_search_tool_structure() {
        let tool = SearchTool::new();

        // Test that we can create the tool
        assert!(!tool.description().is_empty());

        // Test schema is valid JSON
        let schema = tool.parameters_schema();
        assert!(schema.is_object());

        // Ensure we have proper trait implementations
        let _tool_handler: &dyn ToolHandler = &tool;
    }

    // Integration tests
    #[cfg(feature = "integration-tests")]
    mod integration_tests {
        use super::*;
        use rustacean_docs_cache::TieredCache;
        use rustacean_docs_client::DocsClient;
        use serde_json::json;

        async fn create_test_environment() -> (DocsClient, Arc<RwLock<ServerCache>>) {
            let client = DocsClient::new().unwrap();
            let cache = Arc::new(RwLock::new(TieredCache::new(
                vec![],
                rustacean_docs_cache::WriteStrategy::WriteThrough,
            )));
            (client, cache)
        }

        #[tokio::test]
        async fn test_search_tool_integration_structure() {
            // This test demonstrates the integration test structure
            // In a real integration test, we would configure the client to use a mock server
            let (client, cache) = create_test_environment().await;
            let tool = SearchTool::new();

            // Verify that the tool can be created and has proper structure
            assert!(!tool.description().is_empty());
            assert!(tool.parameters_schema().is_object());

            // Test that we can create all the necessary components
            let _client_arc = Arc::new(client);
            let _cache_ref = &cache;

            // This demonstrates the integration without making actual HTTP calls
            // Real integration tests would require proper HTTP mocking infrastructure
        }

        #[tokio::test]
        async fn test_search_tool_cache_behavior() {
            let (_client, _cache) = create_test_environment().await;

            // Test cache key generation
            let input = SearchToolInput {
                query: "test-crate".to_string(),
                limit: Some(20),
            };

            let cache_key = input.cache_key("search");
            assert_eq!(cache_key, "search:test-crate:20");

            // Test that input can be converted to request
            let request = input.to_search_request();
            assert_eq!(request.query, "test-crate");
            assert_eq!(request.limit(), 20);
        }

        #[tokio::test]
        async fn test_search_tool_invalid_input() {
            let (client, cache) = create_test_environment().await;
            let client = Arc::new(client);
            let tool = SearchTool::new();

            // Test empty query
            let params = json!({
                "query": ""
            });

            let result = tool.execute(params, &client, &cache).await;
            assert!(result.is_err(), "Empty query should fail");

            // Test invalid limit
            let params = json!({
                "query": "test",
                "limit": 0
            });

            let result = tool.execute(params, &client, &cache).await;
            assert!(result.is_err(), "Zero limit should fail");

            // Test too high limit
            let params = json!({
                "query": "test",
                "limit": 200
            });

            let result = tool.execute(params, &client, &cache).await;
            assert!(result.is_err(), "Too high limit should fail");
        }

        #[tokio::test]
        async fn test_search_tool_malformed_input() {
            let (client, cache) = create_test_environment().await;
            let client = Arc::new(client);
            let tool = SearchTool::new();

            // Test missing required field
            let params = json!({
                "limit": 10
            });

            let result = tool.execute(params, &client, &cache).await;
            assert!(result.is_err(), "Missing query field should fail");

            // Test wrong type
            let params = json!({
                "query": 123
            });

            let result = tool.execute(params, &client, &cache).await;
            assert!(result.is_err(), "Wrong query type should fail");
        }
    }
}
