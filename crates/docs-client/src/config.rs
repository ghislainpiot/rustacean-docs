use serde::{Deserialize, Serialize};

/// Configuration for HTML parsing selectors and patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HtmlParsingConfig {
    /// Selectors for extracting version information
    pub version_selectors: Vec<String>,
    /// Selectors for extracting crate descriptions
    pub description_selectors: Vec<String>,
    /// Selectors for finding navigation links
    pub navigation_selectors: Vec<String>,
    /// Selectors for finding code examples
    pub code_example_selectors: Vec<String>,
    /// Selectors for extracting item names from documentation pages
    pub item_name_selectors: Vec<String>,
    /// Selectors for determining item kinds
    pub item_kind_selectors: Vec<String>,
    /// Selectors for extracting function/type signatures
    pub signature_selectors: Vec<String>,
    /// Selectors for finding related items
    pub related_items_selectors: Vec<String>,
}

impl Default for HtmlParsingConfig {
    fn default() -> Self {
        Self {
            version_selectors: vec![
                ".version".to_string(),
                ".crate-version".to_string(),
                "h1 .version".to_string(),
                ".nav-version".to_string(),
                "[data-version]".to_string(),
            ],
            description_selectors: vec![
                ".top-doc .docblock p:first-child".to_string(),
                ".docblock:not(.item-decl) p:first-child".to_string(),
                ".item-table dt + dd".to_string(),
                "meta[name='description']".to_string(),
                ".crate-description".to_string(),
                ".description".to_string(),
                ".summary".to_string(),
            ],
            navigation_selectors: vec![
                ".item-table dt a[href]".to_string(),
                ".sidebar .block a[href]".to_string(),
                ".sidebar-elems section a[href]".to_string(),
                "#main-content .item-table a".to_string(),
                "nav.sub a[href]".to_string(),
            ],
            code_example_selectors: vec![
                "pre.rust code".to_string(),
                ".example-wrap pre code".to_string(),
                ".docblock pre.rust".to_string(),
            ],
            item_name_selectors: vec![
                "h1 .mod, h1 .struct, h1 .trait, h1 .enum, h1 .fn, h1 .macro, h1 .constant, h1 .type, h1 .union".to_string(),
                ".main-heading h1 a".to_string(),
                "h1.fqn .in-band".to_string(),
                "h1".to_string(),
                "title".to_string(),
            ],
            item_kind_selectors: vec![
                "h1 .struct".to_string(),
                "h1 .trait".to_string(),
                "h1 .enum".to_string(),
                "h1 .fn".to_string(),
                "h1 .macro".to_string(),
                "h1 .constant".to_string(),
                "h1 .type".to_string(),
                "h1 .union".to_string(),
            ],
            signature_selectors: vec![
                ".item-decl pre.rust".to_string(),
                ".docblock.item-decl".to_string(),
                ".signature".to_string(),
            ],
            related_items_selectors: vec![
                ".impl-items .method a".to_string(),
                ".trait-implementations a".to_string(),
                ".implementors a".to_string(),
            ],
        }
    }
}

/// Configuration for API item pattern matching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiItemPatterns {
    /// Type markers used in docs.rs URLs (e.g., "trait.", "struct.")
    pub item_type_markers: Vec<String>,
    /// File extension for documentation pages
    pub html_extension: String,
    /// Index file name for modules
    pub index_file: String,
}

impl Default for ApiItemPatterns {
    fn default() -> Self {
        Self {
            item_type_markers: vec![
                "trait.".to_string(),
                "struct.".to_string(),
                "enum.".to_string(),
                "fn.".to_string(),
                "macro.".to_string(),
                "derive.".to_string(),
                "constant.".to_string(),
                "type.".to_string(),
                "union.".to_string(),
            ],
            html_extension: ".html".to_string(),
            index_file: "index.html".to_string(),
        }
    }
}

/// Configuration for various URLs and endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlConfig {
    /// Base URL for docs.rs
    pub docs_rs_base: String,
    /// Base URL for crates.io API
    pub crates_io_base: String,
    /// Search endpoint path
    pub search_endpoint: String,
    /// Metadata endpoint path template (use {crate_name} placeholder)
    pub metadata_endpoint: String,
    /// Recent releases endpoint path
    pub recent_releases_endpoint: String,
}

impl Default for UrlConfig {
    fn default() -> Self {
        Self {
            docs_rs_base: "https://docs.rs".to_string(),
            crates_io_base: "https://crates.io".to_string(),
            search_endpoint: "/api/v1/crates".to_string(),
            metadata_endpoint: "/api/v1/crates/{crate_name}".to_string(),
            recent_releases_endpoint: "/api/v1/crates".to_string(),
        }
    }
}

/// Complete configuration for the docs client
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DocsClientConfig {
    /// HTML parsing configuration
    pub html_parsing: HtmlParsingConfig,
    /// API item pattern configuration
    pub api_patterns: ApiItemPatterns,
    /// URL configuration
    pub urls: UrlConfig,
}

impl DocsClientConfig {
    /// Create a new configuration with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Load configuration from a TOML string
    pub fn from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_str)
    }

    /// Serialize configuration to TOML string
    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }

    /// Load configuration from a file
    #[allow(dead_code)]
    pub fn from_file<P: AsRef<std::path::Path>>(
        path: P,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::from_toml(&content)?)
    }

    /// Save configuration to a file
    #[allow(dead_code)]
    pub fn to_file<P: AsRef<std::path::Path>>(
        &self,
        path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let content = self.to_toml()?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_configuration_creation() {
        let config = DocsClientConfig::new();

        // Test that defaults are populated
        assert!(!config.html_parsing.version_selectors.is_empty());
        assert!(!config.api_patterns.item_type_markers.is_empty());
        assert_eq!(config.urls.docs_rs_base, "https://docs.rs");
    }

    #[test]
    fn test_html_parsing_config_defaults() {
        let config = HtmlParsingConfig::default();

        assert!(config.version_selectors.contains(&".version".to_string()));
        assert!(config
            .description_selectors
            .contains(&".crate-description".to_string()));
        assert!(config
            .navigation_selectors
            .contains(&"nav a[href]".to_string()));
    }

    #[test]
    fn test_api_patterns_defaults() {
        let patterns = ApiItemPatterns::default();

        assert!(patterns.item_type_markers.contains(&"trait.".to_string()));
        assert!(patterns.item_type_markers.contains(&"struct.".to_string()));
        assert_eq!(patterns.html_extension, ".html");
        assert_eq!(patterns.index_file, "index.html");
    }

    #[test]
    fn test_url_config_defaults() {
        let urls = UrlConfig::default();

        assert_eq!(urls.docs_rs_base, "https://docs.rs");
        assert_eq!(urls.crates_io_base, "https://crates.io");
        assert_eq!(urls.search_endpoint, "/api/v1/crates");
    }

    #[test]
    fn test_toml_serialization() {
        let config = DocsClientConfig::new();
        let toml_str = config.to_toml().expect("Failed to serialize to TOML");

        assert!(toml_str.contains("[html_parsing]"));
        assert!(toml_str.contains("[api_patterns]"));
        assert!(toml_str.contains("[urls]"));

        // Test round-trip
        let parsed_config = DocsClientConfig::from_toml(&toml_str).expect("Failed to parse TOML");

        assert_eq!(config.urls.docs_rs_base, parsed_config.urls.docs_rs_base);
        assert_eq!(
            config.api_patterns.html_extension,
            parsed_config.api_patterns.html_extension
        );
    }

    #[test]
    fn test_partial_toml_config() {
        let toml_str = r#"
[html_parsing]
version_selectors = [".version", ".crate-version"]
description_selectors = [".crate-description", ".docblock p:first-child"]
navigation_selectors = ["nav a[href]", ".sidebar a[href]"]
code_example_selectors = ["pre.rust code", ".example-wrap pre code"]
item_name_selectors = ["h1.fqn .in-band", "h1 .struct, h1 .trait, h1 .enum, h1 .fn"]
item_kind_selectors = ["h1 .struct", "h1 .trait", "h1 .enum", "h1 .fn"]
signature_selectors = [".item-decl pre.rust", ".docblock.item-decl"]
related_items_selectors = [".impl-items .method a", ".trait-implementations a"]

[urls]
docs_rs_base = "https://custom-docs.rs"
crates_io_base = "https://crates.io"
search_endpoint = "/api/v1/crates"
metadata_endpoint = "/api/v1/crates/{crate_name}"
recent_releases_endpoint = "/api/v1/crates"

[api_patterns]
item_type_markers = ["trait.", "struct.", "enum.", "fn.", "macro.", "derive.", "constant.", "type.", "union."]
html_extension = ".htm"
index_file = "index.html"
"#;

        let config = DocsClientConfig::from_toml(toml_str).expect("Failed to parse partial TOML");

        // Custom values should be applied
        assert_eq!(config.urls.docs_rs_base, "https://custom-docs.rs");
        assert_eq!(config.api_patterns.html_extension, ".htm");

        // Default values should still be present where not overridden
        assert_eq!(config.urls.crates_io_base, "https://crates.io");
        assert!(!config.html_parsing.version_selectors.is_empty());
    }
}
