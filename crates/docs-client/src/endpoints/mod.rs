// Module for API endpoints

pub mod common;
pub mod docs;
pub mod docs_modules;
pub mod metadata;
pub mod releases;
pub mod search;

// Re-export commonly used types
pub use docs::{CrateDocsCacheKey, DocsService, ItemDocsCacheKey, RecentReleasesCacheKey};
pub use metadata::{MetadataCacheKey, MetadataService};
pub use releases::{ReleasesCacheKey, ReleasesService};
pub use search::{SearchCacheKey, SearchService};
