use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use rustacean_docs_cache::{Cache, CacheStats, TieredCache};
use rustacean_docs_client::DocsClient;

use crate::tools::ToolHandler;

// Type alias for our specific cache implementation
type ServerCache = TieredCache<String, Value>;

/// Tool for retrieving comprehensive cache statistics
pub struct CacheStatsTool;

impl CacheStatsTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CacheStatsTool {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheStatsTool {
    /// Format cache statistics into a comprehensive response
    fn format_cache_stats(&self, stats: &CacheStats) -> Value {
        json!({
            "summary": {
                "total_requests": stats.hits + stats.misses,
                "total_hits": stats.hits,
                "total_misses": stats.misses,
                "hit_rate": format!("{:.2}%", stats.hit_rate()),
                "performance_tier": self.assess_performance(stats)
            },
            "cache": {
                "entries": stats.size,
                "capacity": stats.capacity,
                "utilization": format!("{:.1}%", stats.utilization()),
                "hits": stats.hits,
                "misses": stats.misses,
                "hit_rate": format!("{:.2}%", stats.hit_rate())
            },
            "analysis": {
                "efficiency": self.get_efficiency_analysis(stats),
                "recommendations": self.get_recommendations(stats)
            }
        })
    }

    /// Assess overall cache performance
    fn assess_performance(&self, stats: &CacheStats) -> &'static str {
        let hit_rate = stats.hit_rate() * 100.0; // Convert to percentage
        match hit_rate {
            rate if rate >= 80.0 => "Excellent",
            rate if rate >= 60.0 => "Good",
            rate if rate >= 40.0 => "Fair",
            rate if rate >= 20.0 => "Poor",
            _ => "Critical",
        }
    }

    /// Get efficiency analysis
    fn get_efficiency_analysis(&self, stats: &CacheStats) -> Value {
        let hit_rate = stats.hit_rate() * 100.0; // Convert to percentage
        let utilization = stats.utilization();

        json!({
            "hit_rate_category": match hit_rate {
                rate if rate >= 80.0 => "excellent",
                rate if rate >= 60.0 => "good",
                rate if rate >= 40.0 => "moderate",
                _ => "poor"
            },
            "utilization_category": match utilization {
                util if util >= 90.0 => "high",
                util if util >= 70.0 => "moderate",
                util if util >= 30.0 => "low",
                _ => "very_low"
            },
            "total_operations": stats.hits + stats.misses
        })
    }

    /// Get recommendations for cache optimization
    fn get_recommendations(&self, stats: &CacheStats) -> Vec<&'static str> {
        let mut recommendations = Vec::new();
        let hit_rate = stats.hit_rate() * 100.0; // Convert to percentage
        let utilization = stats.utilization();

        if hit_rate < 50.0 {
            recommendations.push("Consider implementing smarter caching strategies");
        }

        if utilization > 95.0 {
            recommendations.push("Cache is near capacity - consider increasing size");
        } else if utilization < 20.0 && stats.size > 0 {
            recommendations.push("Cache utilization is low - consider reducing size");
        }

        if stats.hits + stats.misses < 100 {
            recommendations.push("Cache needs more usage to provide meaningful statistics");
        }

        if recommendations.is_empty() {
            recommendations.push("Cache performance is optimal");
        }

        recommendations
    }
}

#[async_trait]
impl ToolHandler for CacheStatsTool {
    async fn execute(
        &self,
        _params: Value,
        _client: &Arc<DocsClient>,
        cache: &Arc<RwLock<ServerCache>>,
    ) -> Result<Value> {
        debug!("Retrieving cache statistics");

        let cache_guard = cache.read().await;
        let stats = cache_guard.stats();
        drop(cache_guard);

        let formatted_stats = self.format_cache_stats(&stats);

        info!(
            total_requests = stats.hits + stats.misses,
            hit_rate = %format!("{:.2}%", stats.hit_rate()),
            utilization = %format!("{:.1}%", stats.utilization()),
            "Cache statistics retrieved"
        );

        Ok(formatted_stats)
    }

    fn description(&self) -> &str {
        "Get comprehensive cache statistics including hit rates, utilization, and performance analysis"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": [],
            "additionalProperties": false
        })
    }
}

/// Tool for clearing cache data
pub struct ClearCacheTool;

impl ClearCacheTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ClearCacheTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolHandler for ClearCacheTool {
    async fn execute(
        &self,
        _params: Value,
        _client: &Arc<DocsClient>,
        cache: &Arc<RwLock<ServerCache>>,
    ) -> Result<Value> {
        debug!("Clearing cache");

        let cache_guard = cache.write().await;
        let stats_before = cache_guard.stats();

        cache_guard.clear().await?;

        let _stats_after = cache_guard.stats();
        drop(cache_guard);

        info!(
            items_cleared = stats_before.size,
            "Cache cleared successfully"
        );

        Ok(json!({
            "success": true,
            "items_cleared": stats_before.size,
            "message": format!("Successfully cleared {} items from cache", stats_before.size)
        }))
    }

    fn description(&self) -> &str {
        "Clear all cached data from the cache"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": [],
            "additionalProperties": false
        })
    }
}

/// Tool for getting cache configuration and basic info
pub struct CacheInfoTool;

impl CacheInfoTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CacheInfoTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolHandler for CacheInfoTool {
    async fn execute(
        &self,
        _params: Value,
        _client: &Arc<DocsClient>,
        cache: &Arc<RwLock<ServerCache>>,
    ) -> Result<Value> {
        debug!("Retrieving cache information");

        let cache_guard = cache.read().await;
        let stats = cache_guard.stats();
        drop(cache_guard);

        Ok(json!({
            "cache_type": "TieredCache",
            "current_size": stats.size,
            "capacity": stats.capacity,
            "utilization_percent": stats.utilization(),
            "hit_rate_percent": stats.hit_rate(),
            "total_operations": stats.hits + stats.misses,
            "status": if stats.size == 0 { "empty" } else { "active" }
        }))
    }

    fn description(&self) -> &str {
        "Get basic cache configuration and status information"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": [],
            "additionalProperties": false
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    async fn create_test_cache() -> Arc<RwLock<ServerCache>> {
        let cache = ServerCache::new(vec![], rustacean_docs_cache::WriteStrategy::WriteThrough);
        Arc::new(RwLock::new(cache))
    }

    #[test]
    fn test_cache_stats_tool_creation() {
        let tool = CacheStatsTool::new();
        assert!(!tool.description().is_empty());
        assert!(tool.parameters_schema().is_object());
    }

    #[test]
    fn test_clear_cache_tool_creation() {
        let tool = ClearCacheTool::new();
        assert!(!tool.description().is_empty());
        assert!(tool.parameters_schema().is_object());
    }

    #[test]
    fn test_cache_info_tool_creation() {
        let tool = CacheInfoTool::new();
        assert!(!tool.description().is_empty());
        assert!(tool.parameters_schema().is_object());
    }

    #[test]
    fn test_tool_descriptions() {
        let stats_tool = CacheStatsTool::new();
        let clear_tool = ClearCacheTool::new();
        let info_tool = CacheInfoTool::new();

        assert!(stats_tool.description().contains("cache statistics"));
        assert!(clear_tool.description().contains("Clear"));
        assert!(info_tool.description().contains("cache configuration"));
    }

    #[test]
    fn test_cache_stats_performance_assessment() {
        let tool = CacheStatsTool::new();

        // Test different performance levels
        let excellent_stats = rustacean_docs_cache::CacheStats {
            hits: 90,
            misses: 10,
            size: 50,
            capacity: 100,
        };
        assert_eq!(tool.assess_performance(&excellent_stats), "Excellent");

        let poor_stats = rustacean_docs_cache::CacheStats {
            hits: 20,
            misses: 80,
            size: 10,
            capacity: 100,
        };
        assert_eq!(tool.assess_performance(&poor_stats), "Poor");
    }

    #[tokio::test]
    async fn test_cache_stats_tool_execution() {
        let tool = CacheStatsTool::new();
        let client = Arc::new(DocsClient::new().unwrap());
        let cache = create_test_cache().await;

        let params = json!({});
        let result = tool.execute(params, &client, &cache).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response["summary"].is_object());
        assert!(response["cache"].is_object());
        assert!(response["analysis"].is_object());
    }

    #[tokio::test]
    async fn test_clear_cache_tool_execution() {
        let tool = ClearCacheTool::new();
        let client = Arc::new(DocsClient::new().unwrap());
        let cache = create_test_cache().await;

        let params = json!({});
        let result = tool.execute(params, &client, &cache).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response["success"], true);
        assert!(response["message"].is_string());
    }

    #[tokio::test]
    async fn test_cache_info_tool_execution() {
        let tool = CacheInfoTool::new();
        let client = Arc::new(DocsClient::new().unwrap());
        let cache = create_test_cache().await;

        let params = json!({});
        let result = tool.execute(params, &client, &cache).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response["cache_type"], "TieredCache");
        assert!(response["current_size"].is_number());
        assert!(response["capacity"].is_number());
        assert!(response["status"].is_string());
    }

    #[test]
    fn test_parameters_schemas() {
        let stats_tool = CacheStatsTool::new();
        let clear_tool = ClearCacheTool::new();
        let info_tool = CacheInfoTool::new();

        // Test each tool's schema individually
        let stats_schema = stats_tool.parameters_schema();
        assert_eq!(stats_schema["type"], "object");
        assert!(stats_schema["properties"].is_object());
        assert_eq!(stats_schema["required"].as_array().unwrap().len(), 0);

        let clear_schema = clear_tool.parameters_schema();
        assert_eq!(clear_schema["type"], "object");
        assert!(clear_schema["properties"].is_object());
        assert_eq!(clear_schema["required"].as_array().unwrap().len(), 0);

        let info_schema = info_tool.parameters_schema();
        assert_eq!(info_schema["type"], "object");
        assert!(info_schema["properties"].is_object());
        assert_eq!(info_schema["required"].as_array().unwrap().len(), 0);
    }
}
