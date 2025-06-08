use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info};

use rustacean_docs_cache::MemoryCache;
use rustacean_docs_client::DocsClient;

use crate::config::Config;
use crate::tools::ToolHandler;

// Type alias for our specific cache implementation
type ServerCache = MemoryCache<String, Value>;

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
    pub fn new(config: Config) -> Result<Self> {
        config.validate()?;

        let client = Arc::new(DocsClient::new()?);
        let ttl = Duration::from_secs(config.cache.memory_ttl_secs);
        let cache = Arc::new(RwLock::new(MemoryCache::new(
            config.cache.memory_max_entries,
            ttl,
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

    pub fn with_default_config() -> Result<Self> {
        Self::new(Config::load()?)
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
        self.register_tool("get_crate_metadata", Box::new(crate::tools::CrateMetadataTool::new()))?;

        // Register cache management tools
        self.register_tool("get_cache_stats", Box::new(crate::tools::CacheStatsTool::new()))?;
        self.register_tool("clear_cache", Box::new(crate::tools::ClearCacheTool::new()))?;

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
            cache.stats().await
        };

        info!("Final cache statistics: {:?}", cache_stats);

        Ok(())
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::with_default_config().expect("Failed to create default MCP server")
    }
}

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
        let server = McpServer::new(config);
        assert!(server.is_ok());
    }

    #[tokio::test]
    async fn test_server_initialization() {
        let config = Config::default();
        let mut server = McpServer::new(config).unwrap();
        let result = server.initialize().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_tool_registration() {
        let config = Config::default();
        let mut server = McpServer::new(config).unwrap();

        let tool = Box::new(MockTool);
        let result = server.register_tool("test_tool", tool);
        assert!(result.is_ok());

        let tools = server.list_tools();
        assert!(tools.contains(&"test_tool".to_string()));
    }

    #[tokio::test]
    async fn test_duplicate_tool_registration() {
        let config = Config::default();
        let mut server = McpServer::new(config).unwrap();

        let tool1 = Box::new(MockTool);
        let tool2 = Box::new(MockTool);

        server.register_tool("test_tool", tool1).unwrap();
        let result = server.register_tool("test_tool", tool2);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tool_execution() {
        let config = Config::default();
        let mut server = McpServer::new(config).unwrap();

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
        let server = McpServer::new(config).unwrap();

        let params = serde_json::json!({});
        let result = server.handle_tool_call("unknown_tool", params).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_server_info() {
        let config = Config::default();
        let server = McpServer::new(config).unwrap();

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
        let server = McpServer::new(config).unwrap();

        let result = server.shutdown().await;
        assert!(result.is_ok());
    }
}
