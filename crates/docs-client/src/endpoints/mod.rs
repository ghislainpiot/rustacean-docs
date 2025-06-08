// Module for API endpoints

pub mod docs;
pub mod metadata;
pub mod search;

// Re-export commonly used types
pub use docs::{CrateDocsCacheKey, DocsService, ItemDocsCacheKey, RecentReleasesCacheKey};
pub use metadata::{MetadataCacheKey, MetadataService};
pub use search::{SearchCacheKey, SearchService};
