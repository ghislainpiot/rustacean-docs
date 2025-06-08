use serde::{Deserialize, Serialize};
use url::Url;

/// Request for fetching comprehensive crate documentation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrateDocsRequest {
    /// Name of the crate
    pub crate_name: String,
    /// Optional version (defaults to latest)
    pub version: Option<String>,
}

impl CrateDocsRequest {
    pub fn new(crate_name: impl Into<String>) -> Self {
        Self {
            crate_name: crate_name.into(),
            version: None,
        }
    }

    pub fn with_version(crate_name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            crate_name: crate_name.into(),
            version: Some(version.into()),
        }
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    pub crate_name: String,
    /// Item identifier - can be simple name or full path
    pub item_path: String,
    /// Specific version to query (defaults to latest)
    pub version: Option<String>,
}

impl ItemDocsRequest {
    pub fn new(crate_name: impl Into<String>, item_path: impl Into<String>) -> Self {
        Self {
            crate_name: crate_name.into(),
            item_path: item_path.into(),
            version: None,
        }
    }

    pub fn with_version(
        crate_name: impl Into<String>,
        item_path: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            crate_name: crate_name.into(),
            item_path: item_path.into(),
            version: Some(version.into()),
        }
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
        self.limit.unwrap_or(20).min(100)
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
        let req = CrateDocsRequest::new("tokio");
        assert_eq!(req.crate_name, "tokio");
        assert_eq!(req.version, None);
    }

    #[test]
    fn test_crate_docs_request_with_version() {
        let req = CrateDocsRequest::with_version("tokio", "1.0.0");
        assert_eq!(req.crate_name, "tokio");
        assert_eq!(req.version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_item_docs_request_new() {
        let req = ItemDocsRequest::new("tokio", "spawn");
        assert_eq!(req.crate_name, "tokio");
        assert_eq!(req.item_path, "spawn");
        assert_eq!(req.version, None);
    }

    #[test]
    fn test_item_docs_request_with_version() {
        let req = ItemDocsRequest::with_version("tokio", "spawn", "1.0.0");
        assert_eq!(req.crate_name, "tokio");
        assert_eq!(req.item_path, "spawn");
        assert_eq!(req.version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_recent_releases_request_new() {
        let req = RecentReleasesRequest::new();
        assert_eq!(req.limit, None);
        assert_eq!(req.limit(), 20);
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
        assert_eq!(req.limit(), 100); // Should be clamped to max 100
    }

    #[test]
    fn test_recent_releases_request_default() {
        let req = RecentReleasesRequest::default();
        assert_eq!(req.limit, None);
        assert_eq!(req.limit(), 20);
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
