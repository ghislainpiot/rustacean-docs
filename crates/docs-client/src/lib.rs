pub mod client;
pub mod config;
pub mod endpoints;
pub mod error_handling;
pub mod html_parser;
pub mod service_config;

pub use client::{ClientConfig, DocsClient};
pub use config::{DocsClientConfig, HtmlParsingConfig, ApiItemPatterns, UrlConfig};
pub use html_parser::HtmlParser;
pub use service_config::{ServiceConfig, ServiceBuilder, ServicesRegistry};
pub use endpoints::{
    CrateDocsCacheKey, DocsService, ItemDocsCacheKey, MetadataCacheKey, MetadataService,
    RecentReleasesCacheKey, ReleasesCacheKey, ReleasesService, SearchCacheKey, SearchService,
};
