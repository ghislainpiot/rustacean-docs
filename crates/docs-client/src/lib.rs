pub mod client;
pub mod endpoints;
pub mod error_handling;
pub mod html_parser;

pub use client::{ClientConfig, DocsClient};
pub use html_parser::HtmlParser;
pub use endpoints::{
    CrateDocsCacheKey, DocsService, ItemDocsCacheKey, MetadataCacheKey, MetadataService,
    RecentReleasesCacheKey, ReleasesCacheKey, ReleasesService, SearchCacheKey, SearchService,
};
