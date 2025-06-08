use anyhow::Result;
use async_trait::async_trait;
use rust_mcp_sdk::{
    McpServer,
    schema::{
        CallToolRequest, CallToolResult, ListToolsRequest, ListToolsResult, 
        RpcError, Tool,
        schema_utils::CallToolError
    }
};
use rust_mcp_sdk::mcp_server::ServerHandler;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use rustacean_docs_cache::TieredCache;
use rustacean_docs_client::DocsClient;

use crate::config::Config;
use crate::tools::{
    SearchTool, CrateDocsTool, ItemDocsTool, CrateMetadataTool, 
    RecentReleasesTool, CacheStatsTool, ClearCacheTool, CacheMaintenanceTool,
    ToolHandler
};

type ServerCache = TieredCache<String, Value>;

pub struct ToolInfo {
    pub name: String,
    pub description: String,
}

pub struct RustaceanDocsHandler {
    client: Arc<DocsClient>,
    cache: Arc<RwLock<ServerCache>>,
    #[allow(dead_code)]
    config: Config,
}

impl RustaceanDocsHandler {
    pub async fn new(config: Config) -> Result<Self> {
        config.validate()?;

        let client = Arc::new(DocsClient::new()?);

        // Create cache directory if it doesn't exist
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("rustacean-docs");

        if !cache_dir.exists() {
            std::fs::create_dir_all(&cache_dir)?;
        }

        let memory_ttl = std::time::Duration::from_secs(config.cache.memory_ttl_secs);
        let disk_ttl = std::time::Duration::from_secs(config.cache.disk_ttl_secs);
        let disk_max_size = config.cache.disk_max_size_mb * 1024 * 1024; // Convert MB to bytes

        let cache = Arc::new(RwLock::new(
            TieredCache::new(
                config.cache.memory_max_entries,
                memory_ttl,
                cache_dir,
                disk_ttl,
                disk_max_size,
            )
            .await?,
        ));

        info!(
            "Initialized Rustacean Docs MCP Handler: {} v{}",
            config.server.name, config.server.version
        );

        Ok(Self {
            client,
            cache,
            config,
        })
    }

    fn create_tools(&self) -> Vec<Tool> {
        vec![
            Tool {
                name: "search_crate".to_string(),
                description: Some(SearchTool::new().description().to_string()),
                input_schema: serde_json::from_value(SearchTool::new().parameters_schema()).unwrap(),
                annotations: None,
            },
            Tool {
                name: "get_crate_docs".to_string(),
                description: Some(CrateDocsTool::new().description().to_string()),
                input_schema: serde_json::from_value(CrateDocsTool::new().parameters_schema()).unwrap(),
                annotations: None,
            },
            Tool {
                name: "get_item_docs".to_string(),
                description: Some(ItemDocsTool::new().description().to_string()),
                input_schema: serde_json::from_value(ItemDocsTool::new().parameters_schema()).unwrap(),
                annotations: None,
            },
            Tool {
                name: "get_crate_metadata".to_string(),
                description: Some(CrateMetadataTool::new().description().to_string()),
                input_schema: serde_json::from_value(CrateMetadataTool::new().parameters_schema()).unwrap(),
                annotations: None,
            },
            Tool {
                name: "list_recent_releases".to_string(),
                description: Some(RecentReleasesTool::new().description().to_string()),
                input_schema: serde_json::from_value(RecentReleasesTool::new().parameters_schema()).unwrap(),
                annotations: None,
            },
            Tool {
                name: "get_cache_stats".to_string(),
                description: Some(CacheStatsTool::new().description().to_string()),
                input_schema: serde_json::from_value(CacheStatsTool::new().parameters_schema()).unwrap(),
                annotations: None,
            },
            Tool {
                name: "clear_cache".to_string(),
                description: Some(ClearCacheTool::new().description().to_string()),
                input_schema: serde_json::from_value(ClearCacheTool::new().parameters_schema()).unwrap(),
                annotations: None,
            },
            Tool {
                name: "cache_maintenance".to_string(),
                description: Some(CacheMaintenanceTool::new().description().to_string()),
                input_schema: serde_json::from_value(CacheMaintenanceTool::new().parameters_schema()).unwrap(),
                annotations: None,
            },
        ]
    }

    pub async fn execute_tool_directly(&self, tool_name: &str, params: Value) -> Result<Value> {
        self.execute_tool(tool_name, params).await
    }

    pub fn get_available_tools(&self) -> Vec<ToolInfo> {
        vec![
            ToolInfo {
                name: "search_crate".to_string(),
                description: SearchTool::new().description().to_string(),
            },
            ToolInfo {
                name: "get_crate_docs".to_string(),
                description: CrateDocsTool::new().description().to_string(),
            },
            ToolInfo {
                name: "get_item_docs".to_string(),
                description: ItemDocsTool::new().description().to_string(),
            },
            ToolInfo {
                name: "get_crate_metadata".to_string(),
                description: CrateMetadataTool::new().description().to_string(),
            },
            ToolInfo {
                name: "list_recent_releases".to_string(),
                description: RecentReleasesTool::new().description().to_string(),
            },
            ToolInfo {
                name: "get_cache_stats".to_string(),
                description: CacheStatsTool::new().description().to_string(),
            },
            ToolInfo {
                name: "clear_cache".to_string(),
                description: ClearCacheTool::new().description().to_string(),
            },
            ToolInfo {
                name: "cache_maintenance".to_string(),
                description: CacheMaintenanceTool::new().description().to_string(),
            },
        ]
    }

    pub fn get_tool_schema(&self, tool_name: &str) -> Result<Value> {
        let schema = match tool_name {
            "search_crate" => SearchTool::new().parameters_schema(),
            "get_crate_docs" => CrateDocsTool::new().parameters_schema(),
            "get_item_docs" => ItemDocsTool::new().parameters_schema(),
            "get_crate_metadata" => CrateMetadataTool::new().parameters_schema(),
            "list_recent_releases" => RecentReleasesTool::new().parameters_schema(),
            "get_cache_stats" => CacheStatsTool::new().parameters_schema(),
            "clear_cache" => ClearCacheTool::new().parameters_schema(),
            "cache_maintenance" => CacheMaintenanceTool::new().parameters_schema(),
            _ => return Err(anyhow::anyhow!("Unknown tool: {}", tool_name)),
        };
        Ok(schema)
    }

    async fn execute_tool(&self, tool_name: &str, params: Value) -> Result<Value> {
        let result = match tool_name {
            "search_crate" => {
                SearchTool::new().execute(params, &self.client, &self.cache).await
            }
            "get_crate_docs" => {
                CrateDocsTool::new().execute(params, &self.client, &self.cache).await
            }
            "get_item_docs" => {
                ItemDocsTool::new().execute(params, &self.client, &self.cache).await
            }
            "get_crate_metadata" => {
                CrateMetadataTool::new().execute(params, &self.client, &self.cache).await
            }
            "list_recent_releases" => {
                RecentReleasesTool::new().execute(params, &self.client, &self.cache).await
            }
            "get_cache_stats" => {
                CacheStatsTool::new().execute(params, &self.client, &self.cache).await
            }
            "clear_cache" => {
                ClearCacheTool::new().execute(params, &self.client, &self.cache).await
            }
            "cache_maintenance" => {
                CacheMaintenanceTool::new().execute(params, &self.client, &self.cache).await
            }
            _ => {
                return Err(anyhow::anyhow!("Unknown tool: {}", tool_name));
            }
        };

        result
    }
}

#[async_trait]
impl ServerHandler for RustaceanDocsHandler {
    async fn handle_list_tools_request(
        &self,
        _request: ListToolsRequest,
        _runtime: &dyn McpServer,
    ) -> Result<ListToolsResult, RpcError> {
        debug!("Handling list_tools request");

        let tools = self.create_tools();
        info!("Listed {} available tools", tools.len());

        Ok(ListToolsResult {
            tools,
            meta: None,
            next_cursor: None,
        })
    }

    async fn handle_call_tool_request(
        &self,
        request: CallToolRequest,
        _runtime: &dyn McpServer,
    ) -> Result<CallToolResult, CallToolError> {
        debug!("Handling call_tool request for: {}", request.params.name);

        // Extract the actual tool arguments from the request
        let params = match &request.params.arguments {
            Some(args_map) => serde_json::Value::Object(args_map.clone()),
            None => serde_json::Value::Object(serde_json::Map::new()),
        };
        
        debug!("Executing tool: {} with params: {}", request.params.name, params);
        
        match self.execute_tool(&request.params.name, params).await {
            Ok(result) => {
                debug!("Tool execution successful for: {}", request.params.name);
                Ok(CallToolResult::text_content(
                    serde_json::to_string_pretty(&result).map_err(|e| {
                        error!("Failed to serialize tool result: {}", e);
                        CallToolError::new(std::io::Error::new(std::io::ErrorKind::Other, format!("Serialization error: {}", e)))
                    })?,
                    None,
                ))
            }
            Err(e) => {
                error!("Tool execution failed for {}: {}", request.params.name, e);
                Err(CallToolError::new(std::io::Error::new(std::io::ErrorKind::Other, format!("Tool execution error: {}", e))))
            }
        }
    }
}