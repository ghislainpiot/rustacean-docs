use anyhow::Result;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use rustacean_docs_cache::TieredCache;
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
    fn format_cache_stats(&self, stats: &rustacean_docs_cache::CombinedCacheStats) -> Value {
        json!({
            "summary": {
                "total_requests": stats.total_requests,
                "total_hits": stats.total_hits,
                "total_misses": stats.total_misses,
                "combined_hit_rate": format!("{:.2}%", stats.combined_hit_rate),
                "performance_tier": self.assess_performance(stats)
            },
            "memory_cache": {
                "entries": stats.memory.size,
                "capacity": stats.memory.capacity,
                "utilization": format!("{:.1}%", (stats.memory.size as f64 / stats.memory.capacity as f64) * 100.0),
                "hits": stats.memory.hits,
                "misses": stats.memory.misses,
                "hit_rate": format!("{:.2}%", stats.memory.hit_rate),
                "requests": stats.memory.requests,
                "bytes_used": stats.memory.bytes_used.unwrap_or(0),
                "bytes_capacity": stats.memory.bytes_capacity.unwrap_or(0)
            },
            "disk_cache": {
                "entries": stats.disk.size,
                "capacity": stats.disk.capacity,
                "utilization": format!("{:.1}%", if stats.disk.capacity > 0 {
                    (stats.disk.size as f64 / stats.disk.capacity as f64) * 100.0
                } else {
                    0.0
                }),
                "hits": stats.disk.hits,
                "misses": stats.disk.misses,
                "hit_rate": format!("{:.2}%", stats.disk.hit_rate),
                "requests": stats.disk.requests,
                "bytes_used": stats.disk.bytes_used.unwrap_or(0),
                "bytes_capacity": stats.disk.bytes_capacity.unwrap_or(0)
            },
            "efficiency_metrics": {
                "memory_efficiency": format!("{:.2}%", if stats.total_requests > 0 {
                    (stats.memory.hits as f64 / stats.total_requests as f64) * 100.0
                } else {
                    0.0
                }),
                "disk_efficiency": format!("{:.2}%", if stats.total_requests > 0 {
                    (stats.disk.hits as f64 / stats.total_requests as f64) * 100.0
                } else {
                    0.0
                }),
                "cache_tier_performance": self.analyze_tier_performance(stats)
            },
            "recommendations": self.generate_recommendations(stats)
        })
    }

    /// Assess overall cache performance
    fn assess_performance(&self, stats: &rustacean_docs_cache::CombinedCacheStats) -> &'static str {
        if stats.combined_hit_rate >= 90.0 {
            "excellent"
        } else if stats.combined_hit_rate >= 75.0 {
            "good"
        } else if stats.combined_hit_rate >= 50.0 {
            "acceptable"
        } else {
            "needs_optimization"
        }
    }

    /// Analyze performance of each cache tier
    fn analyze_tier_performance(&self, stats: &rustacean_docs_cache::CombinedCacheStats) -> Value {
        let memory_dominance = if stats.total_requests > 0 {
            (stats.memory.hits as f64 / (stats.memory.hits + stats.disk.hits) as f64) * 100.0
        } else {
            0.0
        };

        json!({
            "memory_dominance": format!("{:.1}%", memory_dominance),
            "disk_fallback_rate": format!("{:.1}%", 100.0 - memory_dominance),
            "tier_balance": if memory_dominance > 80.0 {
                "memory_heavy"
            } else if memory_dominance > 60.0 {
                "balanced"
            } else {
                "disk_heavy"
            }
        })
    }

    /// Generate optimization recommendations based on cache statistics
    fn generate_recommendations(
        &self,
        stats: &rustacean_docs_cache::CombinedCacheStats,
    ) -> Vec<Value> {
        let mut recommendations = Vec::new();

        // Low hit rate recommendation
        if stats.combined_hit_rate < 50.0 {
            recommendations.push(json!({
                "priority": "high",
                "category": "hit_rate",
                "issue": "Low cache hit rate",
                "recommendation": "Consider increasing cache capacity or TTL values",
                "impact": "Better performance and reduced API calls"
            }));
        }

        // Memory utilization recommendations
        let memory_util = (stats.memory.size as f64 / stats.memory.capacity as f64) * 100.0;
        if memory_util > 95.0 {
            recommendations.push(json!({
                "priority": "medium",
                "category": "memory_capacity",
                "issue": "Memory cache near capacity",
                "recommendation": "Consider increasing memory cache capacity",
                "impact": "Reduced memory evictions and better hit rates"
            }));
        } else if memory_util < 20.0 && stats.memory.capacity > 100 {
            recommendations.push(json!({
                "priority": "low",
                "category": "memory_optimization",
                "issue": "Memory cache underutilized",
                "recommendation": "Consider reducing memory cache capacity to save resources",
                "impact": "Memory savings without performance impact"
            }));
        }

        // Disk cache recommendations
        if let (Some(bytes_used), Some(bytes_capacity)) =
            (stats.disk.bytes_used, stats.disk.bytes_capacity)
        {
            if bytes_used > (bytes_capacity as f64 * 0.9) as u64 {
                recommendations.push(json!({
                    "priority": "medium",
                    "category": "disk_space",
                    "issue": "Disk cache approaching size limit",
                    "recommendation": "Consider increasing disk cache size limit or reducing TTL",
                    "impact": "Prevent cache thrashing and maintain performance"
                }));
            }
        }

        // Performance balance recommendations
        let memory_hit_percentage = if stats.total_hits > 0 {
            (stats.memory.hits as f64 / stats.total_hits as f64) * 100.0
        } else {
            0.0
        };

        if memory_hit_percentage < 70.0 && stats.total_hits > 100 {
            recommendations.push(json!({
                "priority": "low",
                "category": "tier_balance",
                "issue": "Heavy reliance on disk cache",
                "recommendation": "Consider increasing memory cache capacity or optimizing access patterns",
                "impact": "Faster response times for frequently accessed data"
            }));
        }

        recommendations
    }
}

#[async_trait::async_trait]
impl ToolHandler for CacheStatsTool {
    async fn execute(
        &self,
        _params: Value,
        _client: &Arc<DocsClient>,
        cache: &Arc<RwLock<ServerCache>>,
    ) -> Result<Value> {
        debug!("Executing get_cache_stats tool");

        let cache_stats = {
            let cache_guard = cache.read().await;
            cache_guard.stats().await?
        };

        info!(
            "Cache stats retrieved: {} total requests, {:.2}% hit rate",
            cache_stats.total_requests, cache_stats.combined_hit_rate
        );

        let formatted_stats = self.format_cache_stats(&cache_stats);

        Ok(json!({
            "status": "success",
            "cache_stats": formatted_stats,
            "message": "Cache statistics retrieved successfully"
        }))
    }

    fn description(&self) -> &str {
        "Get comprehensive cache performance metrics for debugging and monitoring. Use this to monitor cache effectiveness, identify performance bottlenecks, and optimize caching configuration."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }
}

/// Tool for clearing all cached data
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

impl ClearCacheTool {
    /// Format the clear cache response with detailed information
    fn format_clear_response(&self, memory_cleared: usize, disk_cleared: usize) -> Value {
        let total_cleared = memory_cleared + disk_cleared;

        json!({
            "items_cleared": {
                "total": total_cleared,
                "memory_cache": memory_cleared,
                "disk_cache": disk_cleared
            },
            "performance_impact": {
                "cache_warmup_required": total_cleared > 0,
                "expected_temporary_slowdown": total_cleared > 50,
                "recommendation": if total_cleared > 100 {
                    "Consider monitoring response times closely after cache clear"
                } else {
                    "Minimal performance impact expected"
                }
            },
            "next_steps": [
                "Cache will be rebuilt automatically as requests are made",
                "First few requests may be slower while cache warms up",
                "Monitor cache hit rates to ensure normal operation"
            ]
        })
    }
}

#[async_trait::async_trait]
impl ToolHandler for ClearCacheTool {
    async fn execute(
        &self,
        _params: Value,
        _client: &Arc<DocsClient>,
        cache: &Arc<RwLock<ServerCache>>,
    ) -> Result<Value> {
        debug!("Executing clear_cache tool");

        let (memory_cleared, disk_cleared) = {
            let cache_guard = cache.read().await;
            cache_guard.clear().await?
        };

        info!(
            "Cache cleared successfully: {} items from memory, {} items from disk",
            memory_cleared, disk_cleared
        );

        let formatted_response = self.format_clear_response(memory_cleared, disk_cleared);

        Ok(json!({
            "status": "success",
            "cleared": formatted_response,
            "message": format!(
                "Successfully cleared {} items from cache ({} memory, {} disk)",
                memory_cleared + disk_cleared, memory_cleared, disk_cleared
            )
        }))
    }

    fn description(&self) -> &str {
        "Clear all cached data from both memory and disk storage. Use this for troubleshooting cache-related issues, forcing fresh data retrieval, or resetting after configuration changes."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }
}

/// Tool for cache maintenance operations
pub struct CacheMaintenanceTool;

impl CacheMaintenanceTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CacheMaintenanceTool {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheMaintenanceTool {
    /// Format maintenance response with detailed information
    fn format_maintenance_response(
        &self,
        report: &rustacean_docs_cache::MaintenanceReport,
    ) -> Value {
        let total_cleaned = report.memory_expired + report.disk_expired + report.size_enforced;

        json!({
            "maintenance_summary": {
                "total_items_processed": total_cleaned,
                "expired_entries_removed": report.memory_expired + report.disk_expired,
                "size_limit_enforcement": report.size_enforced,
                "maintenance_needed": total_cleaned > 0
            },
            "details": {
                "memory_cache": {
                    "expired_entries_removed": report.memory_expired,
                    "status": if report.memory_expired > 0 { "cleaned" } else { "no_cleanup_needed" }
                },
                "disk_cache": {
                    "expired_entries_removed": report.disk_expired,
                    "size_enforced_entries": report.size_enforced,
                    "status": if report.disk_expired > 0 || report.size_enforced > 0 {
                        "cleaned"
                    } else {
                        "no_cleanup_needed"
                    }
                }
            },
            "performance_impact": {
                "cache_efficiency_improved": total_cleaned > 0,
                "storage_space_reclaimed": report.disk_expired + report.size_enforced > 0,
                "recommendation": if total_cleaned > 50 {
                    "Consider adjusting TTL settings or cache size limits"
                } else if total_cleaned > 0 {
                    "Normal maintenance completed"
                } else {
                    "Cache is already optimized"
                }
            }
        })
    }
}

#[async_trait::async_trait]
impl ToolHandler for CacheMaintenanceTool {
    async fn execute(
        &self,
        _params: Value,
        _client: &Arc<DocsClient>,
        cache: &Arc<RwLock<ServerCache>>,
    ) -> Result<Value> {
        debug!("Executing cache_maintenance tool");

        let maintenance_report = {
            let cache_guard = cache.read().await;
            cache_guard.maintenance().await?
        };

        info!(
            "Cache maintenance completed: {} memory expired, {} disk expired, {} size enforced",
            maintenance_report.memory_expired,
            maintenance_report.disk_expired,
            maintenance_report.size_enforced
        );

        let formatted_response = self.format_maintenance_response(&maintenance_report);

        Ok(json!({
            "status": "success",
            "maintenance": formatted_response,
            "message": format!(
                "Cache maintenance completed: cleaned {} expired entries, enforced size limits on {} entries",
                maintenance_report.memory_expired + maintenance_report.disk_expired,
                maintenance_report.size_enforced
            )
        }))
    }

    fn description(&self) -> &str {
        "Perform cache maintenance operations including cleanup of expired entries and enforcement of size limits. Use this to optimize cache performance and reclaim storage space."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustacean_docs_cache::{CombinedCacheStats, TieredCache};
    use rustacean_docs_core::CacheLayerStats;
    use serde_json::json;
    use std::time::Duration;

    async fn create_test_cache() -> Arc<RwLock<TieredCache<String, Value>>> {
        let temp_dir = std::env::temp_dir().join("test_cache");
        if temp_dir.exists() {
            std::fs::remove_dir_all(&temp_dir).ok();
        }
        std::fs::create_dir_all(&temp_dir).unwrap();

        let cache = TieredCache::new(
            10,                       // memory capacity
            Duration::from_secs(300), // memory TTL
            temp_dir,
            Duration::from_secs(3600), // disk TTL
            1024 * 1024,               // 1MB disk size
        )
        .await
        .unwrap();
        Arc::new(RwLock::new(cache))
    }

    #[tokio::test]
    async fn test_cache_stats_tool() {
        let cache = create_test_cache().await;
        let tool = CacheStatsTool::new();
        let client = Arc::new(rustacean_docs_client::DocsClient::new().unwrap());

        // Add some data to cache
        {
            let cache_guard = cache.read().await;
            cache_guard
                .insert("test_key".to_string(), json!({"value": "test"}))
                .await
                .unwrap();
        }

        let result = tool.execute(json!({}), &client, &cache).await.unwrap();

        assert_eq!(result["status"], "success");
        assert!(result["cache_stats"]["summary"].is_object());
        assert!(result["cache_stats"]["memory_cache"].is_object());
        assert!(result["cache_stats"]["disk_cache"].is_object());
        assert!(result["cache_stats"]["efficiency_metrics"].is_object());
    }

    #[tokio::test]
    async fn test_clear_cache_tool() {
        let cache = create_test_cache().await;
        let tool = ClearCacheTool::new();
        let client = Arc::new(rustacean_docs_client::DocsClient::new().unwrap());

        // Add some data to cache
        {
            let cache_guard = cache.read().await;
            cache_guard
                .insert("test_key1".to_string(), json!({"value": "test1"}))
                .await
                .unwrap();
            cache_guard
                .insert("test_key2".to_string(), json!({"value": "test2"}))
                .await
                .unwrap();
        }

        let result = tool.execute(json!({}), &client, &cache).await.unwrap();

        assert_eq!(result["status"], "success");
        assert!(
            result["cleared"]["items_cleared"]["total"]
                .as_u64()
                .unwrap()
                >= 2
        );

        // Verify cache is actually cleared
        {
            let cache_guard = cache.read().await;
            assert_eq!(
                cache_guard.get(&"test_key1".to_string()).await.unwrap(),
                None
            );
            assert_eq!(
                cache_guard.get(&"test_key2".to_string()).await.unwrap(),
                None
            );
        }
    }

    #[tokio::test]
    async fn test_cache_maintenance_tool() {
        let cache = create_test_cache().await;
        let tool = CacheMaintenanceTool::new();
        let client = Arc::new(rustacean_docs_client::DocsClient::new().unwrap());

        let result = tool.execute(json!({}), &client, &cache).await.unwrap();

        assert_eq!(result["status"], "success");
        assert!(result["maintenance"]["maintenance_summary"].is_object());
        assert!(result["maintenance"]["details"]["memory_cache"].is_object());
        assert!(result["maintenance"]["details"]["disk_cache"].is_object());
    }

    #[test]
    fn test_performance_assessment() {
        let tool = CacheStatsTool::new();

        let excellent_stats = CombinedCacheStats {
            memory: CacheLayerStats {
                size: 10,
                capacity: 100,
                requests: 100,
                hits: 95,
                misses: 5,
                hit_rate: 95.0,
                bytes_used: None,
                bytes_capacity: None,
            },
            disk: CacheLayerStats {
                size: 5,
                capacity: 1000,
                requests: 100,
                hits: 95,
                misses: 5,
                hit_rate: 95.0,
                bytes_used: Some(1024),
                bytes_capacity: Some(1024 * 1024),
            },
            total_requests: 100,
            total_hits: 95,
            total_misses: 5,
            combined_hit_rate: 95.0,
        };
        assert_eq!(tool.assess_performance(&excellent_stats), "excellent");

        let poor_stats = CombinedCacheStats {
            memory: CacheLayerStats {
                size: 5,
                capacity: 100,
                requests: 100,
                hits: 30,
                misses: 70,
                hit_rate: 30.0,
                bytes_used: None,
                bytes_capacity: None,
            },
            disk: CacheLayerStats {
                size: 2,
                capacity: 1000,
                requests: 100,
                hits: 30,
                misses: 70,
                hit_rate: 30.0,
                bytes_used: Some(512),
                bytes_capacity: Some(1024 * 1024),
            },
            total_requests: 100,
            total_hits: 30,
            total_misses: 70,
            combined_hit_rate: 30.0,
        };
        assert_eq!(tool.assess_performance(&poor_stats), "needs_optimization");
    }

    #[test]
    fn test_tool_descriptions() {
        let cache_stats_tool = CacheStatsTool::new();
        let clear_cache_tool = ClearCacheTool::new();
        let maintenance_tool = CacheMaintenanceTool::new();

        assert!(!cache_stats_tool.description().is_empty());
        assert!(!clear_cache_tool.description().is_empty());
        assert!(!maintenance_tool.description().is_empty());
    }

    #[test]
    fn test_parameter_schemas() {
        let cache_stats_tool = CacheStatsTool::new();
        let clear_cache_tool = ClearCacheTool::new();
        let maintenance_tool = CacheMaintenanceTool::new();

        // All cache operation tools should require no parameters
        assert_eq!(cache_stats_tool.parameters_schema()["required"], json!([]));
        assert_eq!(clear_cache_tool.parameters_schema()["required"], json!([]));
        assert_eq!(maintenance_tool.parameters_schema()["required"], json!([]));
    }

    #[test]
    fn test_recommendation_generation() {
        let tool = CacheStatsTool::new();

        // Test low hit rate scenario
        let low_hit_stats = CombinedCacheStats {
            memory: CacheLayerStats {
                size: 50,
                capacity: 100,
                requests: 100,
                hits: 10,
                misses: 90,
                hit_rate: 10.0,
                bytes_used: Some(1024),
                bytes_capacity: Some(2048),
            },
            disk: CacheLayerStats {
                size: 20,
                capacity: 1000,
                requests: 100,
                hits: 10,
                misses: 90,
                hit_rate: 10.0,
                bytes_used: Some(512),
                bytes_capacity: Some(1024 * 1024),
            },
            total_requests: 100,
            total_hits: 10,
            total_misses: 90,
            combined_hit_rate: 10.0,
        };

        let recommendations = tool.generate_recommendations(&low_hit_stats);
        assert!(!recommendations.is_empty());

        // Should contain high priority recommendation for low hit rate
        let high_priority_rec = recommendations
            .iter()
            .find(|r| r["priority"] == "high")
            .expect("Should have high priority recommendation");
        assert_eq!(high_priority_rec["category"], "hit_rate");
    }
}
