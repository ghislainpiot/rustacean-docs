use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateDocsRequest {
    pub crate_name: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateDocsResponse {
    pub name: String,
    pub version: String,
    pub documentation: String,
}