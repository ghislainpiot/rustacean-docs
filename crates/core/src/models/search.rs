use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<CrateSearchResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateSearchResult {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
}