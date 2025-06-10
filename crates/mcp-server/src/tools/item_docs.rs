use super::ToolHandler;
use anyhow::Result;
use rustacean_docs_cache::{Cache, TieredCache};
use rustacean_docs_client::DocsClient;
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
        cache: &Arc<RwLock<TieredCache<String, Value>>>,
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

        // Generate cache key
        let cache_key = format!(
            "item_docs:{}:{}:{}",
            crate_name,
            item_path,
            version.as_deref().unwrap_or("latest")
        );

        // Try to get from cache first
        {
            let cache_guard = cache.read().await;
            if let Ok(Some(cached_result)) = cache_guard.get(&cache_key).await {
                trace!(
                    crate_name = %crate_name,
                    item_path = %item_path,
                    cache_key = %cache_key,
                    "Item docs cache hit"
                );
                return Ok(cached_result);
            }
        }

        trace!(
            crate_name = %crate_name,
            item_path = %item_path,
            cache_key = %cache_key,
            "Item docs cache miss, fetching from API"
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

        // Execute request
        let response = client.get_item_docs(request).await?;

        debug!(
            crate_name = %response.crate_name,
            item_name = %response.name,
            has_signature = response.signature.is_some(),
            has_description = response.description.is_some(),
            examples_count = response.examples.len(),
            related_items_count = response.related_items.len(),
            "Item documentation retrieved successfully"
        );

        // Convert to JSON response
        let json_response = json!({
            "crate_name": response.crate_name,
            "item_path": response.item_path,
            "name": response.name,
            "kind": response.kind,
            "signature": response.signature,
            "description": response.description,
            "examples": response.examples.iter().map(|ex| json!({
                "title": ex.title,
                "code": ex.code,
                "language": ex.language,
                "is_runnable": ex.is_runnable
            })).collect::<Vec<_>>(),
            "docs_url": response.docs_url.map(|url| url.to_string()),
            "related_items": response.related_items
        });

        // Store in cache for future requests
        {
            let cache_guard = cache.read().await;
            if let Err(e) = cache_guard
                .insert(cache_key.clone(), json_response.clone())
                .await
            {
                debug!("Failed to cache item docs result: {}", e);
            }
        }

        trace!(
            cache_key = %cache_key,
            "Item docs result cached"
        );

        Ok(json_response)
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

        let cache = TieredCache::new(
            10,
            std::time::Duration::from_secs(60),
            temp_dir,
            std::time::Duration::from_secs(3600),
            1024 * 1024,
        )
        .await
        .unwrap();
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
