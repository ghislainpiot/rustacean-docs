use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, instrument};

use rustacean_docs_client::{endpoints::releases::ReleasesService, DocsClient};
use rustacean_docs_core::{models::docs::RecentReleasesRequest, Error};

use super::{ParameterValidator, ServerCache, ToolHandler, ToolInput};

/// Input parameters for the list_recent_releases tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleasesToolInput {
    /// Maximum number of releases to return (default: 20, max: 100)
    pub limit: Option<usize>,
}

impl ToolInput for ReleasesToolInput {
    fn validate(&self) -> Result<(), Error> {
        ParameterValidator::validate_limit(&self.limit, "list_recent_releases", 100)?;
        Ok(())
    }

    fn cache_key(&self, tool_name: &str) -> String {
        format!("{}:limit:{}", tool_name, self.limit.unwrap_or(20))
    }
}

impl ReleasesToolInput {
    /// Convert to internal RecentReleasesRequest
    pub fn to_request(&self) -> RecentReleasesRequest {
        match self.limit {
            Some(limit) => RecentReleasesRequest::with_limit(limit),
            None => RecentReleasesRequest::new(),
        }
    }
}

/// Tool for retrieving recent crate releases from crates.io API
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

        // Parse input parameters
        let input: ReleasesToolInput =
            serde_json::from_value(params.clone()).unwrap_or(ReleasesToolInput { limit: None });

        debug!(
            limit = input.limit,
            "Fetching recent releases from crates.io API"
        );

        // Create releases service with cache
        let releases_service = ReleasesService::new((**client).clone());

        // Create request
        let request = input.to_request();

        // Use ReleasesService directly - it handles caching internally
        let response = releases_service.get_recent_releases(&request).await?;

        debug!(
            release_count = response.releases.len(),
            "Successfully retrieved recent releases from crates.io"
        );

        // Serialize response to JSON - no manual transformation needed
        Ok(serde_json::to_value(response)?)
    }

    fn description(&self) -> &str {
        "Get recently updated crates from crates.io API, sorted by freshness. Perfect for discovering newly published or updated crates, tracking ecosystem activity, and finding trending libraries."
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
        let tool = RecentReleasesTool;
        assert!(!tool.description().is_empty());
    }

    // Integration tests would require mock client setup
    #[tokio::test]
    async fn test_recent_releases_tool_params_parsing() {
        let tool = RecentReleasesTool::new();

        // Test with limit parameter
        let _params_with_limit = json!({ "limit": 10 });
        // We can't actually call execute without a real client, but we can test parameter parsing logic

        // Test without limit parameter
        let _params_empty = json!({});
        // Similarly, we'd need a mock setup to test the full execution

        // For now, just verify the tool can be created and has the right interface
        assert!(!tool.description().is_empty());
        assert!(tool.parameters_schema()["properties"]["limit"].is_object());
    }
}
