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
        CacheConfig, CacheStats, ClearCacheResponse, CrateMetadata,
        CrateMetadataRequest, Dependency, DependencyKind, DownloadStats,
        VersionInfo,
    },
    search::{CrateSearchResult, SearchRequest, SearchResponse},
};

// Re-export version utilities
pub use utils::version::{
    is_latest_version, normalize_version, resolve_version, to_optional_version, DEFAULT_VERSION,
};
