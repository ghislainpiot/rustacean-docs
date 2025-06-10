use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

/// Request for crate metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrateMetadataRequest {
    /// Name of the crate
    pub crate_name: String,
    /// Specific version to query (defaults to latest)
    pub version: Option<String>,
}

impl CrateMetadataRequest {
    pub fn new(crate_name: impl Into<String>) -> Self {
        Self {
            crate_name: crate_name.into(),
            version: None,
        }
    }

    pub fn with_version(crate_name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            crate_name: crate_name.into(),
            version: Some(version.into()),
        }
    }
}

/// Comprehensive crate metadata including project information and dependencies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrateMetadata {
    /// Crate name
    pub name: String,
    /// Current version
    pub version: String,
    /// Brief description
    pub description: Option<String>,
    /// License information (SPDX identifier or free-form)
    pub license: Option<String>,
    /// Repository URL
    pub repository: Option<Url>,
    /// Homepage URL
    pub homepage: Option<Url>,
    /// Documentation URL
    pub documentation: Option<Url>,
    /// Authors
    pub authors: Vec<String>,
    /// Keywords
    pub keywords: Vec<String>,
    /// Categories
    pub categories: Vec<String>,
    /// Download statistics
    pub downloads: DownloadStats,
    /// Version history
    pub versions: Vec<VersionInfo>,
    /// Dependencies for the current version
    pub dependencies: Vec<Dependency>,
    /// Development dependencies
    pub dev_dependencies: Vec<Dependency>,
    /// Build dependencies
    pub build_dependencies: Vec<Dependency>,
    /// Crate features
    pub features: HashMap<String, Vec<String>>,
    /// Minimum supported Rust version
    pub rust_version: Option<String>,
    /// Publication date
    pub created_at: Option<DateTime<Utc>>,
    /// Last update date
    pub updated_at: Option<DateTime<Utc>>,
}

/// Download statistics for a crate
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DownloadStats {
    /// Total downloads across all versions
    pub total: u64,
    /// Downloads for the current version
    pub version: u64,
    /// Recent downloads (last 90 days)
    pub recent: u64,
}

/// Information about a specific version
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VersionInfo {
    /// Version number
    pub num: String,
    /// Publication date
    pub created_at: DateTime<Utc>,
    /// Whether this version has been yanked
    pub yanked: bool,
    /// Rust version requirement
    pub rust_version: Option<String>,
    /// Download count for this version
    pub downloads: u64,
    /// Features available in this version
    pub features: HashMap<String, Vec<String>>,
}

/// Dependency information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Dependency {
    /// Dependency name
    pub name: String,
    /// Version requirement
    pub version_req: String,
    /// Dependency features
    pub features: Vec<String>,
    /// Whether it's optional
    pub optional: bool,
    /// Whether it uses default features
    pub default_features: bool,
    /// Target platform (if any)
    pub target: Option<String>,
    /// Dependency kind (normal, dev, build)
    pub kind: DependencyKind,
}

/// Type of dependency
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DependencyKind {
    Normal,
    Dev,
    Build,
}

/// Cache statistics - simplified version for the new cache design
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CacheStats {
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
    /// Current number of items in cache
    pub size: usize,
    /// Maximum capacity
    pub capacity: usize,
    /// Cache configuration
    pub config: CacheConfig,
}

impl CacheStats {
    /// Calculate hit rate as a percentage
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            (self.hits as f64 / total as f64) * 100.0
        }
    }

    /// Calculate cache utilization as a percentage
    pub fn utilization(&self) -> f64 {
        if self.capacity == 0 {
            0.0
        } else {
            (self.size as f64 / self.capacity as f64) * 100.0
        }
    }
}

/// Cache configuration information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CacheConfig {
    /// Memory cache capacity
    pub memory_capacity: usize,
    /// Disk cache maximum size in bytes
    pub disk_capacity_bytes: u64,
    /// Default TTL for documentation (seconds)
    pub docs_ttl_seconds: u64,
    /// Default TTL for search results (seconds)
    pub search_ttl_seconds: u64,
    /// Cache directory path
    pub cache_dir: String,
}

/// Response for cache clear operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClearCacheResponse {
    /// Whether the operation was successful
    pub success: bool,
    /// Number of items cleared from memory cache
    pub memory_items_cleared: usize,
    /// Number of bytes cleared from disk cache
    pub disk_bytes_cleared: u64,
    /// Confirmation message
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;
    use url::Url;

    #[test]
    fn test_crate_metadata_request_new() {
        let req = CrateMetadataRequest::new("tokio");
        assert_eq!(req.crate_name, "tokio");
        assert_eq!(req.version, None);
    }

    #[test]
    fn test_crate_metadata_request_with_version() {
        let req = CrateMetadataRequest::with_version("tokio", "1.0.0");
        assert_eq!(req.crate_name, "tokio");
        assert_eq!(req.version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_dependency_kind_serialization() {
        let kinds = vec![
            DependencyKind::Normal,
            DependencyKind::Dev,
            DependencyKind::Build,
        ];

        for kind in kinds {
            let json = serde_json::to_string(&kind).unwrap();
            let deserialized: DependencyKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, deserialized);
        }
    }

    #[test]
    fn test_download_stats_serialization() {
        let stats = DownloadStats {
            total: 1000000,
            version: 50000,
            recent: 10000,
        };

        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: DownloadStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats, deserialized);
    }

    #[test]
    fn test_version_info_serialization() {
        let mut features = HashMap::new();
        features.insert("default".to_string(), vec!["std".to_string()]);

        let version = VersionInfo {
            num: "1.0.0".to_string(),
            created_at: Utc::now(),
            yanked: false,
            rust_version: Some("1.70.0".to_string()),
            downloads: 100000,
            features,
        };

        let json = serde_json::to_string(&version).unwrap();
        let deserialized: VersionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(version, deserialized);
    }

    #[test]
    fn test_dependency_serialization() {
        let dep = Dependency {
            name: "serde".to_string(),
            version_req: "^1.0".to_string(),
            features: vec!["derive".to_string()],
            optional: false,
            default_features: true,
            target: None,
            kind: DependencyKind::Normal,
        };

        let json = serde_json::to_string(&dep).unwrap();
        let deserialized: Dependency = serde_json::from_str(&json).unwrap();
        assert_eq!(dep, deserialized);
    }

    #[test]
    fn test_crate_metadata_serialization() {
        let mut features = HashMap::new();
        features.insert("default".to_string(), vec!["std".to_string()]);

        let metadata = CrateMetadata {
            name: "tokio".to_string(),
            version: "1.35.0".to_string(),
            description: Some(
                "A runtime for writing reliable, asynchronous applications".to_string(),
            ),
            license: Some("MIT".to_string()),
            repository: Some(Url::parse("https://github.com/tokio-rs/tokio").unwrap()),
            homepage: Some(Url::parse("https://tokio.rs").unwrap()),
            documentation: Some(Url::parse("https://docs.rs/tokio").unwrap()),
            authors: vec!["Tokio Contributors".to_string()],
            keywords: vec!["async".to_string(), "runtime".to_string()],
            categories: vec!["asynchronous".to_string()],
            downloads: DownloadStats {
                total: 1000000,
                version: 50000,
                recent: 10000,
            },
            versions: vec![],
            dependencies: vec![],
            dev_dependencies: vec![],
            build_dependencies: vec![],
            features,
            rust_version: Some("1.70.0".to_string()),
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
        };

        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: CrateMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(metadata, deserialized);
    }

    #[test]
    fn test_cache_layer_stats_serialization() {
        let stats = CacheLayerStats {
            size: 500,
            capacity: 1000,
            requests: 10000,
            hits: 8000,
            misses: 2000,
            hit_rate: 80.0,
            bytes_used: Some(1048576),
            bytes_capacity: Some(10485760),
        };

        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: CacheLayerStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats, deserialized);
    }

    #[test]
    fn test_performance_stats_serialization() {
        let stats = PerformanceStats {
            avg_hit_time_ms: 5.2,
            avg_miss_time_ms: 250.5,
            avg_population_time_ms: 180.3,
            evictions: 100,
            expirations: 50,
        };

        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: PerformanceStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats, deserialized);
    }

    #[test]
    fn test_cache_config_serialization() {
        let config = CacheConfig {
            memory_capacity: 1000,
            disk_capacity_bytes: 524288000,
            docs_ttl_seconds: 3600,
            search_ttl_seconds: 300,
            cache_dir: "/tmp/rustacean-docs".to_string(),
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: CacheConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_cache_stats_serialization() {
        let stats = CacheStats {
            memory: CacheLayerStats {
                size: 500,
                capacity: 1000,
                requests: 10000,
                hits: 8000,
                misses: 2000,
                hit_rate: 80.0,
                bytes_used: None,
                bytes_capacity: None,
            },
            disk: CacheLayerStats {
                size: 100,
                capacity: 1000,
                requests: 2000,
                hits: 1800,
                misses: 200,
                hit_rate: 90.0,
                bytes_used: Some(1048576),
                bytes_capacity: Some(524288000),
            },
            performance: PerformanceStats {
                avg_hit_time_ms: 5.2,
                avg_miss_time_ms: 250.5,
                avg_population_time_ms: 180.3,
                evictions: 100,
                expirations: 50,
            },
            config: CacheConfig {
                memory_capacity: 1000,
                disk_capacity_bytes: 524288000,
                docs_ttl_seconds: 3600,
                search_ttl_seconds: 300,
                cache_dir: "/tmp/rustacean-docs".to_string(),
            },
        };

        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: CacheStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats, deserialized);
    }

    #[test]
    fn test_clear_cache_response_serialization() {
        let response = ClearCacheResponse {
            success: true,
            memory_items_cleared: 500,
            disk_bytes_cleared: 1048576,
            message: "Cache cleared successfully".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: ClearCacheResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(response, deserialized);
    }

    #[test]
    fn test_metadata_models_minimal_data() {
        let metadata = CrateMetadata {
            name: "minimal".to_string(),
            version: "0.1.0".to_string(),
            description: None,
            license: None,
            repository: None,
            homepage: None,
            documentation: None,
            authors: vec![],
            keywords: vec![],
            categories: vec![],
            downloads: DownloadStats {
                total: 0,
                version: 0,
                recent: 0,
            },
            versions: vec![],
            dependencies: vec![],
            dev_dependencies: vec![],
            build_dependencies: vec![],
            features: HashMap::new(),
            rust_version: None,
            created_at: None,
            updated_at: None,
        };

        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: CrateMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(metadata, deserialized);
    }
}
