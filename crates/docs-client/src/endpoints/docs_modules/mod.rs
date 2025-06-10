pub mod cache_keys;
pub mod service;

// Re-export commonly used types
pub use cache_keys::{CrateDocsCacheKey, ItemDocsCacheKey, RecentReleasesCacheKey};
pub use service::DocsService;
