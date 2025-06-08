use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

use rustacean_docs_cache::TieredCache;
use rustacean_docs_client::DocsClient;
use rustacean_docs_core::Error;

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

/// Shared trait for tool input parameters providing validation and cache key generation
pub trait ToolInput: Serialize + for<'de> Deserialize<'de> + Send + Sync {
    /// Validate the input parameters
    fn validate(&self) -> Result<(), Error>;
    
    /// Generate cache key for this input
    fn cache_key(&self, tool_name: &str) -> String;
}

/// Shared parameter validation utilities
pub struct ParameterValidator;

impl ParameterValidator {
    /// Validate crate name format
    pub fn validate_crate_name(name: &str, tool_name: &str) -> Result<(), Error> {
        if name.trim().is_empty() {
            return Err(Error::invalid_input(
                tool_name,
                "crate_name cannot be empty",
            ));
        }

        // Basic crate name validation - should contain only valid characters
        if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Err(Error::invalid_input(
                tool_name,
                "crate_name contains invalid characters",
            ));
        }

        Ok(())
    }

    /// Validate version format if provided
    pub fn validate_version(version: &Option<String>, tool_name: &str) -> Result<(), Error> {
        if let Some(ref version) = version {
            if version.trim().is_empty() {
                return Err(Error::invalid_input(
                    tool_name,
                    "version cannot be empty string",
                ));
            }
        }
        Ok(())
    }

    /// Validate search query
    pub fn validate_query(query: &str, tool_name: &str) -> Result<(), Error> {
        if query.trim().is_empty() {
            return Err(Error::invalid_input(
                tool_name,
                "query cannot be empty",
            ));
        }
        Ok(())
    }

    /// Validate limit parameter
    pub fn validate_limit(limit: &Option<usize>, tool_name: &str, max_limit: usize) -> Result<(), Error> {
        if let Some(limit) = limit {
            if *limit == 0 {
                return Err(Error::invalid_input(
                    tool_name,
                    "limit must be greater than 0",
                ));
            }
            if *limit > max_limit {
                return Err(Error::invalid_input(
                    tool_name,
                    &format!("limit cannot exceed {} for performance reasons", max_limit),
                ));
            }
        }
        Ok(())
    }
}

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
