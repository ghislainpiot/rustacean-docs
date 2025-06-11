pub mod constants;
pub mod error;
pub mod models;
pub mod traits;
pub mod types;
pub mod utils;

pub use error::{
    CacheError, ConfigError, DocsError, Error, ErrorBuilder, ErrorCategory, ErrorContext,
    NetworkError, ProtocolError, Result, StructuredContext,
};

// Re-export commonly used models for convenience
pub use models::{
    docs::{
        CodeExample, CrateCategories, CrateDocsRequest, CrateDocsResponse, CrateItem, CrateRelease,
        CrateSummary, ItemDocsRequest, ItemDocsResponse, ItemKind, RecentReleasesRequest,
        RecentReleasesResponse, Visibility,
    },
    metadata::{
        CacheConfig, CacheStats, ClearCacheResponse, CrateMetadata, CrateMetadataRequest,
        Dependency, DependencyKind, DownloadStats, VersionInfo,
    },
    search::{CrateSearchResult, SearchRequest, SearchResponse},
};

// Re-export version utilities
pub use utils::version::{
    is_latest_version, normalize_version, resolve_version, to_optional_version, DEFAULT_VERSION,
};

// Re-export common types
pub use types::{CrateName, ItemPath, Version};

// Re-export traits
pub use traits::{
    Cacheable, PaginatedRequest, Request, RequestBuilder, Response, VersionedRequest,
};
