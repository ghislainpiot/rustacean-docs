pub mod error;
pub mod models;

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
