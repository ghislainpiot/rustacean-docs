use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info};

use rustacean_docs_cache::{Cache, DiskCache, MemoryCache, TieredCache, WriteStrategy};
use rustacean_docs_client::DocsClient;

use crate::config::Config;
use crate::tools::ToolHandler;

// Type alias for our specific cache implementation
type ServerCache = TieredCache<String, Value>;

pub struct McpServer {
    tools: HashMap<String, Box<dyn ToolHandler>>,
    client: Arc<DocsClient>,
    cache: Arc<RwLock<ServerCache>>,
    config: Config,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub name: String,
    pub version: String,
    pub description: String,
    pub max_cache_size: usize,
    pub cache_ttl_secs: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            name: "rustacean-docs-mcp".to_string(),
            version: "0.1.0".to_string(),
            description: "MCP server for Rust documentation access".to_string(),
            max_cache_size: 1000,
            cache_ttl_secs: 3600, // 1 hour
        }
    }
}

impl McpServer {
    pub async fn new(config: Config) -> Result<Self> {
        config.validate()?;

        let client = Arc::new(DocsClient::new()?);

        // Create cache directory if it doesn't exist
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("rustacean-docs");

        if !cache_dir.exists() {
            tokio::fs::create_dir_all(&cache_dir).await?;
        }

        let _memory_ttl = Duration::from_secs(config.cache.memory_ttl_secs);
        let _disk_ttl = Duration::from_secs(config.cache.disk_ttl_secs);
        let _disk_max_size = config.cache.disk_max_size_mb * 1024 * 1024; // Convert MB to bytes

        // Create individual cache layers
        let memory_cache = MemoryCache::new(config.cache.memory_max_entries);
        let disk_cache = DiskCache::new(&cache_dir);

        // Wrap memory cache to match error type
        struct MemoryCacheWrapper(MemoryCache<String, Value>);

        #[async_trait]
        impl Cache for MemoryCacheWrapper {
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

        // Create tiered cache
        let cache = Arc::new(RwLock::new(TieredCache::new(
            vec![
                Box::new(MemoryCacheWrapper(memory_cache)),
                Box::new(disk_cache),
            ],
            WriteStrategy::WriteThrough,
        )));

        let mut server = Self {
            tools: HashMap::new(),
            client,
            cache,
            config,
        };

        server.register_default_tools()?;

        Ok(server)
    }

    pub async fn with_default_config() -> Result<Self> {
        Self::new(Config::load()?).await
    }

    pub async fn initialize(&mut self) -> Result<()> {
        info!(
            "Initializing MCP server: {} v{}",
            self.config.server.name, self.config.server.version
        );

        debug!(
            "Registered tools: {:?}",
            self.tools.keys().collect::<Vec<_>>()
        );
        debug!("Server configuration: {:?}", self.config);

        Ok(())
    }

    fn register_default_tools(&mut self) -> Result<()> {
        info!("Registering default tools...");

        // Register the search tool
        self.register_tool("search_crate", Box::new(crate::tools::SearchTool::new()))?;

        // Register the crate docs tool
        self.register_tool(
            "get_crate_docs",
            Box::new(crate::tools::CrateDocsTool::new()),
        )?;

        // Register the item docs tool
        self.register_tool("get_item_docs", Box::new(crate::tools::ItemDocsTool::new()))?;

        // Register the metadata tool
        self.register_tool(
            "get_crate_metadata",
            Box::new(crate::tools::CrateMetadataTool::new()),
        )?;

        // Register the recent releases tool
        self.register_tool(
            "list_recent_releases",
            Box::new(crate::tools::RecentReleasesTool::new()),
        )?;

        // Register cache management tools
        self.register_tool(
            "get_cache_stats",
            Box::new(crate::tools::CacheStatsTool::new()),
        )?;
        self.register_tool("clear_cache", Box::new(crate::tools::ClearCacheTool::new()))?;
        self.register_tool("cache_info", Box::new(crate::tools::CacheInfoTool::new()))?;

        info!("Registered {} tools", self.tools.len());
        Ok(())
    }

    pub fn register_tool(&mut self, name: &str, handler: Box<dyn ToolHandler>) -> Result<()> {
        if self.tools.contains_key(name) {
            return Err(anyhow::anyhow!("Tool '{}' is already registered", name));
        }

        info!("Registering tool: {}", name);
        self.tools.insert(name.to_string(), handler);
        Ok(())
    }

    pub fn list_tools(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    pub async fn handle_tool_call(&self, tool_name: &str, params: Value) -> Result<Value> {
        debug!("Handling tool call: {} with params: {}", tool_name, params);

        let tool = self
            .tools
            .get(tool_name)
            .ok_or_else(|| anyhow::anyhow!("Unknown tool: {}", tool_name))?;

        tool.execute(params, &self.client, &self.cache).await
    }

    pub fn get_server_info(&self) -> Value {
        serde_json::json!({
            "name": self.config.server.name,
            "version": self.config.server.version,
            "description": self.config.server.description,
            "tools": self.list_tools(),
            "cache": {
                "max_entries": self.config.cache.memory_max_entries,
                "ttl_secs": self.config.cache.memory_ttl_secs
            }
        })
    }

    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down MCP server");

        // Clear cache statistics or perform cleanup
        let cache_stats = {
            let cache = self.cache.read().await;
            cache.stats()
        };

        info!("Final cache statistics: {:?}", cache_stats);

        Ok(())
    }
}

// Note: Default implementation is not provided for McpServer because it requires async initialization.
// Use McpServer::with_default_config().await instead.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    struct MockTool;

    #[async_trait::async_trait]
    impl ToolHandler for MockTool {
        async fn execute(
            &self,
            _params: Value,
            _client: &Arc<DocsClient>,
            _cache: &Arc<RwLock<ServerCache>>,
        ) -> Result<Value> {
            Ok(serde_json::json!({"result": "mock"}))
        }

        fn description(&self) -> &str {
            "Mock tool for testing"
        }

        fn parameters_schema(&self) -> Value {
            serde_json::json!({})
        }
    }

    #[tokio::test]
    async fn test_server_creation() {
        let config = Config::default();
        let server = McpServer::new(config).await;
        assert!(server.is_ok());
    }

    #[tokio::test]
    async fn test_server_initialization() {
        let config = Config::default();
        let mut server = McpServer::new(config).await.unwrap();
        let result = server.initialize().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_tool_registration() {
        let config = Config::default();
        let mut server = McpServer::new(config).await.unwrap();

        let tool = Box::new(MockTool);
        let result = server.register_tool("test_tool", tool);
        assert!(result.is_ok());

        let tools = server.list_tools();
        assert!(tools.contains(&"test_tool".to_string()));
    }

    #[tokio::test]
    async fn test_duplicate_tool_registration() {
        let config = Config::default();
        let mut server = McpServer::new(config).await.unwrap();

        let tool1 = Box::new(MockTool);
        let tool2 = Box::new(MockTool);

        server.register_tool("test_tool", tool1).unwrap();
        let result = server.register_tool("test_tool", tool2);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tool_execution() {
        let config = Config::default();
        let mut server = McpServer::new(config).await.unwrap();

        let tool = Box::new(MockTool);
        server.register_tool("test_tool", tool).unwrap();

        let params = serde_json::json!({});
        let result = server.handle_tool_call("test_tool", params).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response["result"], "mock");
    }

    #[tokio::test]
    async fn test_unknown_tool_execution() {
        let config = Config::default();
        let server = McpServer::new(config).await.unwrap();

        let params = serde_json::json!({});
        let result = server.handle_tool_call("unknown_tool", params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_server_info() {
        let config = Config::default();
        let server = McpServer::new(config).await.unwrap();

        let info = server.get_server_info();
        assert!(info["name"].is_string());
        assert!(info["version"].is_string());
        assert!(info["description"].is_string());
        assert!(info["tools"].is_array());
        assert!(info["cache"].is_object());
    }

    #[tokio::test]
    async fn test_server_shutdown() {
        let config = Config::default();
        let server = McpServer::new(config).await.unwrap();

        let result = server.shutdown().await;
        assert!(result.is_ok());
    }
}
