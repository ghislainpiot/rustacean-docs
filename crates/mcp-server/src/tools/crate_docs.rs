use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, trace};

use rustacean_docs_cache::TieredCache;
use rustacean_docs_client::DocsClient;
use rustacean_docs_core::{
    models::docs::{CrateDocsRequest, CrateDocsResponse},
    Error,
};

use crate::tools::{
    CacheConfig, CacheStrategy, ErrorHandler, ParameterValidator, ToolErrorContext, ToolHandler,
    ToolInput,
};

// Type alias for our specific cache implementation
type ServerCache = TieredCache<String, Value>;

/// Input parameters for the get_crate_docs tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateDocsToolInput {
    /// Name of the crate (required)
    pub crate_name: String,
    /// Specific version to query (optional, defaults to latest)
    pub version: Option<String>,
}

impl ToolInput for CrateDocsToolInput {
    fn validate(&self) -> Result<(), Error> {
        ParameterValidator::validate_crate_name(&self.crate_name, "get_crate_docs")?;
        ParameterValidator::validate_version(&self.version, "get_crate_docs")?;
        Ok(())
    }

    fn cache_key(&self, tool_name: &str) -> String {
        match &self.version {
            Some(version) => format!("{}:{}:{}", tool_name, self.crate_name, version),
            None => format!("{}:{}:latest", tool_name, self.crate_name),
        }
    }
}

impl CrateDocsToolInput {
    /// Convert to internal CrateDocsRequest
    pub fn to_crate_docs_request(&self) -> CrateDocsRequest {
        match &self.version {
            Some(version) => CrateDocsRequest::with_version(&self.crate_name, version),
            None => CrateDocsRequest::new(&self.crate_name),
        }
    }
}

/// Crate docs tool that fetches comprehensive documentation for a Rust crate
pub struct CrateDocsTool;

impl CrateDocsTool {
    pub fn new() -> Self {
        Self
    }

    /// Transform CrateDocsResponse to JSON value for MCP protocol
    fn response_to_json(response: CrateDocsResponse) -> Value {
        serde_json::json!({
            "crate_name": response.name,
            "version": response.version,
            "docs_url": response.docs_url,
            "summary": {
                "description": response.summary.description,
                "module_count": response.summary.module_count,
                "struct_count": response.summary.struct_count,
                "trait_count": response.summary.trait_count,
                "function_count": response.summary.function_count,
                "enum_count": response.summary.enum_count,
                "features": response.summary.features
            },
            "categories": {
                "core_types": response.categories.core_types,
                "traits": response.categories.traits,
                "modules": response.categories.modules,
                "functions": response.categories.functions,
                "macros": response.categories.macros,
                "constants": response.categories.constants
            },
            "items": response.items.iter().map(|item| {
                serde_json::json!({
                    "name": item.name,
                    "kind": format!("{:?}", item.kind),
                    "summary": item.summary,
                    "path": item.path,
                    "visibility": format!("{:?}", item.visibility),
                    "is_async": item.is_async,
                    "signature": item.signature,
                    "docs_path": item.docs_path
                })
            }).collect::<Vec<_>>(),
            "examples": response.examples.iter().map(|example| {
                serde_json::json!({
                    "title": example.title,
                    "code": example.code,
                    "language": example.language,
                    "is_runnable": example.is_runnable
                })
            }).collect::<Vec<_>>()
        })
    }
}

#[async_trait::async_trait]
impl ToolHandler for CrateDocsTool {
    async fn execute(
        &self,
        params: Value,
        client: &Arc<DocsClient>,
        cache: &Arc<RwLock<ServerCache>>,
    ) -> Result<Value> {
        trace!("Executing crate docs tool with params: {}", params);

        // Parse input parameters
        let input: CrateDocsToolInput = serde_json::from_value(params.clone()).map_err(|e| {
            anyhow::anyhow!(
                "{}: {}",
                ErrorHandler::parameter_parsing_context("get_crate_docs"),
                e
            )
        })?;

        debug!(
            crate_name = %input.crate_name,
            version = ?input.version,
            "Processing crate docs request"
        );

        // Use unified cache strategy
        CacheStrategy::execute_with_cache(
            "crate_docs",
            params,
            input,
            CacheConfig::default(),
            client,
            cache,
            |input, client| async move {
                // Fetch from docs.rs
                let docs_request = input.to_crate_docs_request();
                let docs_response = client.get_crate_docs(docs_request).await.crate_context(
                    "fetch documentation",
                    &input.crate_name,
                    input.version.as_deref(),
                )?;

                debug!(
                    crate_name = %docs_response.name,
                    version = %docs_response.version,
                    item_count = docs_response.items.len(),
                    example_count = docs_response.examples.len(),
                    "Crate documentation fetched successfully"
                );

                // Transform response to JSON
                let json_response = Self::response_to_json(docs_response);
                Ok(json_response)
            },
        )
        .await
    }

    fn description(&self) -> &str {
        "Fetch comprehensive documentation overview for a specific crate with enhanced LLM-friendly structure. Returns organized information including summary, categories, complete item listings with metadata, and code examples."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "crate_name": {
                    "type": "string",
                    "description": "Name of the crate (e.g., \"serde\", \"tokio\")",
                    "minLength": 1,
                    "pattern": "^[a-zA-Z0-9_-]+$"
                },
                "version": {
                    "type": "string",
                    "description": "Optional version (defaults to latest stable version)",
                    "examples": ["1.0.0", "0.11.4", "2.0.0-alpha.1"]
                }
            },
            "required": ["crate_name"],
            "additionalProperties": false
        })
    }
}

impl Default for CrateDocsTool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_crate_docs_tool_input_validation() {
        // Valid input with version
        let valid_input = CrateDocsToolInput {
            crate_name: "tokio".to_string(),
            version: Some("1.0.0".to_string()),
        };
        assert!(valid_input.validate().is_ok());

        // Valid input without version
        let valid_no_version = CrateDocsToolInput {
            crate_name: "serde".to_string(),
            version: None,
        };
        assert!(valid_no_version.validate().is_ok());

        // Empty crate name
        let empty_crate = CrateDocsToolInput {
            crate_name: "".to_string(),
            version: None,
        };
        assert!(empty_crate.validate().is_err());

        // Whitespace crate name
        let whitespace_crate = CrateDocsToolInput {
            crate_name: "   ".to_string(),
            version: None,
        };
        assert!(whitespace_crate.validate().is_err());

        // Invalid characters in crate name
        let invalid_crate = CrateDocsToolInput {
            crate_name: "invalid/crate@name".to_string(),
            version: None,
        };
        assert!(invalid_crate.validate().is_err());

        // Empty version string
        let empty_version = CrateDocsToolInput {
            crate_name: "tokio".to_string(),
            version: Some("".to_string()),
        };
        assert!(empty_version.validate().is_err());

        // Valid crate name with hyphens and underscores
        let valid_with_separators = CrateDocsToolInput {
            crate_name: "async-trait".to_string(),
            version: None,
        };
        assert!(valid_with_separators.validate().is_ok());

        let valid_with_underscores = CrateDocsToolInput {
            crate_name: "proc_macro2".to_string(),
            version: None,
        };
        assert!(valid_with_underscores.validate().is_ok());
    }

    #[test]
    fn test_crate_docs_tool_input_to_request() {
        let input_with_version = CrateDocsToolInput {
            crate_name: "tokio".to_string(),
            version: Some("1.35.0".to_string()),
        };
        let request = input_with_version.to_crate_docs_request();
        assert_eq!(request.crate_name, "tokio");
        assert_eq!(request.version, Some("1.35.0".to_string()));

        let input_no_version = CrateDocsToolInput {
            crate_name: "serde".to_string(),
            version: None,
        };
        let request = input_no_version.to_crate_docs_request();
        assert_eq!(request.crate_name, "serde");
        assert_eq!(request.version, None);
    }

    #[test]
    fn test_crate_docs_tool_cache_key() {
        let input1 = CrateDocsToolInput {
            crate_name: "tokio".to_string(),
            version: Some("1.0.0".to_string()),
        };
        let key1 = input1.cache_key("crate_docs");
        assert_eq!(key1, "crate_docs:tokio:1.0.0");

        let input2 = CrateDocsToolInput {
            crate_name: "serde".to_string(),
            version: None,
        };
        let key2 = input2.cache_key("crate_docs");
        assert_eq!(key2, "crate_docs:serde:latest");

        // Same crate, different version should have different keys
        let input3 = CrateDocsToolInput {
            crate_name: "tokio".to_string(),
            version: Some("1.1.0".to_string()),
        };
        let key3 = input3.cache_key("crate_docs");
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_crate_docs_tool_input_serialization() {
        let input = CrateDocsToolInput {
            crate_name: "async-trait".to_string(),
            version: Some("0.1.68".to_string()),
        };

        // Test serialization
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(json["crate_name"], "async-trait");
        assert_eq!(json["version"], "0.1.68");

        // Test deserialization
        let deserialized: CrateDocsToolInput = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized.crate_name, input.crate_name);
        assert_eq!(deserialized.version, input.version);
    }

    #[test]
    fn test_crate_docs_tool_input_from_json() {
        // Test with version
        let json_with_version = json!({
            "crate_name": "reqwest",
            "version": "0.11.4"
        });

        let input: CrateDocsToolInput = serde_json::from_value(json_with_version).unwrap();
        assert_eq!(input.crate_name, "reqwest");
        assert_eq!(input.version, Some("0.11.4".to_string()));

        // Test without version
        let json_no_version = json!({
            "crate_name": "tracing"
        });

        let input: CrateDocsToolInput = serde_json::from_value(json_no_version).unwrap();
        assert_eq!(input.crate_name, "tracing");
        assert_eq!(input.version, None);

        // Test invalid JSON structure
        let invalid_json = json!({
            "not_crate_name": "test"
        });

        let result: Result<CrateDocsToolInput, _> = serde_json::from_value(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_crate_docs_tool_description() {
        let tool = CrateDocsTool::new();
        let description = tool.description();
        assert!(!description.is_empty());
        assert!(description.contains("comprehensive"));
        assert!(description.contains("documentation"));
        assert!(description.contains("crate"));
    }

    #[test]
    fn test_crate_docs_tool_parameters_schema() {
        let tool = CrateDocsTool::new();
        let schema = tool.parameters_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["crate_name"].is_object());
        assert!(schema["properties"]["version"].is_object());
        assert_eq!(schema["required"][0], "crate_name");

        // Check crate_name property
        let crate_name_prop = &schema["properties"]["crate_name"];
        assert_eq!(crate_name_prop["type"], "string");
        assert_eq!(crate_name_prop["minLength"], 1);
        assert!(crate_name_prop["pattern"].is_string());

        // Check version property
        let version_prop = &schema["properties"]["version"];
        assert_eq!(version_prop["type"], "string");
        assert!(version_prop["examples"].is_array());
    }

    #[test]
    fn test_crate_docs_tool_response_to_json() {
        use rustacean_docs_core::models::docs::{
            CodeExample, CrateCategories, CrateDocsResponse, CrateItem, CrateSummary, ItemKind,
            Visibility,
        };
        use url::Url;

        let crate_item = CrateItem {
            name: "spawn".to_string(),
            kind: ItemKind::Function,
            summary: Some("Spawn a new task".to_string()),
            path: "fn.spawn.html".to_string(),
            visibility: Visibility::Public,
            is_async: true,
            signature: Some("pub fn spawn<T>(task: T) -> JoinHandle<T::Output>".to_string()),
            docs_path: Some("fn.spawn.html".to_string()),
        };

        let code_example = CodeExample {
            title: Some("Basic usage".to_string()),
            code: "tokio::spawn(async { println!(\"Hello\"); });".to_string(),
            language: "rust".to_string(),
            is_runnable: true,
        };

        let docs_response = CrateDocsResponse {
            name: "tokio".to_string(),
            version: "1.35.0".to_string(),
            summary: CrateSummary {
                description: Some("A runtime for async applications".to_string()),
                module_count: 5,
                struct_count: 10,
                trait_count: 3,
                function_count: 25,
                enum_count: 2,
                features: vec!["full".to_string(), "rt".to_string()],
            },
            categories: CrateCategories {
                core_types: vec!["Runtime".to_string()],
                traits: vec!["Future".to_string()],
                modules: vec!["sync".to_string()],
                functions: vec!["spawn".to_string()],
                macros: vec!["select!".to_string()],
                constants: vec![],
            },
            items: vec![crate_item],
            examples: vec![code_example],
            docs_url: Some(Url::parse("https://docs.rs/tokio/1.35.0").unwrap()),
        };

        let json = CrateDocsTool::response_to_json(docs_response);

        assert_eq!(json["crate_name"], "tokio");
        assert_eq!(json["version"], "1.35.0");
        assert_eq!(json["docs_url"], "https://docs.rs/tokio/1.35.0");

        // Check summary
        let summary = &json["summary"];
        assert_eq!(summary["description"], "A runtime for async applications");
        assert_eq!(summary["module_count"], 5);
        assert_eq!(summary["struct_count"], 10);

        // Check categories
        let categories = &json["categories"];
        assert_eq!(categories["core_types"][0], "Runtime");
        assert_eq!(categories["traits"][0], "Future");

        // Check items
        assert!(json["items"].is_array());
        assert_eq!(json["items"].as_array().unwrap().len(), 1);
        let item = &json["items"][0];
        assert_eq!(item["name"], "spawn");
        assert_eq!(item["kind"], "Function");
        assert_eq!(item["is_async"], true);

        // Check examples
        assert!(json["examples"].is_array());
        assert_eq!(json["examples"].as_array().unwrap().len(), 1);
        let example = &json["examples"][0];
        assert_eq!(example["title"], "Basic usage");
        assert_eq!(example["is_runnable"], true);
    }

    // Mock tests to ensure the structure is correct
    #[tokio::test]
    async fn test_crate_docs_tool_structure() {
        let tool = CrateDocsTool::new();

        // Test that we can create the tool
        assert!(!tool.description().is_empty());

        // Test schema is valid JSON
        let schema = tool.parameters_schema();
        assert!(schema.is_object());

        // Ensure we have proper trait implementations
        let _tool_handler: &dyn ToolHandler = &tool;
    }

    // Integration tests would be added here for actual HTTP calls
    #[cfg(feature = "integration-tests")]
    mod integration_tests {
        use super::*;
        use rustacean_docs_cache::TieredCache;
        use rustacean_docs_client::DocsClient;
        use serde_json::json;
        use std::time::Duration;

        async fn create_test_environment() -> (DocsClient, Arc<RwLock<ServerCache>>) {
            let client = DocsClient::new().unwrap();
            let cache = Arc::new(RwLock::new(
                TieredCache::new(
                    50,                        // memory capacity
                    Duration::from_secs(3600), // memory TTL
                    1024 * 1024,               // disk max size (1MB)
                    Duration::from_secs(7200), // disk TTL
                    std::env::temp_dir().join("test_cache_crate_docs"),
                )
                .await
                .unwrap(),
            ));
            (client, cache)
        }

        #[tokio::test]
        async fn test_crate_docs_tool_invalid_input() {
            let (client, cache) = create_test_environment().await;
            let client = Arc::new(client);
            let tool = CrateDocsTool::new();

            // Test empty crate name
            let params = json!({
                "crate_name": ""
            });

            let result = tool.execute(params, &client, &cache).await;
            assert!(result.is_err(), "Empty crate name should fail");

            // Test invalid crate name
            let params = json!({
                "crate_name": "invalid/name"
            });

            let result = tool.execute(params, &client, &cache).await;
            assert!(result.is_err(), "Invalid crate name should fail");

            // Test empty version
            let params = json!({
                "crate_name": "tokio",
                "version": ""
            });

            let result = tool.execute(params, &client, &cache).await;
            assert!(result.is_err(), "Empty version should fail");
        }

        #[tokio::test]
        async fn test_crate_docs_tool_malformed_input() {
            let (client, cache) = create_test_environment().await;
            let client = Arc::new(client);
            let tool = CrateDocsTool::new();

            // Test missing required field
            let params = json!({
                "version": "1.0.0"
            });

            let result = tool.execute(params, &client, &cache).await;
            assert!(result.is_err(), "Missing crate_name field should fail");

            // Test wrong type
            let params = json!({
                "crate_name": 123
            });

            let result = tool.execute(params, &client, &cache).await;
            assert!(result.is_err(), "Wrong crate_name type should fail");
        }

        #[tokio::test]
        async fn test_crate_docs_tool_cache_behavior() {
            let (client, cache) = create_test_environment().await;
            let _client = Arc::new(client);

            // Test cache key generation
            let input = CrateDocsToolInput {
                crate_name: "test-crate".to_string(),
                version: Some("1.0.0".to_string()),
            };

            let cache_key = input.cache_key("crate_docs");
            assert_eq!(cache_key, "crate_docs:test-crate:1.0.0");

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
                let cached = cache_guard.get(&cache_key).await.unwrap();
                assert!(cached.is_some());
                assert_eq!(cached.unwrap(), test_value);
            }
        }
    }
}
