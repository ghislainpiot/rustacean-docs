use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use rustacean_docs_cache::TieredCache;
use rustacean_docs_client::{endpoints::docs_modules::service::DocsService, DocsClient};
use rustacean_docs_core::{
    models::docs::ItemDocsRequest,
    types::{CrateName, ItemPath, Version},
    Error, ErrorBuilder,
};

use crate::tools::{
    CacheConfig, CacheStrategy, ErrorHandler, ParameterValidator, ToolErrorContext, ToolHandler,
    ToolInput,
};

// Type alias for our specific cache implementation
type ServerCache = TieredCache<String, Value>;

/// Input parameters for the get_item_docs tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemDocsToolInput {
    /// Name of the crate (e.g., "serde")
    pub crate_name: String,
    /// Item identifier - can be simple name ("Serialize") or full path ("de/struct.Error.html")
    pub item_path: String,
    /// Specific version to query (defaults to latest stable version)
    pub version: Option<String>,
}

impl ToolInput for ItemDocsToolInput {
    fn validate(&self) -> Result<(), Error> {
        ParameterValidator::validate_crate_name(&self.crate_name, "get_item_docs")?;
        if self.item_path.trim().is_empty() {
            return Err(ErrorBuilder::protocol()
                .invalid_input("get_item_docs", "item_path cannot be empty"));
        }
        ParameterValidator::validate_version(&self.version, "get_item_docs")?;
        Ok(())
    }

    fn cache_key(&self, tool_name: &str) -> String {
        match &self.version {
            Some(version) => format!(
                "{}:{}:{}:{}",
                tool_name, self.crate_name, self.item_path, version
            ),
            None => format!(
                "{}:{}:{}:latest",
                tool_name, self.crate_name, self.item_path
            ),
        }
    }
}

impl ItemDocsToolInput {
    /// Convert to internal ItemDocsRequest
    pub fn to_item_docs_request(&self) -> Result<ItemDocsRequest, Error> {
        let crate_name = CrateName::new(&self.crate_name)
            .map_err(|e| Error::Internal(format!("Invalid crate name: {e}")))?;
        let item_path = ItemPath::new(&self.item_path)
            .map_err(|e| Error::Internal(format!("Invalid item path: {e}")))?;

        match &self.version {
            Some(version) => {
                let version = Version::new(version)
                    .map_err(|e| Error::Internal(format!("Invalid version: {e}")))?;
                Ok(ItemDocsRequest::with_version(
                    crate_name, item_path, version,
                ))
            }
            None => Ok(ItemDocsRequest::new(crate_name, item_path)),
        }
    }
}

/// Tool handler for retrieving specific item documentation
pub struct ItemDocsTool;

impl ItemDocsTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ItemDocsTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ToolHandler for ItemDocsTool {
    async fn execute(
        &self,
        params: Value,
        client: &Arc<DocsClient>,
        cache: &Arc<RwLock<ServerCache>>,
    ) -> Result<Value> {
        debug!("Executing get_item_docs tool with params: {}", params);

        // Parse input parameters
        let input: ItemDocsToolInput = serde_json::from_value(params.clone()).map_err(|e| {
            anyhow::anyhow!(
                "{}: {}",
                ErrorHandler::parameter_parsing_context("get_item_docs"),
                e
            )
        })?;

        debug!(
            crate_name = %input.crate_name,
            item_path = %input.item_path,
            version = ?input.version,
            "Processing item docs request"
        );

        // Use unified cache strategy
        CacheStrategy::execute_with_cache(
            "get_item_docs",
            params,
            input,
            CacheConfig::default(),
            client,
            cache,
            |input, client| async move {
                // Create docs service without internal cache since we're using server-level cache
                let docs_service = DocsService::new(
                    (*client).clone(),
                    0,                                 // disable internal cache
                    std::time::Duration::from_secs(0), // no TTL needed
                );

                // Convert to item docs request
                let request = input.to_item_docs_request()?;

                // Fetch item documentation
                let response = docs_service
                    .get_item_docs(request.clone())
                    .await
                    .crate_context(
                        "fetch item documentation",
                        request.crate_name.as_str(),
                        request.version.as_ref().map(|v| v.as_str()),
                    )?;

                debug!(
                    crate_name = %response.crate_name,
                    item_name = %response.name,
                    has_signature = response.signature.is_some(),
                    has_description = response.description.is_some(),
                    examples_count = response.examples.len(),
                    related_items_count = response.related_items.len(),
                    "Item documentation retrieved successfully"
                );

                // Serialize response to JSON
                Ok(serde_json::to_value(response)?)
            },
        )
        .await
    }

    fn description(&self) -> &str {
        "Get detailed documentation for specific items (functions, structs, traits, enums, modules)"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "crate_name": {
                    "type": "string",
                    "description": "Name of the crate (e.g., \"serde\")"
                },
                "item_path": {
                    "type": "string",
                    "description": "Item identifier - can be simple name (\"Serialize\") or full path (\"de/struct.Error.html\")"
                },
                "version": {
                    "type": "string",
                    "description": "Specific version to query (defaults to latest stable version)"
                }
            },
            "required": ["crate_name", "item_path"],
            "additionalProperties": false
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    async fn create_test_cache() -> Arc<RwLock<ServerCache>> {
        let temp_dir = std::env::temp_dir().join("item_docs_test_cache");
        if temp_dir.exists() {
            tokio::fs::remove_dir_all(&temp_dir).await.ok();
        }
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        // Create individual cache layers
        let memory_cache = rustacean_docs_cache::MemoryCache::new(10);
        let disk_cache = rustacean_docs_cache::DiskCache::new(&temp_dir);

        // Wrap memory cache to match error type
        struct MemoryCacheWrapper(rustacean_docs_cache::MemoryCache<String, Value>);

        #[async_trait::async_trait]
        impl rustacean_docs_cache::Cache for MemoryCacheWrapper {
            type Key = String;
            type Value = Value;
            type Error = anyhow::Error;

            async fn get(&self, key: &Self::Key) -> Result<Option<Self::Value>, Self::Error> {
                self.0
                    .get(key)
                    .await
                    .map_err(|_| anyhow::anyhow!("Memory cache error"))
            }

            async fn insert(&self, key: Self::Key, value: Self::Value) -> Result<(), Self::Error> {
                self.0
                    .insert(key, value)
                    .await
                    .map_err(|_| anyhow::anyhow!("Memory cache error"))
            }

            async fn remove(&self, key: &Self::Key) -> Result<(), Self::Error> {
                self.0
                    .remove(key)
                    .await
                    .map_err(|_| anyhow::anyhow!("Memory cache error"))
            }

            async fn clear(&self) -> Result<(), Self::Error> {
                self.0
                    .clear()
                    .await
                    .map_err(|_| anyhow::anyhow!("Memory cache error"))
            }

            fn stats(&self) -> rustacean_docs_cache::CacheStats {
                self.0.stats()
            }
        }

        let cache = ServerCache::new(
            vec![
                Box::new(MemoryCacheWrapper(memory_cache)),
                Box::new(disk_cache),
            ],
            rustacean_docs_cache::WriteStrategy::WriteThrough,
        );
        Arc::new(RwLock::new(cache))
    }

    #[test]
    fn test_item_docs_tool_creation() {
        let tool = ItemDocsTool::new();
        assert_eq!(
            tool.description(),
            "Get detailed documentation for specific items (functions, structs, traits, enums, modules)"
        );
    }

    #[test]
    fn test_parameters_schema() {
        let tool = ItemDocsTool::new();
        let schema = tool.parameters_schema();

        assert!(schema.is_object());
        let properties = schema.get("properties").unwrap();
        assert!(properties.get("crate_name").is_some());
        assert!(properties.get("item_path").is_some());
        assert!(properties.get("version").is_some());

        let required = schema.get("required").unwrap().as_array().unwrap();
        assert!(required.contains(&json!("crate_name")));
        assert!(required.contains(&json!("item_path")));
        assert!(!required.contains(&json!("version"))); // version is optional
    }

    #[test]
    fn test_item_docs_tool_input_validation() {
        // Valid input
        let valid_input = ItemDocsToolInput {
            crate_name: "tokio".to_string(),
            item_path: "spawn".to_string(),
            version: Some("1.0.0".to_string()),
        };
        assert!(valid_input.validate().is_ok());

        // Empty crate name
        let empty_crate = ItemDocsToolInput {
            crate_name: "".to_string(),
            item_path: "spawn".to_string(),
            version: None,
        };
        assert!(empty_crate.validate().is_err());

        // Empty item path
        let empty_path = ItemDocsToolInput {
            crate_name: "tokio".to_string(),
            item_path: "".to_string(),
            version: None,
        };
        assert!(empty_path.validate().is_err());

        // Empty version
        let empty_version = ItemDocsToolInput {
            crate_name: "tokio".to_string(),
            item_path: "spawn".to_string(),
            version: Some("".to_string()),
        };
        assert!(empty_version.validate().is_err());
    }

    #[test]
    fn test_item_docs_tool_input_to_request() {
        let input_with_version = ItemDocsToolInput {
            crate_name: "tokio".to_string(),
            item_path: "spawn".to_string(),
            version: Some("1.35.0".to_string()),
        };
        let request = input_with_version.to_item_docs_request().unwrap();
        assert_eq!(request.crate_name.as_str(), "tokio");
        assert_eq!(request.item_path.as_str(), "spawn");
        assert_eq!(request.version.as_ref().map(|v| v.as_str()), Some("1.35.0"));

        let input_no_version = ItemDocsToolInput {
            crate_name: "serde".to_string(),
            item_path: "Serialize".to_string(),
            version: None,
        };
        let request = input_no_version.to_item_docs_request().unwrap();
        assert_eq!(request.crate_name.as_str(), "serde");
        assert_eq!(request.item_path.as_str(), "Serialize");
        assert_eq!(request.version, None);
    }

    #[test]
    fn test_item_docs_tool_cache_key() {
        let input1 = ItemDocsToolInput {
            crate_name: "tokio".to_string(),
            item_path: "spawn".to_string(),
            version: Some("1.0.0".to_string()),
        };
        let key1 = input1.cache_key("item_docs");
        assert_eq!(key1, "item_docs:tokio:spawn:1.0.0");

        let input2 = ItemDocsToolInput {
            crate_name: "serde".to_string(),
            item_path: "Serialize".to_string(),
            version: None,
        };
        let key2 = input2.cache_key("item_docs");
        assert_eq!(key2, "item_docs:serde:Serialize:latest");
    }

    #[tokio::test]
    async fn test_execute_missing_crate_name() {
        let tool = ItemDocsTool::new();
        let client = Arc::new(DocsClient::new().unwrap());
        let cache = create_test_cache().await;

        let params = json!({
            "item_path": "spawn"
        });

        let result = tool.execute(params, &client, &cache).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid input parameters"));
    }

    #[tokio::test]
    async fn test_execute_missing_item_path() {
        let tool = ItemDocsTool::new();
        let client = Arc::new(DocsClient::new().unwrap());
        let cache = create_test_cache().await;

        let params = json!({
            "crate_name": "tokio"
        });

        let result = tool.execute(params, &client, &cache).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid input parameters"));
    }

    // Note: More comprehensive integration tests would require mock HTTP clients
    // which would be implemented in integration test files
}
