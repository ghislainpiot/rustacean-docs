// Re-export from the new modular structure
pub use crate::html_parser::{
    parse_crate_documentation, parse_item_documentation, parse_recent_releases,
    resolve_item_path_with_fallback,
};

// Re-export the new modular components
pub use crate::endpoints::docs_modules::{
    cache_keys::{CrateDocsCacheKey, ItemDocsCacheKey, RecentReleasesCacheKey},
    service::DocsService,
};
