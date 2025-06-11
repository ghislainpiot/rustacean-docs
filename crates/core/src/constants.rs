/// Default limit for search results
pub const DEFAULT_SEARCH_LIMIT: usize = 10;

/// Maximum limit for search results
pub const MAX_SEARCH_LIMIT: usize = 50;

/// Default limit for recent releases
pub const DEFAULT_RECENT_RELEASES_LIMIT: usize = 20;

/// Maximum limit for recent releases
pub const MAX_RECENT_RELEASES_LIMIT: usize = 100;

/// Default cache TTL for crate docs (1 hour)
pub const DEFAULT_CRATE_DOCS_TTL: u64 = 3600;

/// Default cache TTL for item docs (1 hour)
pub const DEFAULT_ITEM_DOCS_TTL: u64 = 3600;

/// Default cache TTL for metadata (6 hours)
pub const DEFAULT_METADATA_TTL: u64 = 21600;

/// Default cache TTL for search results (5 minutes)
pub const DEFAULT_SEARCH_TTL: u64 = 300;

/// Default cache TTL for recent releases (30 minutes)
pub const DEFAULT_RECENT_RELEASES_TTL: u64 = 1800;

/// Maximum crate name length
pub const MAX_CRATE_NAME_LENGTH: usize = 64;

/// Maximum number of dependencies to show
pub const MAX_DEPENDENCIES_DISPLAY: usize = 20;

/// Maximum number of features to show
pub const MAX_FEATURES_DISPLAY: usize = 50;
