use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, trace};

use rustacean_docs_cache::TieredCache;
use rustacean_docs_client::DocsClient;
use rustacean_docs_core::{
    error::ErrorContext,
    models::metadata::{CrateMetadata, CrateMetadataRequest},
    Error,
};

use crate::tools::ToolHandler;

// Type alias for our specific cache implementation
type ServerCache = TieredCache<String, Value>;

/// Input parameters for the get_crate_metadata tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataToolInput {
    /// Name of the crate (e.g., "tokio")
    pub crate_name: String,
    /// Specific version to query (defaults to latest stable version)
    pub version: Option<String>,
}

impl MetadataToolInput {
    /// Validate the input parameters
    pub fn validate(&self) -> Result<(), Error> {
        if self.crate_name.trim().is_empty() {
            return Err(Error::invalid_input(
                "get_crate_metadata",
                "crate_name cannot be empty",
            ));
        }

        // Validate version format if provided
        if let Some(ref version) = self.version {
            if version.trim().is_empty() {
                return Err(Error::invalid_input(
                    "get_crate_metadata",
                    "version cannot be empty if provided",
                ));
            }
        }

        Ok(())
    }

    /// Convert to internal request format
    pub fn to_request(&self) -> CrateMetadataRequest {
        match &self.version {
            Some(version) => CrateMetadataRequest::with_version(&self.crate_name, version),
            None => CrateMetadataRequest::new(&self.crate_name),
        }
    }
}

/// Tool for fetching comprehensive crate metadata
pub struct CrateMetadataTool;

impl CrateMetadataTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CrateMetadataTool {
    fn default() -> Self {
        Self::new()
    }
}

impl CrateMetadataTool {
    fn format_metadata_response(&self, metadata: &CrateMetadata) -> Value {
        json!({
            "summary": {
                "name": metadata.name,
                "version": metadata.version,
                "description": metadata.description,
                "license": metadata.license,
                "downloads": {
                    "total": metadata.downloads.total,
                    "version": metadata.downloads.version,
                    "recent": metadata.downloads.recent
                },
                "rust_version": metadata.rust_version,
                "created_at": metadata.created_at,
                "updated_at": metadata.updated_at
            },
            "project": {
                "repository": metadata.repository.as_ref().map(|u| u.as_str()),
                "homepage": metadata.homepage.as_ref().map(|u| u.as_str()),
                "documentation": metadata.documentation.as_ref().map(|u| u.as_str()),
                "authors": metadata.authors,
                "keywords": metadata.keywords,
                "categories": metadata.categories
            },
            "dependencies": {
                "count": metadata.dependencies.len(),
                "list": metadata.dependencies.iter().map(|dep| {
                    json!({
                        "name": dep.name,
                        "version": dep.version_req,
                        "optional": dep.optional,
                        "default_features": dep.default_features,
                        "features": dep.features,
                        "target": dep.target
                    })
                }).collect::<Vec<_>>()
            },
            "dev_dependencies": {
                "count": metadata.dev_dependencies.len(),
                "list": metadata.dev_dependencies.iter().map(|dep| {
                    json!({
                        "name": dep.name,
                        "version": dep.version_req,
                        "optional": dep.optional,
                        "features": dep.features,
                        "target": dep.target
                    })
                }).collect::<Vec<_>>()
            },
            "build_dependencies": {
                "count": metadata.build_dependencies.len(),
                "list": metadata.build_dependencies.iter().map(|dep| {
                    json!({
                        "name": dep.name,
                        "version": dep.version_req,
                        "features": dep.features,
                        "target": dep.target
                    })
                }).collect::<Vec<_>>()
            },
            "features": metadata.features,
            "versions": {
                "count": metadata.versions.len(),
                "latest": metadata.versions.first().map(|v| &v.num),
                "list": metadata.versions.iter().take(10).map(|version| {
                    json!({
                        "num": version.num,
                        "created_at": version.created_at,
                        "yanked": version.yanked,
                        "downloads": version.downloads,
                        "rust_version": version.rust_version
                    })
                }).collect::<Vec<_>>()
            },
            "ecosystem": self.analyze_ecosystem(metadata)
        })
    }

    fn analyze_ecosystem(&self, metadata: &CrateMetadata) -> Value {
        let dep_count = metadata.dependencies.len();
        let dev_dep_count = metadata.dev_dependencies.len();
        let build_dep_count = metadata.build_dependencies.len();
        let total_deps = dep_count + dev_dep_count + build_dep_count;

        // Analyze dependency patterns
        let async_deps = metadata
            .dependencies
            .iter()
            .chain(metadata.dev_dependencies.iter())
            .any(|dep| {
                dep.name.contains("tokio")
                    || dep.name.contains("async")
                    || dep.name.contains("futures")
            });

        let web_deps = metadata
            .dependencies
            .iter()
            .chain(metadata.dev_dependencies.iter())
            .any(|dep| {
                dep.name.contains("reqwest")
                    || dep.name.contains("hyper")
                    || dep.name.contains("axum")
                    || dep.name.contains("warp")
                    || dep.name.contains("actix")
            });

        let serde_deps = metadata
            .dependencies
            .iter()
            .chain(metadata.dev_dependencies.iter())
            .any(|dep| dep.name.contains("serde"));

        // Categorize crate complexity
        let complexity = if total_deps == 0 {
            "minimal"
        } else if total_deps < 5 {
            "simple"
        } else if total_deps < 20 {
            "moderate"
        } else {
            "complex"
        };

        // Popularity assessment
        let popularity = if metadata.downloads.total > 10_000_000 {
            "very_high"
        } else if metadata.downloads.total > 1_000_000 {
            "high"
        } else if metadata.downloads.total > 100_000 {
            "moderate"
        } else if metadata.downloads.total > 10_000 {
            "low"
        } else {
            "very_low"
        };

        json!({
            "dependency_analysis": {
                "total_dependencies": total_deps,
                "runtime_dependencies": dep_count,
                "dev_dependencies": dev_dep_count,
                "build_dependencies": build_dep_count,
                "complexity": complexity,
                "patterns": {
                    "async_programming": async_deps,
                    "web_framework": web_deps,
                    "serialization": serde_deps
                }
            },
            "popularity": {
                "level": popularity,
                "total_downloads": metadata.downloads.total,
                "recent_downloads": metadata.downloads.recent,
                "version_downloads": metadata.downloads.version
            },
            "maintenance": {
                "versions_count": metadata.versions.len(),
                "yanked_versions": metadata.versions.iter().filter(|v| v.yanked).count(),
                "latest_version": metadata.version,
                "last_updated": metadata.updated_at
            }
        })
    }
}

#[async_trait::async_trait]
impl ToolHandler for CrateMetadataTool {
    async fn execute(
        &self,
        params: Value,
        _client: &Arc<DocsClient>,
        _cache: &Arc<RwLock<ServerCache>>,
    ) -> Result<Value> {
        debug!("Executing get_crate_metadata tool with params: {}", params);

        // Parse and validate input
        let input: MetadataToolInput = serde_json::from_value(params)
            .with_context(|| "Invalid input parameters for get_crate_metadata".to_string())?;

        input.validate()?;

        trace!("Validated input for crate: {}", input.crate_name);

        // Convert to request
        let request = input.to_request();

        // Use the docs client to fetch metadata (for now, we'll mock this)
        // TODO: Implement actual metadata fetching once the endpoint is ready
        debug!(
            "Fetching metadata for crate: {} (version: {:?})",
            request.crate_name, request.version
        );

        // For now, return a mock response indicating the feature is coming soon
        let mock_metadata = self.create_mock_metadata(&request);
        let formatted_response = self.format_metadata_response(&mock_metadata);

        Ok(json!({
            "status": "success",
            "crate_name": request.crate_name,
            "version": request.version.unwrap_or_else(|| "latest".to_string()),
            "metadata": formatted_response,
            "note": "Metadata fetching implementation is in progress. This is mock data for testing."
        }))
    }

    fn description(&self) -> &str {
        "Fetch comprehensive crate metadata including license, repository, dependencies, and ecosystem analysis. Essential for understanding crate project information, legal compliance, and integration requirements."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "crate_name": {
                    "type": "string",
                    "description": "Name of the crate (e.g., \"tokio\")"
                },
                "version": {
                    "type": "string",
                    "description": "Specific version to query (defaults to latest stable version)"
                }
            },
            "required": ["crate_name"]
        })
    }
}

impl CrateMetadataTool {
    fn create_mock_metadata(&self, request: &CrateMetadataRequest) -> CrateMetadata {
        use chrono::Utc;
        use rustacean_docs_core::models::metadata::{
            Dependency, DependencyKind, DownloadStats, VersionInfo,
        };
        use std::collections::HashMap;

        let mut features = HashMap::new();
        features.insert("default".to_string(), vec!["std".to_string()]);

        CrateMetadata {
            name: request.crate_name.clone(),
            version: request
                .version
                .clone()
                .unwrap_or_else(|| "1.0.0".to_string()),
            description: Some(format!("Mock description for {}", request.crate_name)),
            license: Some("MIT".to_string()),
            repository: None,
            homepage: None,
            documentation: None,
            authors: vec!["Mock Author".to_string()],
            keywords: vec!["rust".to_string(), "library".to_string()],
            categories: vec!["development-tools".to_string()],
            downloads: DownloadStats {
                total: 100000,
                version: 5000,
                recent: 1000,
            },
            versions: vec![VersionInfo {
                num: "1.0.0".to_string(),
                created_at: Utc::now(),
                yanked: false,
                rust_version: Some("1.70.0".to_string()),
                downloads: 5000,
                features: features.clone(),
            }],
            dependencies: vec![Dependency {
                name: "serde".to_string(),
                version_req: "^1.0".to_string(),
                features: vec!["derive".to_string()],
                optional: false,
                default_features: true,
                target: None,
                kind: DependencyKind::Normal,
            }],
            dev_dependencies: vec![],
            build_dependencies: vec![],
            features,
            rust_version: Some("1.70.0".to_string()),
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_metadata_tool_input_validation() {
        // Valid input
        let valid_input = MetadataToolInput {
            crate_name: "tokio".to_string(),
            version: Some("1.0.0".to_string()),
        };
        assert!(valid_input.validate().is_ok());

        // Empty crate name
        let invalid_input = MetadataToolInput {
            crate_name: "".to_string(),
            version: None,
        };
        assert!(invalid_input.validate().is_err());

        // Empty version
        let invalid_version = MetadataToolInput {
            crate_name: "tokio".to_string(),
            version: Some("".to_string()),
        };
        assert!(invalid_version.validate().is_err());
    }

    #[test]
    fn test_metadata_tool_input_to_request() {
        let input = MetadataToolInput {
            crate_name: "serde".to_string(),
            version: Some("1.0.0".to_string()),
        };

        let request = input.to_request();
        assert_eq!(request.crate_name, "serde");
        assert_eq!(request.version, Some("1.0.0".to_string()));

        let input_no_version = MetadataToolInput {
            crate_name: "tokio".to_string(),
            version: None,
        };

        let request_no_version = input_no_version.to_request();
        assert_eq!(request_no_version.crate_name, "tokio");
        assert_eq!(request_no_version.version, None);
    }

    #[test]
    fn test_tool_descriptions() {
        let metadata_tool = CrateMetadataTool::new();

        assert!(!metadata_tool.description().is_empty());
    }

    #[test]
    fn test_tool_parameter_schemas() {
        let metadata_tool = CrateMetadataTool::new();

        let metadata_schema = metadata_tool.parameters_schema();
        assert!(metadata_schema["properties"]["crate_name"].is_object());
        assert_eq!(metadata_schema["required"], json!(["crate_name"]));
    }

    #[test]
    fn test_ecosystem_analysis() {
        let tool = CrateMetadataTool::new();
        let request = CrateMetadataRequest::new("test");
        let mock_metadata = tool.create_mock_metadata(&request);

        let analysis = tool.analyze_ecosystem(&mock_metadata);
        assert!(analysis["dependency_analysis"].is_object());
        assert!(analysis["popularity"].is_object());
        assert!(analysis["maintenance"].is_object());
    }
}
