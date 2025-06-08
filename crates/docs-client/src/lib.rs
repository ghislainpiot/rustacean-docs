pub mod client;
pub mod endpoints;

pub use client::{ClientConfig, DocsClient};
pub use endpoints::{
    CrateDocsCacheKey, DocsService, ItemDocsCacheKey, RecentReleasesCacheKey, SearchCacheKey,
    SearchService, MetadataCacheKey, MetadataService, ReleasesCacheKey, ReleasesService,
};
