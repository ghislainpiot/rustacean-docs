use crate::{DocsClient, DocsService, MetadataService, ReleasesService, SearchService};
use std::time::Duration;

/// Configuration for all endpoint services
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    /// Default cache capacity for all services
    pub default_cache_capacity: usize,
    /// Default cache TTL for all services
    pub default_cache_ttl: Duration,
    /// Override cache capacity for search service (if None, uses default)
    pub search_cache_capacity: Option<usize>,
    /// Override cache TTL for search service (if None, uses default)
    pub search_cache_ttl: Option<Duration>,
    /// Override cache capacity for metadata service (if None, uses default)
    pub metadata_cache_capacity: Option<usize>,
    /// Override cache TTL for metadata service (if None, uses default)
    pub metadata_cache_ttl: Option<Duration>,
    /// Override cache capacity for docs service (if None, uses default)
    pub docs_cache_capacity: Option<usize>,
    /// Override cache TTL for docs service (if None, uses default)
    pub docs_cache_ttl: Option<Duration>,
    /// Override cache capacity for releases service (if None, uses default)
    pub releases_cache_capacity: Option<usize>,
    /// Override cache TTL for releases service (if None, uses default)
    pub releases_cache_ttl: Option<Duration>,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            default_cache_capacity: 1000,
            default_cache_ttl: Duration::from_secs(3600), // 1 hour
            search_cache_capacity: None,
            search_cache_ttl: None,
            metadata_cache_capacity: None,
            metadata_cache_ttl: None,
            docs_cache_capacity: None,
            docs_cache_ttl: None,
            releases_cache_capacity: Some(100), // Smaller cache for releases
            releases_cache_ttl: Some(Duration::from_secs(1800)), // 30 minutes
        }
    }
}

impl ServiceConfig {
    /// Create a new service configuration with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the default cache capacity for all services
    pub fn with_default_cache_capacity(mut self, capacity: usize) -> Self {
        self.default_cache_capacity = capacity;
        self
    }

    /// Set the default cache TTL for all services
    pub fn with_default_cache_ttl(mut self, ttl: Duration) -> Self {
        self.default_cache_ttl = ttl;
        self
    }

    /// Override search service cache settings
    pub fn with_search_cache(mut self, capacity: usize, ttl: Duration) -> Self {
        self.search_cache_capacity = Some(capacity);
        self.search_cache_ttl = Some(ttl);
        self
    }

    /// Override metadata service cache settings
    pub fn with_metadata_cache(mut self, capacity: usize, ttl: Duration) -> Self {
        self.metadata_cache_capacity = Some(capacity);
        self.metadata_cache_ttl = Some(ttl);
        self
    }

    /// Override docs service cache settings
    pub fn with_docs_cache(mut self, capacity: usize, ttl: Duration) -> Self {
        self.docs_cache_capacity = Some(capacity);
        self.docs_cache_ttl = Some(ttl);
        self
    }

    /// Override releases service cache settings
    pub fn with_releases_cache(mut self, capacity: usize, ttl: Duration) -> Self {
        self.releases_cache_capacity = Some(capacity);
        self.releases_cache_ttl = Some(ttl);
        self
    }

    /// Get effective cache capacity for search service
    pub fn search_cache_capacity(&self) -> usize {
        self.search_cache_capacity
            .unwrap_or(self.default_cache_capacity)
    }

    /// Get effective cache TTL for search service
    pub fn search_cache_ttl(&self) -> Duration {
        self.search_cache_ttl.unwrap_or(self.default_cache_ttl)
    }

    /// Get effective cache capacity for metadata service
    pub fn metadata_cache_capacity(&self) -> usize {
        self.metadata_cache_capacity
            .unwrap_or(self.default_cache_capacity)
    }

    /// Get effective cache TTL for metadata service
    pub fn metadata_cache_ttl(&self) -> Duration {
        self.metadata_cache_ttl.unwrap_or(self.default_cache_ttl)
    }

    /// Get effective cache capacity for docs service
    pub fn docs_cache_capacity(&self) -> usize {
        self.docs_cache_capacity
            .unwrap_or(self.default_cache_capacity)
    }

    /// Get effective cache TTL for docs service
    pub fn docs_cache_ttl(&self) -> Duration {
        self.docs_cache_ttl.unwrap_or(self.default_cache_ttl)
    }

    /// Get effective cache capacity for releases service
    pub fn releases_cache_capacity(&self) -> usize {
        self.releases_cache_capacity
            .unwrap_or(self.default_cache_capacity)
    }

    /// Get effective cache TTL for releases service
    pub fn releases_cache_ttl(&self) -> Duration {
        self.releases_cache_ttl.unwrap_or(self.default_cache_ttl)
    }
}

/// Builder for creating endpoint services with unified configuration
pub struct ServiceBuilder {
    client: DocsClient,
    config: ServiceConfig,
}

impl ServiceBuilder {
    /// Create a new service builder with the given client and default configuration
    pub fn new(client: DocsClient) -> Self {
        Self {
            client,
            config: ServiceConfig::new(),
        }
    }

    /// Create a new service builder with custom configuration
    pub fn with_config(client: DocsClient, config: ServiceConfig) -> Self {
        Self { client, config }
    }

    /// Update the service configuration
    pub fn set_config(mut self, config: ServiceConfig) -> Self {
        self.config = config;
        self
    }

    /// Build a search service with configured cache settings
    pub fn build_search_service(&self) -> SearchService {
        SearchService::new(
            self.client.clone(),
            self.config.search_cache_capacity(),
            self.config.search_cache_ttl(),
        )
    }

    /// Build a metadata service with configured cache settings
    pub fn build_metadata_service(&self) -> MetadataService {
        MetadataService::with_cache_config(
            self.client.clone(),
            self.config.metadata_cache_capacity(),
            self.config.metadata_cache_ttl(),
        )
    }

    /// Build a docs service with configured cache settings
    pub fn build_docs_service(&self) -> DocsService {
        DocsService::new(
            self.client.clone(),
            self.config.docs_cache_capacity(),
            self.config.docs_cache_ttl(),
        )
    }

    /// Build a releases service with configured cache settings
    pub fn build_releases_service(&self) -> ReleasesService {
        ReleasesService::with_cache_config(
            self.client.clone(),
            self.config.releases_cache_capacity(),
            self.config.releases_cache_ttl(),
        )
    }

    /// Build all services at once, returning a services registry
    pub fn build_all_services(&self) -> ServicesRegistry {
        ServicesRegistry {
            search: self.build_search_service(),
            metadata: self.build_metadata_service(),
            docs: self.build_docs_service(),
            releases: self.build_releases_service(),
        }
    }
}

/// Registry containing all endpoint services
pub struct ServicesRegistry {
    pub search: SearchService,
    pub metadata: MetadataService,
    pub docs: DocsService,
    pub releases: ReleasesService,
}

impl ServicesRegistry {
    /// Create a new services registry with default configuration
    pub fn new(client: DocsClient) -> Self {
        ServiceBuilder::new(client).build_all_services()
    }

    /// Create a new services registry with custom configuration
    pub fn with_config(client: DocsClient, config: ServiceConfig) -> Self {
        ServiceBuilder::with_config(client, config).build_all_services()
    }

    /// Get a reference to the search service
    pub fn search(&self) -> &SearchService {
        &self.search
    }

    /// Get a reference to the metadata service
    pub fn metadata(&self) -> &MetadataService {
        &self.metadata
    }

    /// Get a reference to the docs service
    pub fn docs(&self) -> &DocsService {
        &self.docs
    }

    /// Get a reference to the releases service
    pub fn releases(&self) -> &ReleasesService {
        &self.releases
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_service_config_defaults() {
        let config = ServiceConfig::new();

        assert_eq!(config.default_cache_capacity, 1000);
        assert_eq!(config.default_cache_ttl, Duration::from_secs(3600));

        // Test that search uses defaults when no override
        assert_eq!(config.search_cache_capacity(), 1000);
        assert_eq!(config.search_cache_ttl(), Duration::from_secs(3600));

        // Test that releases uses overrides by default
        assert_eq!(config.releases_cache_capacity(), 100);
        assert_eq!(config.releases_cache_ttl(), Duration::from_secs(1800));
    }

    #[test]
    fn test_service_config_builder_pattern() {
        let config = ServiceConfig::new()
            .with_default_cache_capacity(500)
            .with_default_cache_ttl(Duration::from_secs(1800))
            .with_search_cache(200, Duration::from_secs(600))
            .with_metadata_cache(800, Duration::from_secs(7200));

        assert_eq!(config.default_cache_capacity, 500);
        assert_eq!(config.default_cache_ttl, Duration::from_secs(1800));

        assert_eq!(config.search_cache_capacity(), 200);
        assert_eq!(config.search_cache_ttl(), Duration::from_secs(600));

        assert_eq!(config.metadata_cache_capacity(), 800);
        assert_eq!(config.metadata_cache_ttl(), Duration::from_secs(7200));

        // Docs should use defaults since no override
        assert_eq!(config.docs_cache_capacity(), 500);
        assert_eq!(config.docs_cache_ttl(), Duration::from_secs(1800));
    }

    #[test]
    fn test_service_builder_creation() {
        let client = DocsClient::new().unwrap();
        let builder = ServiceBuilder::new(client);

        // Should not panic when building services
        let _search = builder.build_search_service();
        let _metadata = builder.build_metadata_service();
        let _docs = builder.build_docs_service();
        let _releases = builder.build_releases_service();
    }

    #[test]
    fn test_services_registry() {
        let client = DocsClient::new().unwrap();
        let registry = ServicesRegistry::new(client);

        // Should have all services available - just ensure they're not panicking when accessed
        let _search = registry.search();
        let _metadata = registry.metadata();
        let _docs = registry.docs();
        let _releases = registry.releases();
    }

    #[test]
    fn test_services_registry_with_config() {
        let client = DocsClient::new().unwrap();
        let config = ServiceConfig::new()
            .with_default_cache_capacity(250)
            .with_search_cache(50, Duration::from_secs(300));

        let registry = ServicesRegistry::with_config(client, config);

        // Should have all services configured properly - just ensure they're accessible
        let _search = registry.search();
        let _metadata = registry.metadata();
        let _docs = registry.docs();
        let _releases = registry.releases();
    }
}
