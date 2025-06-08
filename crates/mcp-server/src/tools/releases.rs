use anyhow::Result;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, instrument};

use rustacean_docs_client::DocsClient;
use rustacean_docs_core::models::docs::RecentReleasesRequest;

use super::{ServerCache, ToolHandler};

/// Tool for retrieving recent crate releases from docs.rs
pub struct RecentReleasesTool;

impl RecentReleasesTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl ToolHandler for RecentReleasesTool {
    #[instrument(skip(self, client, _cache))]
    async fn execute(
        &self,
        params: Value,
        client: &Arc<DocsClient>,
        _cache: &Arc<RwLock<ServerCache>>,
    ) -> Result<Value> {
        debug!("Executing recent releases tool with params: {:?}", params);

        // Parse parameters
        let limit = params
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);

        // Create request
        let request = if let Some(limit) = limit {
            RecentReleasesRequest::with_limit(limit)
        } else {
            RecentReleasesRequest::new()
        };

        debug!(
            limit = request.limit(),
            "Fetching recent releases from docs.rs"
        );

        // Make the API call
        match client.get_recent_releases(request).await {
            Ok(response) => {
                debug!(
                    release_count = response.releases.len(),
                    "Successfully retrieved recent releases"
                );

                // Convert to JSON response
                let result = json!({
                    "releases": response.releases.iter().map(|release| {
                        json!({
                            "name": release.name,
                            "version": release.version,
                            "description": release.description,
                            "published_at": release.published_at.to_rfc3339(),
                            "docs_url": release.docs_url.as_ref().map(|u| u.to_string())
                        })
                    }).collect::<Vec<_>>()
                });

                Ok(result)
            }
            Err(e) => {
                error!(error = %e, "Failed to fetch recent releases");
                Err(e.into())
            }
        }
    }

    fn description(&self) -> &str {
        "Get recently updated crates from docs.rs homepage, sorted by freshness. Perfect for discovering newly published or updated crates, tracking ecosystem activity, and finding trending libraries."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "number",
                    "description": "Maximum number of releases to return (default: 20, max: 100)",
                    "minimum": 1,
                    "maximum": 100,
                    "default": 20
                }
            },
            "additionalProperties": false
        })
    }
}

impl Default for RecentReleasesTool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_recent_releases_tool_description() {
        let tool = RecentReleasesTool::new();
        let description = tool.description();
        assert!(!description.is_empty());
        assert!(description.contains("recently updated crates"));
    }

    #[test]
    fn test_recent_releases_tool_parameters_schema() {
        let tool = RecentReleasesTool::new();
        let schema = tool.parameters_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["limit"].is_object());
        assert_eq!(schema["properties"]["limit"]["type"], "number");
        assert_eq!(schema["properties"]["limit"]["minimum"], 1);
        assert_eq!(schema["properties"]["limit"]["maximum"], 100);
        assert_eq!(schema["properties"]["limit"]["default"], 20);
    }

    #[test]
    fn test_recent_releases_tool_default() {
        let tool = RecentReleasesTool::default();
        assert!(!tool.description().is_empty());
    }

    // Integration tests would require mock client setup
    #[tokio::test]
    async fn test_recent_releases_tool_params_parsing() {
        let tool = RecentReleasesTool::new();

        // Test with limit parameter
        let params_with_limit = json!({ "limit": 10 });
        // We can't actually call execute without a real client, but we can test parameter parsing logic

        // Test without limit parameter
        let params_empty = json!({});
        // Similarly, we'd need a mock setup to test the full execution
        
        // For now, just verify the tool can be created and has the right interface
        assert!(!tool.description().is_empty());
        assert!(tool.parameters_schema()["properties"]["limit"].is_object());
    }
}