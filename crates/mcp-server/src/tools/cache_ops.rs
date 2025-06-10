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
        let hit_rate = stats.hit_rate();
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
        let hit_rate = stats.hit_rate();
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
        let hit_rate = stats.hit_rate();
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
            "required": []
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
            "required": []
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
            "required": []
        })
    }
}