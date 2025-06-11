pub mod client;
pub mod config;
pub mod endpoints;
pub mod error_handling;
pub mod html_parser;
pub mod service_config;

pub use client::{ClientConfig, DocsClient};
pub use config::{ApiItemPatterns, DocsClientConfig, HtmlParsingConfig, UrlConfig};
pub use endpoints::{
    CrateDocsCacheKey, DocsService, ItemDocsCacheKey, MetadataCacheKey, MetadataService,
    RecentReleasesCacheKey, ReleasesService, SearchCacheKey, SearchService,
};
pub use html_parser::HtmlParser;
pub use service_config::{ServiceBuilder, ServiceConfig, ServicesRegistry};
