use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, trace};

use rustacean_docs_cache::MemoryCache;
use rustacean_docs_client::DocsClient;
use rustacean_docs_core::{
    error::ErrorContext,
    models::search::{SearchRequest, SearchResponse},
    Error,
};

use crate::tools::ToolHandler;

// Type alias for our specific cache implementation
type ServerCache = MemoryCache<String, Value>;

/// Input parameters for the search_crate tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchToolInput {
    /// Search query - can be exact crate name or descriptive keywords
    pub query: String,
    /// Maximum number of results to return (default: 10, max recommended: 50)
    pub limit: Option<usize>,
}

impl SearchToolInput {
    /// Validate the input parameters
    pub fn validate(&self) -> Result<(), Error> {
        if self.query.trim().is_empty() {
            return Err(Error::invalid_input(
                "search_crate",
                "query cannot be empty",
            ));
        }

        if let Some(limit) = self.limit {
            if limit == 0 {
                return Err(Error::invalid_input(
                    "search_crate",
                    "limit must be greater than 0",
                ));
            }
            if limit > 100 {
                return Err(Error::invalid_input(
                    "search_crate",
                    "limit cannot exceed 100 for performance reasons",
                ));
            }
        }

        Ok(())
    }

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

    /// Generate cache key for search requests
    fn cache_key(input: &SearchToolInput) -> String {
        format!("search:{}:{}", input.query, input.limit.unwrap_or(10))
    }

    /// Transform SearchResponse to JSON value for MCP protocol
    fn response_to_json(response: SearchResponse) -> Value {
        serde_json::json!({
            "results": response.results.iter().map(|result| {
                serde_json::json!({
                    "name": result.name,
                    "version": result.version,
                    "description": result.description,
                    "docs_url": result.docs_url,
                    "download_count": result.download_count,
                    "last_updated": result.last_updated,
                    "repository": result.repository,
                    "homepage": result.homepage,
                    "keywords": result.keywords,
                    "categories": result.categories
                })
            }).collect::<Vec<_>>(),
            "total": response.total,
            "query": {
                "returned": response.results.len(),
                "requested": response.results.len()
            }
        })
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

        // Parse and validate input parameters
        let input: SearchToolInput =
            serde_json::from_value(params).context("Invalid search tool input parameters")?;
        input.validate()?;

        debug!(
            query = %input.query,
            limit = input.limit,
            "Processing search request"
        );

        let cache_key = Self::cache_key(&input);

        // Try to get from cache first
        {
            let cache_guard = cache.read().await;
            if let Some(cached_result) = cache_guard.get(&cache_key).await {
                trace!(
                    query = %input.query,
                    cache_key = %cache_key,
                    "Search cache hit"
                );
                return Ok(cached_result);
            }
        }

        trace!(
            query = %input.query,
            cache_key = %cache_key,
            "Search cache miss, fetching from API"
        );

        // Cache miss - fetch from API
        let search_request = input.to_search_request();
        let search_response = client
            .search_crates(search_request)
            .await
            .with_context(|| format!("Failed to search for crates with query: {}", input.query))?;

        debug!(
            query = %input.query,
            total_results = search_response.total.unwrap_or(0),
            returned_results = search_response.results.len(),
            "Search completed successfully"
        );

        // Transform response to JSON
        let json_response = Self::response_to_json(search_response);

        // Store in cache for future requests
        {
            let cache_guard = cache.write().await;
            cache_guard
                .insert(cache_key.clone(), json_response.clone())
                .await;
        }

        trace!(
            cache_key = %cache_key,
            "Search result cached"
        );

        Ok(json_response)
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
        let key1 = SearchTool::cache_key(&input1);
        assert_eq!(key1, "search:tokio:20");

        let input2 = SearchToolInput {
            query: "serde".to_string(),
            limit: None,
        };
        let key2 = SearchTool::cache_key(&input2);
        assert_eq!(key2, "search:serde:10");

        // Same query, different limit should have different keys
        let input3 = SearchToolInput {
            query: "tokio".to_string(),
            limit: Some(30),
        };
        let key3 = SearchTool::cache_key(&input3);
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

    #[test]
    fn test_search_tool_response_to_json() {
        use chrono::Utc;
        use rustacean_docs_core::models::search::{CrateSearchResult, SearchResponse};
        use url::Url;

        let search_result = CrateSearchResult {
            name: "tokio".to_string(),
            version: "1.0.0".to_string(),
            description: Some("An event-driven, non-blocking I/O platform".to_string()),
            docs_url: Some(Url::parse("https://docs.rs/tokio").unwrap()),
            download_count: Some(50000000),
            last_updated: Some(Utc::now()),
            repository: Some(Url::parse("https://github.com/tokio-rs/tokio").unwrap()),
            homepage: Some(Url::parse("https://tokio.rs").unwrap()),
            keywords: vec!["async".to_string(), "io".to_string()],
            categories: vec!["asynchronous".to_string()],
        };

        let search_response = SearchResponse::with_total(vec![search_result], 1);
        let json = SearchTool::response_to_json(search_response);

        assert!(json["results"].is_array());
        assert_eq!(json["results"].as_array().unwrap().len(), 1);
        assert_eq!(json["total"], 1);
        assert!(json["query"].is_object());

        let result = &json["results"][0];
        assert_eq!(result["name"], "tokio");
        assert_eq!(result["version"], "1.0.0");
        assert_eq!(
            result["description"],
            "An event-driven, non-blocking I/O platform"
        );
        assert_eq!(result["docs_url"], "https://docs.rs/tokio");
    }

    // Mock tests to ensure the structure is correct
    #[tokio::test]
    async fn test_search_tool_structure() {
        let tool = SearchTool::new();

        // Test that we can create the tool
        assert_eq!(tool.description().len() > 0, true);

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
        use rustacean_docs_client::DocsClient;
        use serde_json::json;
        use std::time::Duration;

        async fn create_test_environment() -> (DocsClient, Arc<RwLock<ServerCache>>) {
            let client = DocsClient::new().unwrap();
            let cache = Arc::new(RwLock::new(MemoryCache::new(
                100,
                Duration::from_secs(3600),
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
            let (_client, cache) = create_test_environment().await;

            // First, verify cache is empty
            {
                let cache_guard = cache.read().await;
                let stats = cache_guard.stats().await;
                assert_eq!(stats.size, 0);
            }

            // Test cache key generation
            let input = SearchToolInput {
                query: "test-crate".to_string(),
                limit: Some(20),
            };

            let cache_key = SearchTool::cache_key(&input);
            assert_eq!(cache_key, "search:test-crate:20");

            // Test that cache operations work
            let test_value = json!({"test": "data"});
            {
                let cache_guard = cache.write().await;
                cache_guard
                    .insert(cache_key.clone(), test_value.clone())
                    .await;
            }

            {
                let cache_guard = cache.read().await;
                let cached = cache_guard.get(&cache_key).await;
                assert!(cached.is_some());
                assert_eq!(cached.unwrap(), test_value);
            }
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
