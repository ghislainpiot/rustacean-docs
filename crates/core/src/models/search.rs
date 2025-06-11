use crate::{constants::*, traits::*, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use url::Url;

/// Request for searching crates by name or keywords
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchRequest {
    /// Search query - can be exact crate name or descriptive keywords
    pub query: String,
    /// Maximum number of results to return (default: 10, max recommended: 50)
    pub limit: Option<usize>,
}

impl SearchRequest {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            limit: None,
        }
    }

    pub fn with_limit(query: impl Into<String>, limit: usize) -> Self {
        Self {
            query: query.into(),
            limit: Some(limit),
        }
    }

    pub fn limit(&self) -> usize {
        self.limit
            .unwrap_or(DEFAULT_SEARCH_LIMIT)
            .min(MAX_SEARCH_LIMIT)
    }
}

impl Request for SearchRequest {
    type Response = SearchResponse;

    fn validate(&self) -> Result<()> {
        if self.query.is_empty() {
            return Err(crate::ErrorBuilder::protocol()
                .invalid_input("search_crate", "Query cannot be empty"));
        }
        Ok(())
    }

    fn cache_key(&self) -> Option<String> {
        Some(format!("search:{}:{}", self.query, self.limit()))
    }
}

impl PaginatedRequest for SearchRequest {
    fn limit(&self) -> usize {
        self.limit()
    }
}

/// Response containing search results from crates.io
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchResponse {
    /// List of matching crates
    pub results: Vec<CrateSearchResult>,
    /// Total number of available results (may be higher than returned)
    pub total: Option<usize>,
}

impl SearchResponse {
    pub fn new(results: Vec<CrateSearchResult>) -> Self {
        Self {
            results,
            total: None,
        }
    }

    pub fn with_total(results: Vec<CrateSearchResult>, total: usize) -> Self {
        Self {
            results,
            total: Some(total),
        }
    }
}

impl Response for SearchResponse {
    fn cache_ttl(&self) -> Option<u64> {
        Some(DEFAULT_SEARCH_TTL)
    }
}

impl Cacheable for SearchResponse {
    fn cache_key(&self) -> String {
        format!("search_response:{}", self.results.len())
    }

    fn ttl_seconds(&self) -> u64 {
        DEFAULT_SEARCH_TTL
    }
}

/// Individual crate search result with metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrateSearchResult {
    /// Crate name
    pub name: String,
    /// Latest version
    pub version: String,
    /// Brief description of the crate
    pub description: Option<String>,
    /// Documentation URL (typically docs.rs)
    pub docs_url: Option<Url>,
    /// Download count for popularity indication
    pub download_count: Option<u64>,
    /// Last update timestamp
    pub last_updated: Option<DateTime<Utc>>,
    /// Repository URL
    pub repository: Option<Url>,
    /// Crate homepage URL
    pub homepage: Option<Url>,
    /// Keywords associated with the crate
    pub keywords: Vec<String>,
    /// Categories the crate belongs to
    pub categories: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use url::Url;

    #[test]
    fn test_search_request_new() {
        let req = SearchRequest::new("tokio");
        assert_eq!(req.query, "tokio");
        assert_eq!(req.limit, None);
        assert_eq!(req.limit(), DEFAULT_SEARCH_LIMIT);
    }

    #[test]
    fn test_search_request_with_limit() {
        let req = SearchRequest::with_limit("serde", 25);
        assert_eq!(req.query, "serde");
        assert_eq!(req.limit, Some(25));
        assert_eq!(req.limit(), 25);
    }

    #[test]
    fn test_search_request_limit_clamping() {
        let req = SearchRequest::with_limit("test", 100);
        assert_eq!(req.limit(), MAX_SEARCH_LIMIT); // Should be clamped to max
    }

    #[test]
    fn test_search_request_serialization() {
        let req = SearchRequest::with_limit("tokio", 20);
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: SearchRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, deserialized);
    }

    #[test]
    fn test_search_response_new() {
        let results = vec![];
        let response = SearchResponse::new(results.clone());
        assert_eq!(response.results, results);
        assert_eq!(response.total, None);
    }

    #[test]
    fn test_search_response_with_total() {
        let results = vec![];
        let response = SearchResponse::with_total(results.clone(), 100);
        assert_eq!(response.results, results);
        assert_eq!(response.total, Some(100));
    }

    #[test]
    fn test_crate_search_result_serialization() {
        let result = CrateSearchResult {
            name: "tokio".to_string(),
            version: "1.0.0".to_string(),
            description: Some("Async runtime".to_string()),
            docs_url: Some(Url::parse("https://docs.rs/tokio").unwrap()),
            download_count: Some(1000000),
            last_updated: Some(Utc::now()),
            repository: Some(Url::parse("https://github.com/tokio-rs/tokio").unwrap()),
            homepage: Some(Url::parse("https://tokio.rs").unwrap()),
            keywords: vec!["async".to_string(), "runtime".to_string()],
            categories: vec!["asynchronous".to_string()],
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: CrateSearchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }

    #[test]
    fn test_search_models_minimal_data() {
        let result = CrateSearchResult {
            name: "minimal".to_string(),
            version: "0.1.0".to_string(),
            description: None,
            docs_url: None,
            download_count: None,
            last_updated: None,
            repository: None,
            homepage: None,
            keywords: vec![],
            categories: vec![],
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: CrateSearchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
