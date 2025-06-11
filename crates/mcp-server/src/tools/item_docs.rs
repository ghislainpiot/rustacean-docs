use super::ToolHandler;
use anyhow::Result;
use rustacean_docs_cache::TieredCache;
use rustacean_docs_client::{endpoints::docs_modules::service::DocsService, DocsClient};
use rustacean_docs_core::models::docs::ItemDocsRequest;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, trace};

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
        _cache: &Arc<RwLock<TieredCache<String, Value>>>,
    ) -> Result<Value> {
        trace!(params = ?params, "Executing item docs tool");

        // Parse parameters
        let crate_name = params
            .get("crate_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: crate_name"))?;

        let item_path = params
            .get("item_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: item_path"))?;

        let version = params
            .get("version")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Create docs service with cache
        let docs_service = DocsService::new(
            (**client).clone(),
            100,                                  // cache capacity
            std::time::Duration::from_secs(3600), // 1 hour TTL
        );

        // Create request
        let request = if let Some(version) = version {
            ItemDocsRequest::with_version(crate_name, item_path, &version)
        } else {
            ItemDocsRequest::new(crate_name, item_path)
        };

        debug!(
            crate_name = %request.crate_name,
            item_path = %request.item_path,
            version = ?request.version,
            "Getting item documentation"
        );

        // Use DocsService directly - it handles caching internally
        let response = docs_service.get_item_docs(request).await?;

        debug!(
            crate_name = %response.crate_name,
            item_name = %response.name,
            has_signature = response.signature.is_some(),
            has_description = response.description.is_some(),
            examples_count = response.examples.len(),
            related_items_count = response.related_items.len(),
            "Item documentation retrieved successfully"
        );

        // Serialize response to JSON - no manual transformation needed
        Ok(serde_json::to_value(response)?)
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
            "required": ["crate_name", "item_path"]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_cache() -> Arc<RwLock<TieredCache<String, Value>>> {
        let temp_dir = std::env::temp_dir().join("item_docs_test_cache");
        if temp_dir.exists() {
            std::fs::remove_dir_all(&temp_dir).ok();
        }
        std::fs::create_dir_all(&temp_dir).unwrap();

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

        let cache = TieredCache::new(
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
            .contains("Missing required parameter: crate_name"));
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
            .contains("Missing required parameter: item_path"));
    }

    // Note: More comprehensive integration tests would require mock HTTP clients
    // which would be implemented in integration test files
}
