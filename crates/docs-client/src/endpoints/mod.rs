// Module for API endpoints

pub mod docs;
pub mod metadata;
pub mod releases;
pub mod search;

// Re-export commonly used types
pub use docs::{CrateDocsCacheKey, DocsService, ItemDocsCacheKey, RecentReleasesCacheKey};
pub use metadata::{MetadataCacheKey, MetadataService};
pub use releases::{ReleasesCacheKey, ReleasesService};
pub use search::{SearchCacheKey, SearchService};
