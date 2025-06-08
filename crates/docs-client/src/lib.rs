pub mod client;
pub mod endpoints;

pub use client::{ClientConfig, DocsClient};
pub use endpoints::{
    CrateDocsCacheKey, DocsService, ItemDocsCacheKey, MetadataCacheKey, MetadataService,
    RecentReleasesCacheKey, ReleasesCacheKey, ReleasesService, SearchCacheKey, SearchService,
};
