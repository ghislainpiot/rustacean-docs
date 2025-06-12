//! Crate Overview Tool - Hierarchical Tree View of Rust Crate Contents
//!
//! This tool provides LLMs with a structured, visual overview of a Rust crate's public API
//! organized as a tree with categories and documentation paths for easy navigation.
//!
//! # Output Format
//! ```text
//! serde v1.0.219
//! ├── 📦 Modules (2)
//! │   ├── de [de/index.html] - Generic data structure deserialization framework
//! │   └── ser [ser/index.html] - Generic data structure serialization framework
//! ├── 🎯 Traits (4)
//! │   ├── Deserialize [trait.Deserialize.html] - Derive macro available...
//! │   └── Serialize [trait.Serialize.html] - Derive macro available...
//! └── ✨ Macros (3)
//!     └── forward_to_deserialize_any [macro.forward_to_deserialize_any.html] - Helper macro...
//! ```
//!
//! # Usage
//! - **Basic**: `{"crate_name": "serde"}` - Default normal detail level
//! - **Compact**: `{"crate_name": "tokio", "detail_level": "compact"}` - Names and paths only
//! - **Detailed**: `{"crate_name": "clap", "detail_level": "detailed"}` - Full signatures and visibility
//! - **Versioned**: `{"crate_name": "reqwest", "version": "0.11.24"}` - Specific version
//!
//! # Categories & Emojis
//! - 📦 Modules - Organizational units containing other items
//! - 🏗️ Structs - Data structures with named fields
//! - 🔗 Unions - C-style unions for interoperability
//! - 🔢 Enums - Algebraic data types with variants
//! - 🎯 Traits - Interfaces defining shared behavior
//! - 🔧 Functions - Standalone executable code and methods
//! - ✨ Macros - Code generation and metaprogramming
//! - 📌 Constants - Compile-time constant values
//! - 🏷️ Type Aliases - Alternative names for existing types
//!
//! # Detail Levels
//! - **compact**: Just names + paths `[...]` - fastest, minimal
//! - **normal**: Names + paths + brief descriptions - balanced (default)
//! - **detailed**: Names + paths + descriptions + visibility + async markers + signatures
//!
//! # LLM Integration Tips
//! 1. Use paths in `[brackets]` directly with `get_item_docs` tool
//! 2. Start with `compact` for large crates, then drill down
//! 3. Look for 🎯 traits to understand main interfaces
//! 4. Check 📦 modules for crate organization
//! 5. Scan 🔧 functions for entry points

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::Write as FmtWrite;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use rustacean_docs_cache::TieredCache;
use rustacean_docs_client::{endpoints::docs_modules::service::DocsService, DocsClient};
use rustacean_docs_core::{
    models::docs::{CrateDocsRequest, CrateDocsResponse, CrateItem, ItemKind},
    types::{CrateName, Version},
    Error,
};

use crate::tools::{
    CacheConfig, CacheStrategy, ErrorHandler, ParameterValidator, ToolErrorContext, ToolHandler,
    ToolInput,
};

// Type alias for our specific cache implementation
type ServerCache = TieredCache<String, Value>;

/// Detail level for the crate overview output
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DetailLevel {
    /// Just names and paths
    Compact,
    /// Names, paths, and brief descriptions (default)
    Normal,
    /// Include signatures, async indicators, visibility
    Detailed,
}

impl Default for DetailLevel {
    fn default() -> Self {
        Self::Normal
    }
}

/// Input parameters for the get_crate_overview tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateOverviewToolInput {
    /// Name of the crate (required)
    pub crate_name: String,
    /// Specific version to query (optional, defaults to latest)
    pub version: Option<String>,
    /// Level of detail in the output (optional, defaults to normal)
    pub detail_level: Option<DetailLevel>,
}

impl ToolInput for CrateOverviewToolInput {
    fn validate(&self) -> Result<(), Error> {
        ParameterValidator::validate_crate_name(&self.crate_name, "get_crate_overview")?;
        ParameterValidator::validate_version(&self.version, "get_crate_overview")?;
        Ok(())
    }

    fn cache_key(&self, tool_name: &str) -> String {
        let detail_level = self.detail_level.unwrap_or_default();
        match &self.version {
            Some(version) => {
                format!(
                    "{}:{}:{}:{:?}",
                    tool_name, self.crate_name, version, detail_level
                )
            }
            None => format!(
                "{}:{}:latest:{:?}",
                tool_name, self.crate_name, detail_level
            ),
        }
    }
}

impl CrateOverviewToolInput {
    /// Convert to internal CrateDocsRequest
    pub fn to_crate_docs_request(&self) -> Result<CrateDocsRequest, Error> {
        let crate_name = CrateName::new(&self.crate_name)
            .map_err(|e| Error::Internal(format!("Invalid crate name: {e}")))?;

        match &self.version {
            Some(version) => {
                let version = Version::new(version)
                    .map_err(|e| Error::Internal(format!("Invalid version: {e}")))?;
                Ok(CrateDocsRequest::with_version(crate_name, version))
            }
            None => Ok(CrateDocsRequest::new(crate_name)),
        }
    }
}

/// Crate overview tool that generates a tree view of crate contents
pub struct CrateOverviewTool;

impl CrateOverviewTool {
    pub fn new() -> Self {
        Self
    }

    /// Format the crate documentation as a tree structure
    fn format_as_tree(docs: &CrateDocsResponse, detail_level: DetailLevel) -> String {
        let mut output = String::new();

        // Header with crate name and version
        writeln!(&mut output, "{} v{}", docs.name, docs.version).unwrap();

        // Group items by category
        let mut modules = Vec::new();
        let mut structs = Vec::new();
        let mut enums = Vec::new();
        let mut traits = Vec::new();
        let mut functions = Vec::new();
        let mut macros = Vec::new();
        let mut constants = Vec::new();
        let mut type_aliases = Vec::new();
        let mut unions = Vec::new();

        for item in &docs.items {
            match item.kind {
                ItemKind::Module => modules.push(item),
                ItemKind::Struct => structs.push(item),
                ItemKind::Enum => enums.push(item),
                ItemKind::Trait => traits.push(item),
                ItemKind::Function => functions.push(item),
                ItemKind::Method => functions.push(item), // Methods grouped with functions
                ItemKind::Macro => macros.push(item),
                ItemKind::Constant => constants.push(item),
                ItemKind::TypeAlias => type_aliases.push(item),
                ItemKind::Union => unions.push(item),
            }
        }

        // Collect all non-empty categories in order
        let mut categories = Vec::new();

        if !modules.is_empty() {
            categories.push(("📦 Modules", modules));
        }
        if !structs.is_empty() {
            categories.push(("🏗️ Structs", structs));
        }
        if !unions.is_empty() {
            categories.push(("🔗 Unions", unions));
        }
        if !enums.is_empty() {
            categories.push(("🔢 Enums", enums));
        }
        if !traits.is_empty() {
            categories.push(("🎯 Traits", traits));
        }
        if !functions.is_empty() {
            categories.push(("🔧 Functions", functions));
        }
        if !macros.is_empty() {
            categories.push(("✨ Macros", macros));
        }
        if !constants.is_empty() {
            categories.push(("📌 Constants", constants));
        }
        if !type_aliases.is_empty() {
            categories.push(("🏷️ Type Aliases", type_aliases));
        }

        // Format each category
        let total_categories = categories.len();
        for (i, (category_name, items)) in categories.iter().enumerate() {
            let is_last_category = i == total_categories - 1;
            Self::format_category(
                &mut output,
                category_name,
                items,
                detail_level,
                is_last_category,
            );
        }

        output
    }

    /// Format a category of items
    fn format_category(
        output: &mut String,
        category_name: &str,
        items: &[&CrateItem],
        detail_level: DetailLevel,
        is_last_category: bool,
    ) {
        let prefix = if is_last_category {
            "└── "
        } else {
            "├── "
        };
        writeln!(output, "{}{} ({})", prefix, category_name, items.len()).unwrap();

        for (i, item) in items.iter().enumerate() {
            let is_last_item = i == items.len() - 1;
            let item_prefix = if is_last_category { "    " } else { "│   " };
            let item_marker = if is_last_item {
                "└── "
            } else {
                "├── "
            };

            match detail_level {
                DetailLevel::Compact => {
                    write!(output, "{item_prefix}{item_marker}{}", item.name).unwrap();
                    if let Some(path) = &item.docs_path {
                        write!(output, " [{path}]").unwrap();
                    }
                    writeln!(output).unwrap();
                }
                DetailLevel::Normal => {
                    write!(output, "{item_prefix}{item_marker}{}", item.name).unwrap();
                    if let Some(path) = &item.docs_path {
                        write!(output, " [{path}]").unwrap();
                    }
                    if let Some(summary) = &item.summary {
                        let truncated = if summary.len() > 80 {
                            format!("{}...", &summary[..77])
                        } else {
                            summary.clone()
                        };
                        write!(output, " - {truncated}").unwrap();
                    }
                    writeln!(output).unwrap();
                }
                DetailLevel::Detailed => {
                    write!(output, "{item_prefix}{item_marker}").unwrap();

                    // Add visibility indicator
                    match &item.visibility {
                        rustacean_docs_core::models::docs::Visibility::Public => {}
                        rustacean_docs_core::models::docs::Visibility::Crate => {
                            write!(output, "pub(crate) ").unwrap()
                        }
                        rustacean_docs_core::models::docs::Visibility::Module => {
                            write!(output, "pub(super) ").unwrap()
                        }
                        rustacean_docs_core::models::docs::Visibility::Private => {
                            write!(output, "private ").unwrap()
                        }
                    }

                    // Add async indicator
                    if item.is_async {
                        write!(output, "⚡ ").unwrap();
                    }

                    write!(output, "{}", item.name).unwrap();

                    if let Some(path) = &item.docs_path {
                        write!(output, " [{path}]").unwrap();
                    }

                    if let Some(signature) = &item.signature {
                        writeln!(output).unwrap();
                        let sig_prefix = if is_last_category {
                            "        "
                        } else {
                            "│       "
                        };
                        write!(output, "{sig_prefix}  {signature}").unwrap();
                    }

                    if let Some(summary) = &item.summary {
                        if item.signature.is_some() {
                            writeln!(output).unwrap();
                            let sum_prefix = if is_last_category {
                                "        "
                            } else {
                                "│       "
                            };
                            write!(output, "{sum_prefix}  // {summary}").unwrap();
                        } else {
                            write!(output, " - {summary}").unwrap();
                        }
                    }

                    writeln!(output).unwrap();
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for CrateOverviewTool {
    async fn execute(
        &self,
        params: Value,
        client: &Arc<DocsClient>,
        cache: &Arc<RwLock<ServerCache>>,
    ) -> Result<Value> {
        debug!("Executing get_crate_overview tool with params: {}", params);

        // Parse input parameters
        let input: CrateOverviewToolInput =
            serde_json::from_value(params.clone()).map_err(|e| {
                anyhow::anyhow!(
                    "{}: {}",
                    ErrorHandler::parameter_parsing_context("get_crate_overview"),
                    e
                )
            })?;

        debug!(
            crate_name = %input.crate_name,
            version = ?input.version,
            detail_level = ?input.detail_level,
            "Processing crate overview request"
        );

        // Use unified cache strategy
        CacheStrategy::execute_with_cache(
            "get_crate_overview",
            params,
            input,
            CacheConfig::default(),
            client,
            cache,
            |input, client| async move {
                // Create docs service without internal cache since we're using server-level cache
                let docs_service = DocsService::new(
                    (*client).clone(),
                    0,                                 // disable internal cache
                    std::time::Duration::from_secs(0), // no TTL needed
                );

                // Convert to docs request
                let docs_request = input.to_crate_docs_request()?;

                // Fetch documentation
                let docs_response = docs_service
                    .get_crate_docs(docs_request)
                    .await
                    .crate_context(
                        "fetch documentation",
                        &input.crate_name,
                        input.version.as_deref(),
                    )?;

                debug!(
                    crate_name = %docs_response.name,
                    version = %docs_response.version,
                    item_count = docs_response.items.len(),
                    "Crate documentation fetched successfully"
                );

                // Format as tree
                let detail_level = input.detail_level.unwrap_or_default();
                let tree_output = Self::format_as_tree(&docs_response, detail_level);

                // Return as JSON value
                Ok(serde_json::json!({
                    "overview": tree_output,
                    "crate_name": docs_response.name,
                    "version": docs_response.version,
                    "item_count": docs_response.items.len(),
                    "detail_level": detail_level,
                }))
            },
        )
        .await
    }

    fn description(&self) -> &str {
        "Get a tree-structured overview of a crate's contents with hierarchical organization. \
        Returns a visual tree showing all public items grouped by category (modules, structs, enums, traits, functions, macros, constants, type aliases). \
        Each item includes its documentation path in [brackets] for easy reference with get_item_docs. \
        \
        Detail levels: \
        - 'compact': Names and paths only \
        - 'normal': Names, paths, and brief descriptions (default) \
        - 'detailed': Includes visibility indicators, async markers, and signatures \
        \
        Categories are marked with emojis: 📦 Modules, 🏗️ Structs, 🔗 Unions, 🔢 Enums, 🎯 Traits, 🔧 Functions, ✨ Macros, 📌 Constants, 🏷️ Type Aliases. \
        Perfect for quickly understanding crate structure before diving into specific items."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "crate_name": {
                    "type": "string",
                    "description": "Name of the Rust crate to analyze. Must be available on crates.io. Examples: \"serde\", \"tokio\", \"clap\", \"reqwest\"",
                    "minLength": 1,
                    "pattern": "^[a-zA-Z0-9_-]+$",
                    "examples": ["serde", "tokio", "clap", "reqwest", "async-trait", "once_cell"]
                },
                "version": {
                    "type": "string",
                    "description": "Specific version to analyze (defaults to latest stable version). Use semantic versioning format.",
                    "examples": ["1.0.0", "0.11.4", "2.0.0-alpha.1", "1.35.0"],
                    "pattern": "^\\d+\\.\\d+\\.\\d+(?:-[a-zA-Z0-9]+(?:\\.[a-zA-Z0-9]+)*)?$"
                },
                "detail_level": {
                    "type": "string",
                    "description": "Controls the amount of information displayed for each item:\n• 'compact': Shows only item names and documentation paths - minimal, fast overview\n• 'normal': Shows names, paths, and brief descriptions - balanced view (default)\n• 'detailed': Shows names, paths, descriptions, visibility (pub/private), async indicators (⚡), and function signatures - comprehensive view",
                    "enum": ["compact", "normal", "detailed"],
                    "default": "normal",
                    "examples": ["compact", "normal", "detailed"]
                }
            },
            "required": ["crate_name"],
            "additionalProperties": false,
            "examples": [
                {
                    "crate_name": "serde",
                    "description": "Get overview of serde crate with default normal detail level"
                },
                {
                    "crate_name": "tokio",
                    "version": "1.35.0",
                    "detail_level": "compact",
                    "description": "Get compact overview of specific tokio version"
                },
                {
                    "crate_name": "clap",
                    "detail_level": "detailed",
                    "description": "Get detailed overview with signatures and visibility info"
                }
            ]
        })
    }
}

impl Default for CrateOverviewTool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_crate_overview_tool_input_validation() {
        // Valid input with version
        let valid_input = CrateOverviewToolInput {
            crate_name: "tokio".to_string(),
            version: Some("1.0.0".to_string()),
            detail_level: Some(DetailLevel::Normal),
        };
        assert!(valid_input.validate().is_ok());

        // Valid input without version
        let valid_no_version = CrateOverviewToolInput {
            crate_name: "serde".to_string(),
            version: None,
            detail_level: None,
        };
        assert!(valid_no_version.validate().is_ok());

        // Empty crate name
        let empty_crate = CrateOverviewToolInput {
            crate_name: "".to_string(),
            version: None,
            detail_level: None,
        };
        assert!(empty_crate.validate().is_err());

        // Invalid characters in crate name
        let invalid_crate = CrateOverviewToolInput {
            crate_name: "invalid/crate@name".to_string(),
            version: None,
            detail_level: None,
        };
        assert!(invalid_crate.validate().is_err());
    }

    #[test]
    fn test_crate_overview_tool_cache_key() {
        let input1 = CrateOverviewToolInput {
            crate_name: "tokio".to_string(),
            version: Some("1.0.0".to_string()),
            detail_level: Some(DetailLevel::Compact),
        };
        let key1 = input1.cache_key("crate_overview");
        assert_eq!(key1, "crate_overview:tokio:1.0.0:Compact");

        let input2 = CrateOverviewToolInput {
            crate_name: "serde".to_string(),
            version: None,
            detail_level: None,
        };
        let key2 = input2.cache_key("crate_overview");
        assert_eq!(key2, "crate_overview:serde:latest:Normal");

        // Same crate, different detail level should have different keys
        let input3 = CrateOverviewToolInput {
            crate_name: "tokio".to_string(),
            version: Some("1.0.0".to_string()),
            detail_level: Some(DetailLevel::Detailed),
        };
        let key3 = input3.cache_key("crate_overview");
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_detail_level_serialization() {
        let levels = vec![
            DetailLevel::Compact,
            DetailLevel::Normal,
            DetailLevel::Detailed,
        ];

        for level in levels {
            let json = serde_json::to_string(&level).unwrap();
            let deserialized: DetailLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(level, deserialized);
        }
    }

    #[test]
    fn test_crate_overview_tool_description() {
        let tool = CrateOverviewTool::new();
        let description = tool.description();
        assert!(!description.is_empty());
        assert!(description.contains("tree"));
        assert!(description.contains("overview"));
    }

    #[test]
    fn test_crate_overview_tool_parameters_schema() {
        let tool = CrateOverviewTool::new();
        let schema = tool.parameters_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["crate_name"].is_object());
        assert!(schema["properties"]["version"].is_object());
        assert!(schema["properties"]["detail_level"].is_object());
        assert_eq!(schema["required"][0], "crate_name");

        // Check detail_level enum values
        let detail_level_prop = &schema["properties"]["detail_level"];
        assert!(detail_level_prop["enum"].is_array());
        assert_eq!(detail_level_prop["default"], "normal");
    }

    #[test]
    fn test_crate_overview_tool_input_serialization() {
        let input = CrateOverviewToolInput {
            crate_name: "async-trait".to_string(),
            version: Some("0.1.68".to_string()),
            detail_level: Some(DetailLevel::Detailed),
        };

        // Test serialization
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(json["crate_name"], "async-trait");
        assert_eq!(json["version"], "0.1.68");
        assert_eq!(json["detail_level"], "detailed");

        // Test deserialization
        let deserialized: CrateOverviewToolInput = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized.crate_name, input.crate_name);
        assert_eq!(deserialized.version, input.version);
        assert_eq!(deserialized.detail_level, input.detail_level);
    }

    #[test]
    fn test_crate_overview_tool_input_from_json() {
        // Test with all fields
        let json_full = json!({
            "crate_name": "reqwest",
            "version": "0.11.4",
            "detail_level": "compact"
        });

        let input: CrateOverviewToolInput = serde_json::from_value(json_full).unwrap();
        assert_eq!(input.crate_name, "reqwest");
        assert_eq!(input.version, Some("0.11.4".to_string()));
        assert_eq!(input.detail_level, Some(DetailLevel::Compact));

        // Test with minimal fields
        let json_minimal = json!({
            "crate_name": "tracing"
        });

        let input: CrateOverviewToolInput = serde_json::from_value(json_minimal).unwrap();
        assert_eq!(input.crate_name, "tracing");
        assert_eq!(input.version, None);
        assert_eq!(input.detail_level, None);
    }
}
