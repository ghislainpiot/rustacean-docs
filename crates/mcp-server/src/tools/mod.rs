use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

use rustacean_docs_cache::MemoryCache;
use rustacean_docs_client::DocsClient;

// Type alias for our specific cache implementation
type ServerCache = MemoryCache<String, Value>;

#[async_trait::async_trait]
pub trait ToolHandler: Send + Sync {
    async fn execute(
        &self,
        params: Value,
        client: &Arc<DocsClient>,
        cache: &Arc<RwLock<ServerCache>>,
    ) -> Result<Value>;

    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Value;
}
