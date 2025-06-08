pub mod error;
pub mod models;
pub mod utils;

pub use error::{Error, ErrorCategory, ErrorContext, Result};

// Re-export commonly used models for convenience
pub use models::{
    docs::{
        CodeExample, CrateCategories, CrateDocsRequest, CrateDocsResponse, CrateItem, CrateRelease,
        CrateSummary, ItemDocsRequest, ItemDocsResponse, ItemKind, RecentReleasesRequest,
        RecentReleasesResponse, Visibility,
    },
    metadata::{
        CacheConfig, CacheLayerStats, CacheStats, ClearCacheResponse, CrateMetadata,
        CrateMetadataRequest, Dependency, DependencyKind, DownloadStats, PerformanceStats,
        VersionInfo,
    },
    search::{CrateSearchResult, SearchRequest, SearchResponse},
};

// Re-export version utilities
pub use utils::version::{resolve_version, normalize_version, is_latest_version, to_optional_version, DEFAULT_VERSION};
