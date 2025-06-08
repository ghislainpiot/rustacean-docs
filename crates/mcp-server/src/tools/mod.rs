use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, trace};

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

/// Cache configuration for tools
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Whether caching is enabled for this tool
    pub enabled: bool,
    /// Custom cache key prefix (defaults to tool name)
    pub key_prefix: Option<String>,
    /// Whether to cache responses even on errors (usually false)
    pub cache_errors: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            key_prefix: None,
            cache_errors: false,
        }
    }
}

impl CacheConfig {
    /// Create cache config with caching disabled
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            key_prefix: None,
            cache_errors: false,
        }
    }

    /// Create cache config with custom key prefix
    pub fn with_prefix(prefix: impl Into<String>) -> Self {
        Self {
            enabled: true,
            key_prefix: Some(prefix.into()),
            cache_errors: false,
        }
    }
}

/// Client factory for creating clients when ownership is needed
pub struct ClientFactory;

impl ClientFactory {
    /// Create a new DocsClient instance for services that need ownership
    /// This is used for services like MetadataService and ReleasesService
    /// that take ownership of the client rather than borrowing it.
    pub fn create_owned_client() -> Result<DocsClient> {
        DocsClient::new().map_err(|e| anyhow::anyhow!("{}: {}", ErrorHandler::client_creation_context(), e))
    }
}

/// Standard response metadata
#[derive(Debug, Clone, Serialize)]
pub struct ResponseMetadata {
    /// Tool that generated the response
    pub tool: String,
    /// Request timestamp
    pub timestamp: String,
    /// Whether the response was served from cache
    pub cached: bool,
    /// Optional request identifier
    pub request_id: Option<String>,
}

impl ResponseMetadata {
    pub fn new(tool: &str, cached: bool) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();
        
        Self {
            tool: tool.to_string(),
            timestamp,
            cached,
            request_id: None,
        }
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }
}

/// Common response builder for consistent API responses
pub struct ResponseBuilder;

impl ResponseBuilder {
    /// Create a success response with data
    pub fn success<T: Serialize>(data: T) -> Value {
        serde_json::json!({
            "data": data
        })
    }

    /// Create a success response with data and metadata
    pub fn success_with_meta<T: Serialize>(data: T, meta: ResponseMetadata) -> Value {
        serde_json::json!({
            "data": data,
            "meta": meta
        })
    }

    /// Create a paginated response
    pub fn paginated<T: Serialize>(
        items: Vec<T>,
        total: Option<usize>,
        page: Option<usize>,
        limit: Option<usize>,
    ) -> Value {
        serde_json::json!({
            "data": {
                "items": items,
                "pagination": {
                    "total": total,
                    "returned": items.len(),
                    "page": page,
                    "limit": limit
                }
            }
        })
    }

    /// Create a legacy format response (for backward compatibility)
    pub fn legacy<T: Serialize>(data: T) -> Value {
        serde_json::to_value(data).unwrap_or_else(|_| Value::Null)
    }

    /// Create an error response
    pub fn error(message: &str, code: Option<&str>) -> Value {
        serde_json::json!({
            "error": {
                "message": message,
                "code": code
            }
        })
    }
}

/// Common error handling utilities for tools
pub struct ErrorHandler;

impl ErrorHandler {
    /// Add context for parameter parsing errors
    pub fn parameter_parsing_context(tool_name: &str) -> String {
        format!("Invalid input parameters for {}", tool_name)
    }

    /// Add context for API call failures
    pub fn api_call_context(operation: &str, target: &str) -> String {
        format!("Failed to {} for {}", operation, target)
    }

    /// Add context for crate-specific operations
    pub fn crate_operation_context(operation: &str, crate_name: &str, version: Option<&str>) -> String {
        match version {
            Some(v) => format!("Failed to {} for crate: {} version: {}", operation, crate_name, v),
            None => format!("Failed to {} for crate: {}", operation, crate_name),
        }
    }

    /// Add context for search operations
    pub fn search_context(query: &str) -> String {
        format!("Failed to search for crates with query: {}", query)
    }

    /// Add context for client creation errors
    pub fn client_creation_context() -> String {
        "Failed to create HTTP client".to_string()
    }

    /// Add context for cache operations
    pub fn cache_operation_context(operation: &str, key: &str) -> String {
        format!("Cache {} failed for key: {}", operation, key)
    }
}

/// Extension trait to add tool-specific error context
pub trait ToolErrorContext<T> {
    /// Add tool operation context to an error
    fn tool_context(self, tool_name: &str, operation: &str) -> Result<T>;
    
    /// Add crate operation context to an error
    fn crate_context(self, operation: &str, crate_name: &str, version: Option<&str>) -> Result<T>;
    
    /// Add search context to an error
    fn search_context(self, query: &str) -> Result<T>;
}

impl<T, E> ToolErrorContext<T> for std::result::Result<T, E>
where
    E: std::fmt::Display + Send + Sync + 'static,
{
    fn tool_context(self, tool_name: &str, operation: &str) -> Result<T> {
        self.map_err(|e| anyhow::anyhow!("{} in {}: {}", operation, tool_name, e))
    }

    fn crate_context(self, operation: &str, crate_name: &str, version: Option<&str>) -> Result<T> {
        self.map_err(|e| {
            anyhow::anyhow!("{}: {}", ErrorHandler::crate_operation_context(operation, crate_name, version), e)
        })
    }

    fn search_context(self, query: &str) -> Result<T> {
        self.map_err(|e| {
            anyhow::anyhow!("{}: {}", ErrorHandler::search_context(query), e)
        })
    }
}

/// Unified cache execution strategy for tools
pub struct CacheStrategy;

impl CacheStrategy {
    /// Execute a tool with unified caching strategy
    pub async fn execute_with_cache<F, Fut, I>(
        tool_name: &str,
        _params: Value,
        input: I,
        cache_config: CacheConfig,
        client: &Arc<DocsClient>,
        cache: &Arc<RwLock<ServerCache>>,
        operation: F,
    ) -> Result<Value>
    where
        F: FnOnce(I, Arc<DocsClient>) -> Fut + Send,
        Fut: std::future::Future<Output = Result<Value>> + Send,
        I: ToolInput,
    {
        // Validate input
        input.validate()?;

        // Skip cache if disabled
        if !cache_config.enabled {
            trace!(tool = tool_name, "Cache disabled, executing directly");
            return operation(input, client.clone()).await;
        }

        let cache_key = if let Some(prefix) = &cache_config.key_prefix {
            input.cache_key(prefix)
        } else {
            input.cache_key(tool_name)
        };

        // Try to get from cache first
        {
            let cache_guard = cache.read().await;
            if let Ok(Some(cached_result)) = cache_guard.get(&cache_key).await {
                trace!(
                    tool = tool_name,
                    cache_key = %cache_key,
                    "Cache hit"
                );
                return Ok(cached_result);
            }
        }

        trace!(
            tool = tool_name,
            cache_key = %cache_key,
            "Cache miss, executing operation"
        );

        // Cache miss - execute operation
        let result = operation(input, client.clone()).await;

        // Store in cache if successful (or if cache_errors is enabled)
        match &result {
            Ok(response) => {
                let cache_guard = cache.read().await;
                if let Err(e) = cache_guard
                    .insert(cache_key.clone(), response.clone())
                    .await
                {
                    debug!(
                        tool = tool_name,
                        cache_key = %cache_key,
                        error = %e,
                        "Failed to cache result"
                    );
                }
                trace!(
                    tool = tool_name,
                    cache_key = %cache_key,
                    "Result cached"
                );
            }
            Err(e) if cache_config.cache_errors => {
                // Optionally cache errors as well (usually not desired)
                debug!(
                    tool = tool_name,
                    cache_key = %cache_key,
                    error = %e,
                    "Caching error result"
                );
            }
            Err(_) => {
                // Don't cache errors by default
            }
        }

        result
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
