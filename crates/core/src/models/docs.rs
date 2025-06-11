use crate::{
    constants::*,
    traits::*,
    types::{CrateName, ItemPath, Version},
    Result,
};
use serde::{Deserialize, Serialize};
use url::Url;

/// Request for fetching comprehensive crate documentation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrateDocsRequest {
    /// Name of the crate
    pub crate_name: CrateName,
    /// Optional version (defaults to latest)
    pub version: Option<Version>,
}

impl CrateDocsRequest {
    pub fn new(crate_name: CrateName) -> Self {
        Self {
            crate_name,
            version: None,
        }
    }

    pub fn with_version(crate_name: CrateName, version: Version) -> Self {
        Self {
            crate_name,
            version: Some(version),
        }
    }
}

impl Request for CrateDocsRequest {
    type Response = CrateDocsResponse;

    fn validate(&self) -> Result<()> {
        Ok(())
    }

    fn cache_key(&self) -> Option<String> {
        Some(format!(
            "crate_docs:{}:{}",
            self.crate_name.as_str(),
            self.version
                .as_ref()
                .map(|v| v.as_str())
                .unwrap_or("latest")
        ))
    }
}

impl VersionedRequest for CrateDocsRequest {
    fn version(&self) -> Option<&str> {
        self.version.as_ref().map(|v| v.as_str())
    }
}

/// Enhanced crate documentation with LLM-friendly structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrateDocsResponse {
    /// Crate name
    pub name: String,
    /// Version of the documentation
    pub version: String,
    /// Summary information with key metrics
    pub summary: CrateSummary,
    /// Items organized by type for easier consumption
    pub categories: CrateCategories,
    /// Complete item listings with metadata
    pub items: Vec<CrateItem>,
    /// Code examples from the documentation
    pub examples: Vec<CodeExample>,
    /// Documentation URL
    pub docs_url: Option<Url>,
}

impl Response for CrateDocsResponse {
    fn cache_ttl(&self) -> Option<u64> {
        Some(DEFAULT_CRATE_DOCS_TTL)
    }
}

impl Cacheable for CrateDocsResponse {
    fn cache_key(&self) -> String {
        format!("crate_docs:{}:{}", self.name, self.version)
    }

    fn ttl_seconds(&self) -> u64 {
        DEFAULT_CRATE_DOCS_TTL
    }
}

/// Key information and metrics about the crate
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrateSummary {
    /// Brief description
    pub description: Option<String>,
    /// Number of public modules
    pub module_count: usize,
    /// Number of public structs
    pub struct_count: usize,
    /// Number of public traits
    pub trait_count: usize,
    /// Number of public functions
    pub function_count: usize,
    /// Number of public enums
    pub enum_count: usize,
    /// Available features
    pub features: Vec<String>,
}

/// Items organized by type for better navigation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrateCategories {
    /// Core types (structs, enums, type aliases)
    pub core_types: Vec<String>,
    /// Traits and their implementations
    pub traits: Vec<String>,
    /// Modules and their structure
    pub modules: Vec<String>,
    /// Standalone functions
    pub functions: Vec<String>,
    /// Macros
    pub macros: Vec<String>,
    /// Constants
    pub constants: Vec<String>,
}

/// Individual documentation item with metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrateItem {
    /// Item name
    pub name: String,
    /// Item type (struct, trait, function, etc.)
    pub kind: ItemKind,
    /// Brief description
    pub summary: Option<String>,
    /// Full documentation path
    pub path: String,
    /// Visibility (pub, pub(crate), etc.)
    pub visibility: Visibility,
    /// Whether the item is async (for functions)
    pub is_async: bool,
    /// Type information (for functions, methods)
    pub signature: Option<String>,
    /// Documentation URL fragment
    pub docs_path: Option<String>,
}

/// Type of documentation item
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemKind {
    Module,
    Struct,
    Enum,
    Trait,
    Function,
    Method,
    Macro,
    Constant,
    TypeAlias,
    Union,
}

/// Visibility level of an item
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Crate,
    Module,
    Private,
}

/// Code example with context
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodeExample {
    /// Example title or context
    pub title: Option<String>,
    /// Code content
    pub code: String,
    /// Programming language (usually "rust")
    pub language: String,
    /// Whether the example is runnable
    pub is_runnable: bool,
}

/// Request for specific item documentation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ItemDocsRequest {
    /// Name of the crate
    pub crate_name: CrateName,
    /// Item identifier - can be simple name or full path
    pub item_path: ItemPath,
    /// Specific version to query (defaults to latest)
    pub version: Option<Version>,
}

impl ItemDocsRequest {
    pub fn new(crate_name: CrateName, item_path: ItemPath) -> Self {
        Self {
            crate_name,
            item_path,
            version: None,
        }
    }

    pub fn with_version(crate_name: CrateName, item_path: ItemPath, version: Version) -> Self {
        Self {
            crate_name,
            item_path,
            version: Some(version),
        }
    }
}

impl Request for ItemDocsRequest {
    type Response = ItemDocsResponse;

    fn validate(&self) -> Result<()> {
        Ok(())
    }

    fn cache_key(&self) -> Option<String> {
        Some(format!(
            "item_docs:{}:{}:{}",
            self.crate_name.as_str(),
            self.item_path.as_str(),
            self.version
                .as_ref()
                .map(|v| v.as_str())
                .unwrap_or("latest")
        ))
    }
}

impl VersionedRequest for ItemDocsRequest {
    fn version(&self) -> Option<&str> {
        self.version.as_ref().map(|v| v.as_str())
    }
}

/// Detailed documentation for a specific item
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ItemDocsResponse {
    /// Crate name
    pub crate_name: String,
    /// Item path
    pub item_path: String,
    /// Item name
    pub name: String,
    /// Item type
    pub kind: ItemKind,
    /// Function/type signature or definition
    pub signature: Option<String>,
    /// Detailed description
    pub description: Option<String>,
    /// Code examples specific to this item
    pub examples: Vec<CodeExample>,
    /// Documentation URL
    pub docs_url: Option<Url>,
    /// Related items (implementations, traits, etc.)
    pub related_items: Vec<String>,
}

impl Response for ItemDocsResponse {
    fn cache_ttl(&self) -> Option<u64> {
        Some(DEFAULT_ITEM_DOCS_TTL)
    }
}

impl Cacheable for ItemDocsResponse {
    fn cache_key(&self) -> String {
        format!("item_docs:{}:{}", self.crate_name, self.item_path)
    }

    fn ttl_seconds(&self) -> u64 {
        DEFAULT_ITEM_DOCS_TTL
    }
}

/// Recent releases information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecentReleasesRequest {
    /// Maximum number of releases to return (default: 20, max: 100)
    pub limit: Option<usize>,
}

impl RecentReleasesRequest {
    pub fn new() -> Self {
        Self { limit: None }
    }

    pub fn with_limit(limit: usize) -> Self {
        Self { limit: Some(limit) }
    }

    pub fn limit(&self) -> usize {
        self.limit
            .unwrap_or(DEFAULT_RECENT_RELEASES_LIMIT)
            .min(MAX_RECENT_RELEASES_LIMIT)
    }
}

impl Request for RecentReleasesRequest {
    type Response = RecentReleasesResponse;

    fn validate(&self) -> Result<()> {
        Ok(())
    }

    fn cache_key(&self) -> Option<String> {
        Some(format!("recent_releases:{}", self.limit()))
    }
}

impl PaginatedRequest for RecentReleasesRequest {
    fn limit(&self) -> usize {
        self.limit()
    }
}

impl Default for RecentReleasesRequest {
    fn default() -> Self {
        Self::new()
    }
}

/// Response containing recent crate releases
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecentReleasesResponse {
    /// List of recent releases
    pub releases: Vec<CrateRelease>,
}

impl Response for RecentReleasesResponse {
    fn cache_ttl(&self) -> Option<u64> {
        Some(DEFAULT_RECENT_RELEASES_TTL)
    }
}

impl Cacheable for RecentReleasesResponse {
    fn cache_key(&self) -> String {
        format!("recent_releases:{}", self.releases.len())
    }

    fn ttl_seconds(&self) -> u64 {
        DEFAULT_RECENT_RELEASES_TTL
    }
}

/// Information about a recent crate release
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrateRelease {
    /// Crate name
    pub name: String,
    /// Version number
    pub version: String,
    /// Brief description
    pub description: Option<String>,
    /// Publication date
    pub published_at: chrono::DateTime<chrono::Utc>,
    /// Documentation URL
    pub docs_url: Option<Url>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use url::Url;

    #[test]
    fn test_crate_docs_request_new() {
        let crate_name = CrateName::new("tokio").unwrap();
        let req = CrateDocsRequest::new(crate_name.clone());
        assert_eq!(req.crate_name, crate_name);
        assert_eq!(req.version, None);
    }

    #[test]
    fn test_crate_docs_request_with_version() {
        let crate_name = CrateName::new("tokio").unwrap();
        let version = Version::new("1.0.0").unwrap();
        let req = CrateDocsRequest::with_version(crate_name.clone(), version.clone());
        assert_eq!(req.crate_name, crate_name);
        assert_eq!(req.version, Some(version));
    }

    #[test]
    fn test_item_docs_request_new() {
        let crate_name = CrateName::new("tokio").unwrap();
        let item_path = ItemPath::new("spawn").unwrap();
        let req = ItemDocsRequest::new(crate_name.clone(), item_path.clone());
        assert_eq!(req.crate_name, crate_name);
        assert_eq!(req.item_path, item_path);
        assert_eq!(req.version, None);
    }

    #[test]
    fn test_item_docs_request_with_version() {
        let crate_name = CrateName::new("tokio").unwrap();
        let item_path = ItemPath::new("spawn").unwrap();
        let version = Version::new("1.0.0").unwrap();
        let req =
            ItemDocsRequest::with_version(crate_name.clone(), item_path.clone(), version.clone());
        assert_eq!(req.crate_name, crate_name);
        assert_eq!(req.item_path, item_path);
        assert_eq!(req.version, Some(version));
    }

    #[test]
    fn test_recent_releases_request_new() {
        let req = RecentReleasesRequest::new();
        assert_eq!(req.limit, None);
        assert_eq!(req.limit(), DEFAULT_RECENT_RELEASES_LIMIT);
    }

    #[test]
    fn test_recent_releases_request_with_limit() {
        let req = RecentReleasesRequest::with_limit(50);
        assert_eq!(req.limit, Some(50));
        assert_eq!(req.limit(), 50);
    }

    #[test]
    fn test_recent_releases_request_limit_clamping() {
        let req = RecentReleasesRequest::with_limit(200);
        assert_eq!(req.limit(), MAX_RECENT_RELEASES_LIMIT); // Should be clamped to max
    }

    #[test]
    fn test_recent_releases_request_default() {
        let req = RecentReleasesRequest::default();
        assert_eq!(req.limit, None);
        assert_eq!(req.limit(), DEFAULT_RECENT_RELEASES_LIMIT);
    }

    #[test]
    fn test_item_kind_serialization() {
        let kinds = vec![
            ItemKind::Module,
            ItemKind::Struct,
            ItemKind::Enum,
            ItemKind::Trait,
            ItemKind::Function,
            ItemKind::Method,
            ItemKind::Macro,
            ItemKind::Constant,
            ItemKind::TypeAlias,
            ItemKind::Union,
        ];

        for kind in kinds {
            let json = serde_json::to_string(&kind).unwrap();
            let deserialized: ItemKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, deserialized);
        }
    }

    #[test]
    fn test_visibility_serialization() {
        let visibilities = vec![
            Visibility::Public,
            Visibility::Crate,
            Visibility::Module,
            Visibility::Private,
        ];

        for visibility in visibilities {
            let json = serde_json::to_string(&visibility).unwrap();
            let deserialized: Visibility = serde_json::from_str(&json).unwrap();
            assert_eq!(visibility, deserialized);
        }
    }

    #[test]
    fn test_code_example_serialization() {
        let example = CodeExample {
            title: Some("Basic usage".to_string()),
            code: "use tokio;\n\n#[tokio::main]\nasync fn main() {}".to_string(),
            language: "rust".to_string(),
            is_runnable: true,
        };

        let json = serde_json::to_string(&example).unwrap();
        let deserialized: CodeExample = serde_json::from_str(&json).unwrap();
        assert_eq!(example, deserialized);
    }

    #[test]
    fn test_crate_summary_serialization() {
        let summary = CrateSummary {
            description: Some("An async runtime".to_string()),
            module_count: 5,
            struct_count: 20,
            trait_count: 8,
            function_count: 50,
            enum_count: 3,
            features: vec!["default".to_string(), "full".to_string()],
        };

        let json = serde_json::to_string(&summary).unwrap();
        let deserialized: CrateSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(summary, deserialized);
    }

    #[test]
    fn test_crate_item_serialization() {
        let item = CrateItem {
            name: "spawn".to_string(),
            kind: ItemKind::Function,
            summary: Some("Spawn a new task".to_string()),
            path: "tokio::spawn".to_string(),
            visibility: Visibility::Public,
            is_async: false,
            signature: Some("pub fn spawn<T>(future: T) -> JoinHandle<T::Output>".to_string()),
            docs_path: Some("fn.spawn.html".to_string()),
        };

        let json = serde_json::to_string(&item).unwrap();
        let deserialized: CrateItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, deserialized);
    }

    #[test]
    fn test_crate_release_serialization() {
        let release = CrateRelease {
            name: "tokio".to_string(),
            version: "1.35.0".to_string(),
            description: Some(
                "A runtime for writing reliable, asynchronous applications".to_string(),
            ),
            published_at: Utc::now(),
            docs_url: Some(Url::parse("https://docs.rs/tokio/1.35.0").unwrap()),
        };

        let json = serde_json::to_string(&release).unwrap();
        let deserialized: CrateRelease = serde_json::from_str(&json).unwrap();
        assert_eq!(release, deserialized);
    }

    #[test]
    fn test_docs_models_minimal_data() {
        let response = CrateDocsResponse {
            name: "minimal".to_string(),
            version: "0.1.0".to_string(),
            summary: CrateSummary {
                description: None,
                module_count: 0,
                struct_count: 0,
                trait_count: 0,
                function_count: 0,
                enum_count: 0,
                features: vec![],
            },
            categories: CrateCategories {
                core_types: vec![],
                traits: vec![],
                modules: vec![],
                functions: vec![],
                macros: vec![],
                constants: vec![],
            },
            items: vec![],
            examples: vec![],
            docs_url: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: CrateDocsResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(response, deserialized);
    }
}
