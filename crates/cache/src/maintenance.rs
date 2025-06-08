use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, Interval};
use tracing::{debug, error, info, warn};

use crate::strategy::{MaintenanceReport, TieredCache};

/// Configuration for cache maintenance operations
#[derive(Debug, Clone)]
pub struct MaintenanceConfig {
    /// How often to run maintenance operations
    pub interval: Duration,
    /// Whether to enable automatic maintenance
    pub enabled: bool,
    /// Maximum number of consecutive failures before disabling maintenance
    pub max_failures: u32,
}

impl Default for MaintenanceConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(300), // 5 minutes
            enabled: true,
            max_failures: 5,
        }
    }
}

/// Background task manager for cache maintenance operations
pub struct MaintenanceManager<K, V> {
    cache: Arc<TieredCache<K, V>>,
    config: MaintenanceConfig,
    interval: Interval,
    failure_count: u32,
    enabled: bool,
}

impl<K, V> MaintenanceManager<K, V>
where
    K: std::hash::Hash + Eq + Clone + Send + Sync + std::fmt::Display + 'static,
    V: Clone + Send + Sync + serde::Serialize + serde::de::DeserializeOwned + 'static,
{
    /// Create a new maintenance manager
    pub fn new(cache: Arc<TieredCache<K, V>>, config: MaintenanceConfig) -> Self {
        let interval = interval(config.interval);

        debug!(
            "Created maintenance manager with interval {:?}",
            config.interval
        );

        Self {
            cache,
            config: config.clone(),
            interval,
            failure_count: 0,
            enabled: config.enabled,
        }
    }

    /// Start the maintenance task (runs indefinitely until cancelled)
    pub async fn run(&mut self) {
        if !self.enabled {
            debug!("Maintenance manager is disabled");
            return;
        }

        info!("Starting cache maintenance manager");

        loop {
            self.interval.tick().await;

            if !self.enabled {
                debug!("Maintenance disabled, stopping");
                break;
            }

            match self.run_maintenance().await {
                Ok(report) => {
                    debug!("Maintenance completed successfully: {:?}", report);
                    self.failure_count = 0; // Reset failure count on success
                }
                Err(e) => {
                    self.failure_count += 1;
                    error!("Maintenance failed (attempt {}): {}", self.failure_count, e);

                    if self.failure_count >= self.config.max_failures {
                        warn!(
                            "Too many consecutive maintenance failures ({}), disabling maintenance",
                            self.failure_count
                        );
                        self.enabled = false;
                    }
                }
            }
        }

        info!("Cache maintenance manager stopped");
    }

    /// Run a single maintenance cycle
    async fn run_maintenance(&self) -> Result<MaintenanceReport> {
        debug!("Running cache maintenance cycle");

        let start = std::time::Instant::now();
        let report = self.cache.maintenance().await?;
        let duration = start.elapsed();

        debug!(
            "Maintenance cycle completed in {:?}: expired {} memory + {} disk entries, enforced size on {} entries",
            duration, report.memory_expired, report.disk_expired, report.size_enforced
        );

        Ok(report)
    }

    /// Manually trigger a maintenance cycle
    pub async fn trigger_maintenance(&self) -> Result<MaintenanceReport> {
        info!("Manually triggering cache maintenance");
        self.run_maintenance().await
    }

    /// Check if maintenance is currently enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable maintenance (resets failure count)
    pub fn enable(&mut self) {
        info!("Enabling cache maintenance");
        self.enabled = true;
        self.failure_count = 0;
    }

    /// Disable maintenance
    pub fn disable(&mut self) {
        info!("Disabling cache maintenance");
        self.enabled = false;
    }

    /// Get current failure count
    pub fn failure_count(&self) -> u32 {
        self.failure_count
    }

    /// Update configuration
    pub fn update_config(&mut self, config: MaintenanceConfig) {
        info!("Updating maintenance configuration: {:?}", config);

        // Update interval if it changed
        if config.interval != self.config.interval {
            self.interval = interval(config.interval);
        }

        self.config = config;
        self.enabled = self.config.enabled && self.failure_count < self.config.max_failures;
    }
}

/// Utility for running one-off cache operations
pub struct CacheManager<K, V> {
    cache: Arc<TieredCache<K, V>>,
}

impl<K, V> CacheManager<K, V>
where
    K: std::hash::Hash + Eq + Clone + Send + Sync + std::fmt::Display,
    V: Clone + Send + Sync + serde::Serialize + serde::de::DeserializeOwned,
{
    /// Create a new cache manager
    pub fn new(cache: Arc<TieredCache<K, V>>) -> Self {
        Self { cache }
    }

    /// Get comprehensive cache statistics
    pub async fn get_stats(&self) -> Result<crate::strategy::CombinedCacheStats> {
        self.cache.stats().await
    }

    /// Clear all caches
    pub async fn clear_all(&self) -> Result<(usize, usize)> {
        info!("Clearing all caches");
        let result = self.cache.clear().await?;
        info!(
            "Cleared {} memory entries, {} disk entries",
            result.0, result.1
        );
        Ok(result)
    }

    /// Run cache maintenance manually
    pub async fn run_maintenance(&self) -> Result<MaintenanceReport> {
        info!("Running manual cache maintenance");
        let report = self.cache.maintenance().await?;
        info!("Maintenance completed: {:?}", report);
        Ok(report)
    }

    /// Cleanup only expired entries
    pub async fn cleanup_expired(&self) -> Result<(usize, usize)> {
        info!("Cleaning up expired cache entries");
        let result = self.cache.cleanup_expired().await?;
        info!(
            "Cleaned {} memory entries, {} disk entries",
            result.0, result.1
        );
        Ok(result)
    }

    /// Enforce size limits on disk cache
    pub async fn enforce_size_limits(&self) -> Result<usize> {
        info!("Enforcing cache size limits");
        let result = self.cache.enforce_size_limits().await?;
        info!("Size enforcement removed {} entries", result);
        Ok(result)
    }

    /// Get cache size information
    pub async fn get_cache_sizes(&self) -> Result<CacheSizeInfo> {
        let stats = self.cache.stats().await?;

        Ok(CacheSizeInfo {
            memory_entries: stats.memory.size,
            memory_capacity: stats.memory.capacity,
            disk_entries: stats.disk.size,
            disk_bytes_used: stats.disk.bytes_used.unwrap_or(0),
            disk_bytes_capacity: stats.disk.bytes_capacity.unwrap_or(0),
        })
    }

    /// Check cache health (hit rates, sizes, etc.)
    pub async fn health_check(&self) -> Result<CacheHealthReport> {
        let stats = self.cache.stats().await?;
        let size_info = self.get_cache_sizes().await?;

        // Calculate health metrics
        let memory_utilization = if stats.memory.capacity > 0 {
            (stats.memory.size as f64 / stats.memory.capacity as f64) * 100.0
        } else {
            0.0
        };

        let disk_utilization = if size_info.disk_bytes_capacity > 0 {
            (size_info.disk_bytes_used as f64 / size_info.disk_bytes_capacity as f64) * 100.0
        } else {
            0.0
        };

        // Determine health status
        let health_status = if stats.combined_hit_rate < 50.0 {
            HealthStatus::Poor
        } else if memory_utilization > 90.0 || disk_utilization > 90.0 {
            HealthStatus::Warning
        } else if stats.combined_hit_rate > 80.0
            && memory_utilization < 80.0
            && disk_utilization < 80.0
        {
            HealthStatus::Excellent
        } else {
            HealthStatus::Good
        };

        Ok(CacheHealthReport {
            status: health_status,
            combined_hit_rate: stats.combined_hit_rate,
            memory_hit_rate: stats.memory.hit_rate,
            disk_hit_rate: stats.disk.hit_rate,
            memory_utilization,
            disk_utilization,
            total_requests: stats.total_requests,
            recommendations: generate_recommendations(&stats, &size_info, &health_status),
        })
    }
}

/// Cache size information
#[derive(Debug, Clone)]
pub struct CacheSizeInfo {
    pub memory_entries: usize,
    pub memory_capacity: usize,
    pub disk_entries: usize,
    pub disk_bytes_used: u64,
    pub disk_bytes_capacity: u64,
}

/// Health status of the cache system
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HealthStatus {
    Excellent,
    Good,
    Warning,
    Poor,
}

/// Comprehensive cache health report
#[derive(Debug, Clone)]
pub struct CacheHealthReport {
    pub status: HealthStatus,
    pub combined_hit_rate: f64,
    pub memory_hit_rate: f64,
    pub disk_hit_rate: f64,
    pub memory_utilization: f64,
    pub disk_utilization: f64,
    pub total_requests: u64,
    pub recommendations: Vec<String>,
}

/// Generate recommendations based on cache health
fn generate_recommendations(
    stats: &crate::strategy::CombinedCacheStats,
    size_info: &CacheSizeInfo,
    health_status: &HealthStatus,
) -> Vec<String> {
    let mut recommendations = Vec::new();

    match health_status {
        HealthStatus::Poor => {
            if stats.combined_hit_rate < 30.0 {
                recommendations.push("Cache hit rate is very low. Consider increasing cache sizes or reviewing cache TTL settings.".to_string());
            }
            if stats.total_requests < 100 {
                recommendations.push("Low request volume. Cache benefits may be minimal with current usage patterns.".to_string());
            }
        }
        HealthStatus::Warning => {
            if size_info.memory_capacity > 0
                && (size_info.memory_entries as f64 / size_info.memory_capacity as f64) > 0.9
            {
                recommendations.push(
                    "Memory cache is near capacity. Consider increasing memory cache size."
                        .to_string(),
                );
            }
            if size_info.disk_bytes_capacity > 0
                && (size_info.disk_bytes_used as f64 / size_info.disk_bytes_capacity as f64) > 0.9
            {
                recommendations.push("Disk cache is near capacity. Consider increasing disk cache size or reducing TTL.".to_string());
            }
        }
        HealthStatus::Good => {
            if stats.combined_hit_rate > 90.0 {
                recommendations.push("Consider reducing cache sizes to free up resources while maintaining performance.".to_string());
            }
        }
        HealthStatus::Excellent => {
            recommendations
                .push("Cache is performing optimally. No immediate changes needed.".to_string());
        }
    }

    // General recommendations
    if stats.memory.hit_rate > stats.disk.hit_rate + 20.0 {
        recommendations.push("Memory cache is significantly outperforming disk cache. Consider increasing memory cache size.".to_string());
    }

    if stats.disk.hit_rate < 20.0 && stats.disk.size > 1000 {
        recommendations.push("Disk cache has low hit rate but many entries. Consider reviewing cache key distribution or TTL settings.".to_string());
    }

    recommendations
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::TempDir;

    async fn create_test_cache() -> (Arc<TieredCache<String, String>>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let cache = TieredCache::new(
            10,
            Duration::from_secs(60),
            temp_dir.path(),
            Duration::from_secs(60),
            1024 * 1024,
        )
        .await
        .unwrap();
        (Arc::new(cache), temp_dir)
    }

    #[tokio::test]
    async fn test_cache_manager_operations() {
        let (cache, _temp_dir) = create_test_cache().await;
        let manager = CacheManager::new(cache.clone());

        // Insert some test data
        cache
            .insert("key1".to_string(), "value1".to_string())
            .await
            .unwrap();
        cache
            .insert("key2".to_string(), "value2".to_string())
            .await
            .unwrap();

        // Make some requests to generate stats
        let _ = cache.get(&"key1".to_string()).await.unwrap();
        let _ = cache.get(&"key2".to_string()).await.unwrap();

        // Test stats
        let stats = manager.get_stats().await.unwrap();
        assert!(stats.total_requests > 0);

        // Test size info
        let size_info = manager.get_cache_sizes().await.unwrap();
        assert!(size_info.memory_entries > 0);

        // Test health check
        let health = manager.health_check().await.unwrap();
        assert!(matches!(
            health.status,
            HealthStatus::Good | HealthStatus::Excellent
        ));

        // Test cleanup
        let (mem_cleaned, disk_cleaned) = manager.cleanup_expired().await.unwrap();
        // With fresh data and long TTL, should clean 0 entries
        assert_eq!(mem_cleaned, 0);
        assert_eq!(disk_cleaned, 0);

        // Test clear
        let (mem_cleared, disk_cleared) = manager.clear_all().await.unwrap();
        assert!(mem_cleared > 0);
        assert!(disk_cleared > 0);
    }

    #[tokio::test]
    async fn test_maintenance_config() {
        let config = MaintenanceConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_failures, 5);
        assert_eq!(config.interval, Duration::from_secs(300));

        let custom_config = MaintenanceConfig {
            interval: Duration::from_secs(60),
            enabled: false,
            max_failures: 3,
        };
        assert!(!custom_config.enabled);
        assert_eq!(custom_config.max_failures, 3);
        assert_eq!(custom_config.interval, Duration::from_secs(60));
    }

    #[tokio::test]
    async fn test_maintenance_manager_creation() {
        let (cache, _temp_dir) = create_test_cache().await;
        let config = MaintenanceConfig::default();
        let manager = MaintenanceManager::new(cache, config);

        assert!(manager.is_enabled());
        assert_eq!(manager.failure_count(), 0);
    }

    #[tokio::test]
    async fn test_manual_maintenance_trigger() {
        let (cache, _temp_dir) = create_test_cache().await;
        let config = MaintenanceConfig::default();
        let manager = MaintenanceManager::new(cache, config);

        let report = manager.trigger_maintenance().await.unwrap();
        // With empty cache, should have 0 operations
        assert_eq!(report.memory_expired, 0);
        assert_eq!(report.disk_expired, 0);
        assert_eq!(report.size_enforced, 0);
    }

    #[tokio::test]
    async fn test_enable_disable_maintenance() {
        let (cache, _temp_dir) = create_test_cache().await;
        let config = MaintenanceConfig::default();
        let mut manager = MaintenanceManager::new(cache, config);

        assert!(manager.is_enabled());

        manager.disable();
        assert!(!manager.is_enabled());

        manager.enable();
        assert!(manager.is_enabled());
        assert_eq!(manager.failure_count(), 0); // Should reset on enable
    }

    #[tokio::test]
    async fn test_health_check_recommendations() {
        let (cache, _temp_dir) = create_test_cache().await;
        let manager = CacheManager::new(cache.clone());

        // With empty cache, should get recommendations about low usage
        let health = manager.health_check().await.unwrap();
        assert!(!health.recommendations.is_empty());

        // Add some data and check again
        for i in 0..5 {
            cache
                .insert(format!("key{}", i), format!("value{}", i))
                .await
                .unwrap();
        }

        // Generate some cache hits
        for i in 0..5 {
            let _ = cache.get(&format!("key{}", i)).await.unwrap();
        }

        let health = manager.health_check().await.unwrap();
        // Should now have better status
        assert!(matches!(
            health.status,
            HealthStatus::Good | HealthStatus::Excellent
        ));
    }
}
