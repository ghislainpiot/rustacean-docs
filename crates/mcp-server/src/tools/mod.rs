use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

use rustacean_docs_cache::TieredCache;
use rustacean_docs_client::DocsClient;

pub mod cache_ops;
pub mod crate_docs;
pub mod item_docs;
pub mod metadata;
pub mod releases;
pub mod search;

// Re-export tools for convenience
pub use cache_ops::{CacheMaintenanceTool, CacheStatsTool, ClearCacheTool};
pub use crate_docs::CrateDocsTool;
pub use item_docs::ItemDocsTool;
pub use metadata::CrateMetadataTool;
pub use releases::RecentReleasesTool;
pub use search::SearchTool;

// Type alias for our specific cache implementation
type ServerCache = TieredCache<String, Value>;

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
