use crate::{
    client::DocsClient,
    error_handling::{build_basic_docs_url, handle_http_response, parse_json_response},
};
use chrono::Utc;
use rustacean_docs_cache::{Cache, MemoryCache};
use rustacean_docs_core::{
    error::Error,
    models::metadata::{
        CrateMetadata, CrateMetadataRequest, Dependency, DependencyKind, DownloadStats, VersionInfo,
    },
};
use serde::Deserialize;
use std::{collections::HashMap, hash::Hash, sync::Arc, time::Duration};
use tracing::{debug, error, trace};
use url::Url;

/// Cache key for metadata requests
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MetadataCacheKey {
    crate_name: String,
    version: Option<String>,
}

impl MetadataCacheKey {
    #[allow(dead_code)]
    fn new(request: &CrateMetadataRequest) -> Self {
        Self {
            crate_name: request.crate_name.clone(),
            version: request.version.clone(),
        }
    }
}

/// Raw crates.io API response for crate metadata
#[derive(Debug, Deserialize)]
struct CratesIoResponse {
    #[serde(rename = "crate")]
    crate_info: CratesIoCrate,
    versions: Vec<CratesIoVersion>,
}

/// Response from the dependencies endpoint
#[derive(Debug, Deserialize)]
struct DependenciesResponse {
    dependencies: Vec<CratesIoDependency>,
}

/// Crate information from crates.io API
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct CratesIoCrate {
    id: String,
    name: String,
    description: Option<String>,
    homepage: Option<String>,
    documentation: Option<String>,
    repository: Option<String>,
    downloads: u64,
    recent_downloads: Option<u64>,
    keywords: Vec<String>,
    categories: Vec<String>,
    created_at: String,
    updated_at: String,
    max_version: String,
    max_stable_version: Option<String>,
}

/// Version information from crates.io API
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct CratesIoVersion {
    id: u64,
    #[serde(rename = "crate")]
    crate_name: String,
    num: String,
    yanked: bool,
    created_at: String,
    updated_at: String,
    downloads: u64,
    features: HashMap<String, Vec<String>>,
    rust_version: Option<String>,
    license: Option<String>,
    crate_size: Option<u64>,
    published_by: Option<CratesIoUser>,
    dependencies: Option<Vec<CratesIoDependency>>,
}

/// User information from crates.io API
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct CratesIoUser {
    id: u64,
    login: String,
    name: Option<String>,
    avatar: Option<String>,
    url: Option<String>,
}

/// Dependency information from crates.io API
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct CratesIoDependency {
    id: u64,
    version_id: u64,
    crate_id: String,
    req: String,
    optional: bool,
    default_features: bool,
    features: Vec<String>,
    target: Option<String>,
    kind: String,
    downloads: Option<u64>,
}

impl From<CratesIoDependency> for Dependency {
    fn from(dep: CratesIoDependency) -> Self {
        let kind = match dep.kind.as_str() {
            "dev" => DependencyKind::Dev,
            "build" => DependencyKind::Build,
            _ => DependencyKind::Normal,
        };

        Self {
            name: dep.crate_id,
            version_req: dep.req,
            features: dep.features,
            optional: dep.optional,
            default_features: dep.default_features,
            target: dep.target,
            kind,
        }
    }
}

/// Metadata service for fetching crate metadata
pub struct MetadataService {
    client: DocsClient,
    cache: Arc<MemoryCache<MetadataCacheKey, CrateMetadata>>,
}

impl MetadataService {
    /// Create a new metadata service with default cache settings
    pub fn new(client: DocsClient) -> Self {
        Self::with_cache_config(client, 1000, Duration::from_secs(3600))
    }

    /// Create a new metadata service with custom cache configuration
    pub fn with_cache_config(
        client: DocsClient,
        cache_capacity: usize,
        cache_ttl: Duration,
    ) -> Self {
        let cache = Arc::new(MemoryCache::new(cache_capacity));

        debug!(
            cache_capacity = cache_capacity,
            cache_ttl_secs = cache_ttl.as_secs(),
            "Created metadata service with cache"
        );

        Self { client, cache }
    }

    /// Fetch comprehensive metadata for a crate
    pub async fn get_crate_metadata(
        &self,
        request: &CrateMetadataRequest,
    ) -> Result<CrateMetadata, Error> {
        let cache_key = MetadataCacheKey::new(request);

        // Try to get from cache first
        if let Ok(Some(cached_metadata)) = self.cache.get(&cache_key).await {
            trace!(
                crate_name = %request.crate_name,
                version = ?request.version,
                "Metadata cache hit"
            );
            return Ok(cached_metadata);
        }

        trace!(
            crate_name = %request.crate_name,
            version = ?request.version,
            "Metadata cache miss, fetching from API"
        );

        let metadata = self.fetch_metadata_from_api(request).await?;

        // Store in cache for future requests
        let _ = self.cache.insert(cache_key, metadata.clone()).await;

        debug!(
            crate_name = %request.crate_name,
            version = ?request.version,
            "Metadata fetched and cached successfully"
        );

        Ok(metadata)
    }

    async fn fetch_metadata_from_api(
        &self,
        request: &CrateMetadataRequest,
    ) -> Result<CrateMetadata, Error> {
        let url = format!("https://crates.io/api/v1/crates/{}", request.crate_name);

        debug!("Requesting metadata from: {}", url);

        let response = self
            .client
            .inner_client()
            .get(&url)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to fetch metadata from crates.io: {}", e);
                Error::Network(e)
            })?;

        let response = handle_http_response(
            response,
            &format!("crates.io metadata for {}", request.crate_name),
        )
        .await?;
        let mut crates_io_response: CratesIoResponse =
            parse_json_response(response, "crates.io metadata").await?;

        // Fetch dependencies separately for the target version
        let target_version = request
            .version
            .as_deref()
            .unwrap_or(&crates_io_response.crate_info.max_version);

        if let Ok(dependencies) = self
            .fetch_dependencies(&request.crate_name, target_version)
            .await
        {
            // Find the target version and add the dependencies to it
            if let Some(version_info) = crates_io_response
                .versions
                .iter_mut()
                .find(|v| v.num == target_version)
            {
                version_info.dependencies = Some(dependencies);
            }
        }

        self.transform_metadata(crates_io_response, request).await
    }

    async fn fetch_dependencies(
        &self,
        crate_name: &str,
        version: &str,
    ) -> Result<Vec<CratesIoDependency>, Error> {
        let url = format!(
            "https://crates.io/api/v1/crates/{crate_name}/{version}/dependencies"
        );

        debug!("Requesting dependencies from: {}", url);

        let response = self
            .client
            .inner_client()
            .get(&url)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to fetch dependencies from crates.io: {}", e);
                Error::Network(e)
            })?;

        let response = handle_http_response(
            response,
            &format!("crates.io dependencies for {crate_name}:{version}"),
        )
        .await?;

        let deps_response: DependenciesResponse =
            parse_json_response(response, "crates.io dependencies").await?;

        Ok(deps_response.dependencies)
    }

    async fn transform_metadata(
        &self,
        response: CratesIoResponse,
        request: &CrateMetadataRequest,
    ) -> Result<CrateMetadata, Error> {
        let crate_info = response.crate_info;

        // Find the target version or use latest
        let target_version = request
            .version
            .as_deref()
            .unwrap_or(&crate_info.max_version);

        let target_version_info = response
            .versions
            .iter()
            .find(|v| v.num == target_version)
            .cloned()
            .ok_or_else(|| {
                Error::invalid_version(format!(
                    "Version {} not found for crate {}",
                    target_version, request.crate_name
                ))
            })?;

        // Parse URLs safely
        let repository = crate_info
            .repository
            .as_ref()
            .and_then(|url_str| Url::parse(url_str).ok());

        let homepage = crate_info
            .homepage
            .as_ref()
            .and_then(|url_str| Url::parse(url_str).ok());

        let documentation = crate_info
            .documentation
            .as_ref()
            .and_then(|url_str| Url::parse(url_str).ok())
            .or_else(|| {
                // Default to docs.rs URL if no documentation URL provided
                build_basic_docs_url(&crate_info.name)
            });

        // Parse timestamps
        let created_at = chrono::DateTime::parse_from_rfc3339(&crate_info.created_at)
            .ok()
            .map(|dt| dt.with_timezone(&Utc));

        let updated_at = chrono::DateTime::parse_from_rfc3339(&crate_info.updated_at)
            .ok()
            .map(|dt| dt.with_timezone(&Utc));

        // Extract dependencies before consuming the response
        let (dependencies, dev_dependencies, build_dependencies) = self
            .categorize_dependencies(target_version_info.dependencies.as_ref().unwrap_or(&vec![]));

        // Transform version information
        let versions = response
            .versions
            .into_iter()
            .map(|v| {
                let created_at = chrono::DateTime::parse_from_rfc3339(&v.created_at)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc);

                VersionInfo {
                    num: v.num,
                    created_at,
                    yanked: v.yanked,
                    rust_version: v.rust_version,
                    downloads: v.downloads,
                    features: v.features,
                }
            })
            .collect();

        // Extract fields before move
        let license = target_version_info.license.clone();
        let version_downloads = target_version_info.downloads;
        let features = target_version_info.features.clone();
        let rust_version = target_version_info.rust_version.clone();
        let authors = self.extract_authors(&target_version_info).await;

        let metadata = CrateMetadata {
            name: crate_info.name,
            version: target_version.to_string(),
            description: crate_info.description,
            license,
            repository,
            homepage,
            documentation,
            authors,
            keywords: crate_info.keywords,
            categories: crate_info.categories,
            downloads: DownloadStats {
                total: crate_info.downloads,
                version: version_downloads,
                recent: crate_info.recent_downloads.unwrap_or(0),
            },
            versions,
            dependencies,
            dev_dependencies,
            build_dependencies,
            features,
            rust_version,
            created_at,
            updated_at,
        };

        debug!(
            "Successfully transformed metadata for {}",
            request.crate_name
        );
        Ok(metadata)
    }

    fn categorize_dependencies(
        &self,
        deps: &[CratesIoDependency],
    ) -> (Vec<Dependency>, Vec<Dependency>, Vec<Dependency>) {
        let mut normal = Vec::new();
        let mut dev = Vec::new();
        let mut build = Vec::new();

        for dep in deps.iter().cloned() {
            let dependency = Dependency::from(dep);
            match dependency.kind {
                DependencyKind::Normal => normal.push(dependency),
                DependencyKind::Dev => dev.push(dependency),
                DependencyKind::Build => build.push(dependency),
            }
        }

        (normal, dev, build)
    }

    async fn extract_authors(&self, version_info: &CratesIoVersion) -> Vec<String> {
        // For now, we'll use the publisher information if available
        // In the future, we could fetch from Cargo.toml or other sources
        if let Some(ref publisher) = version_info.published_by {
            vec![publisher
                .name
                .clone()
                .unwrap_or_else(|| publisher.login.clone())]
        } else {
            vec![]
        }
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> rustacean_docs_cache::CacheStats {
        self.cache.stats()
    }

    /// Clear the entire cache
    pub async fn clear_cache(&self) -> Result<(), rustacean_docs_core::Error> {
        let _ = self.cache.clear().await;
        Ok(())
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_client() -> DocsClient {
        DocsClient::test_client().expect("Failed to create test client")
    }

    #[test]
    fn test_metadata_cache_key() {
        let req1 = CrateMetadataRequest::new("serde");
        let req2 = CrateMetadataRequest::with_version("serde", "1.0.0");

        let key1 = MetadataCacheKey::new(&req1);
        let key2 = MetadataCacheKey::new(&req2);

        assert_eq!(key1.crate_name, "serde");
        assert_eq!(key1.version, None);
        assert_eq!(key2.crate_name, "serde");
        assert_eq!(key2.version, Some("1.0.0".to_string()));
    }

    #[tokio::test]
    async fn test_dependency_transformation() {
        let crates_dep = CratesIoDependency {
            id: 1,
            version_id: 1,
            crate_id: "serde".to_string(),
            req: "^1.0".to_string(),
            optional: false,
            default_features: true,
            features: vec!["derive".to_string()],
            target: None,
            kind: "normal".to_string(),
            downloads: Some(1000),
        };

        let dependency = Dependency::from(crates_dep);

        assert_eq!(dependency.name, "serde");
        assert_eq!(dependency.version_req, "^1.0");
        assert_eq!(dependency.features, vec!["derive"]);
        assert!(!dependency.optional);
        assert!(dependency.default_features);
        assert_eq!(dependency.target, None);
        assert_eq!(dependency.kind, DependencyKind::Normal);
    }

    #[tokio::test]
    async fn test_dependency_kinds() {
        let normal_dep = CratesIoDependency {
            id: 1,
            version_id: 1,
            crate_id: "serde".to_string(),
            req: "^1.0".to_string(),
            optional: false,
            default_features: true,
            features: vec![],
            target: None,
            kind: "normal".to_string(),
            downloads: None,
        };

        let dev_dep = CratesIoDependency {
            id: 2,
            version_id: 1,
            crate_id: "tokio-test".to_string(),
            req: "^0.4".to_string(),
            optional: false,
            default_features: true,
            features: vec![],
            target: None,
            kind: "dev".to_string(),
            downloads: None,
        };

        let build_dep = CratesIoDependency {
            id: 3,
            version_id: 1,
            crate_id: "cc".to_string(),
            req: "^1.0".to_string(),
            optional: false,
            default_features: true,
            features: vec![],
            target: None,
            kind: "build".to_string(),
            downloads: None,
        };

        assert_eq!(Dependency::from(normal_dep).kind, DependencyKind::Normal);
        assert_eq!(Dependency::from(dev_dep).kind, DependencyKind::Dev);
        assert_eq!(Dependency::from(build_dep).kind, DependencyKind::Build);
    }

    #[test]
    fn test_metadata_service_creation() {
        let client = create_test_client();
        let _service = MetadataService::new(client);

        // Basic verification that service can be created
        // Test passes by completing without panic
    }
}
