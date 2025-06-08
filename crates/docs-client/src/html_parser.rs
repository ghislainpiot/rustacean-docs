use crate::config::{ApiItemPatterns, HtmlParsingConfig};
use rustacean_docs_core::models::docs::CodeExample;
use scraper::{ElementRef, Html, Selector};
use tracing::trace;

/// Centralized HTML parser utility for docs.rs content
pub struct HtmlParser {
    document: Html,
    html_config: HtmlParsingConfig,
    api_patterns: ApiItemPatterns,
}

impl HtmlParser {
    /// Create a new HTML parser from HTML content with default configuration
    pub fn new(html: &str) -> Self {
        Self::with_config(
            html,
            HtmlParsingConfig::default(),
            ApiItemPatterns::default(),
        )
    }

    /// Create a new HTML parser with custom configuration
    pub fn with_config(
        html: &str,
        html_config: HtmlParsingConfig,
        api_patterns: ApiItemPatterns,
    ) -> Self {
        Self {
            document: Html::parse_document(html),
            html_config,
            api_patterns,
        }
    }

    /// Get a reference to the parsed document
    pub fn document(&self) -> &Html {
        &self.document
    }

    /// Extract elements matching any of the given selectors
    pub fn extract_by_selectors(&self, selectors: &[&str]) -> Vec<ElementRef<'_>> {
        let mut elements = Vec::new();

        for selector_str in selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                elements.extend(self.document.select(&selector));
            }
        }

        elements
    }

    /// Extract the first element matching any of the given selectors
    pub fn extract_first_by_selectors(&self, selectors: &[&str]) -> Option<ElementRef<'_>> {
        for selector_str in selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = self.document.select(&selector).next() {
                    return Some(element);
                }
            }
        }
        None
    }

    /// Extract trimmed text content from an element
    pub fn extract_text_from_element(element: &ElementRef) -> Option<String> {
        let text = element.text().collect::<String>().trim().to_string();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    /// Extract trimmed text from the first matching selector
    pub fn extract_text_by_selectors(&self, selectors: &[&str]) -> Option<String> {
        self.extract_first_by_selectors(selectors)
            .and_then(|element| Self::extract_text_from_element(&element))
    }

    /// Extract href attribute from an element
    pub fn extract_href_from_element(element: &ElementRef) -> Option<String> {
        element.value().attr("href").map(|s| s.to_string())
    }

    /// Extract title attribute from an element
    pub fn extract_title_from_element(element: &ElementRef) -> Option<String> {
        element.value().attr("title").map(|s| s.to_string())
    }

    /// Extract version from page using configured patterns
    pub fn extract_version(&self) -> Option<String> {
        let version_selectors: Vec<&str> = self
            .html_config
            .version_selectors
            .iter()
            .map(|s| s.as_str())
            .collect();

        // Try direct version selectors first
        if let Some(version) = self.extract_text_by_selectors(&version_selectors) {
            return Some(version.trim_start_matches('v').to_string());
        }

        // Try extracting from title or meta tags
        let title_selectors = ["title", "meta[name='description']"];
        for selector_str in &title_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = self.document.select(&selector).next() {
                    let content = if selector_str.starts_with("meta") {
                        element.value().attr("content").unwrap_or("")
                    } else {
                        &element.text().collect::<String>()
                    };

                    // Look for version patterns in content
                    if let Some(version) = self.extract_version_from_text(content) {
                        return Some(version);
                    }
                }
            }
        }

        None
    }

    /// Extract description using configured selectors
    pub fn extract_description(&self) -> Option<String> {
        for selector_str in &self.html_config.description_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = self.document.select(&selector).next() {
                    let text = if selector_str.starts_with("meta") {
                        element.value().attr("content").unwrap_or("").to_string()
                    } else {
                        element.text().collect::<String>().trim().to_string()
                    };

                    if !text.is_empty() {
                        return Some(text);
                    }
                }
            }
        }

        None
    }

    /// Extract navigation links that match API item patterns
    pub fn extract_api_links(&self) -> Vec<(String, String)> {
        let mut links = Vec::new();

        let nav_selectors: Vec<&str> = self
            .html_config
            .navigation_selectors
            .iter()
            .map(|s| s.as_str())
            .collect();

        for element in self.extract_by_selectors(&nav_selectors) {
            if let Some(href) = Self::extract_href_from_element(&element) {
                trace!(href = %href, "Found link in navigation");

                if self.is_api_item_href(&href) {
                    if let Some(text) = Self::extract_text_from_element(&element) {
                        trace!(href = %href, text = %text, "Link matches API item pattern");
                        links.push((text, href));
                    }
                }
            }
        }

        links
    }

    /// Check if an href points to an actual API item using configured patterns
    pub fn is_api_item_href(&self, href: &str) -> bool {
        // Skip external links, anchors, and non-API paths
        if href.starts_with("http") || href.starts_with("//") || href.starts_with('#') {
            return false;
        }

        // Check against configured API item patterns
        for pattern in &self.api_patterns.item_type_markers {
            if href.contains(pattern) && href.ends_with(&self.api_patterns.html_extension) {
                return true;
            }
        }

        // Check for index files (modules)
        if href.ends_with(&format!("/{}", self.api_patterns.index_file))
            && !href.starts_with("../")
            && href != self.api_patterns.index_file
        {
            return true;
        }

        false
    }

    /// Static version for backward compatibility and external use
    pub fn is_api_item_href_static(href: &str) -> bool {
        let patterns = ApiItemPatterns::default();

        // Skip external links, anchors, and non-API paths
        if href.starts_with("http") || href.starts_with("//") || href.starts_with('#') {
            return false;
        }

        // Check against default API item patterns
        for pattern in &patterns.item_type_markers {
            if href.contains(pattern) && href.ends_with(&patterns.html_extension) {
                return true;
            }
        }

        // Check for index files (modules)
        href.ends_with(&format!("/{}", patterns.index_file))
            && !href.starts_with("../")
            && href != patterns.index_file
    }

    /// Extract code examples from the documentation using configured selectors
    pub fn extract_code_examples(&self) -> Vec<CodeExample> {
        let mut examples = Vec::new();

        let code_selectors: Vec<&str> = self
            .html_config
            .code_example_selectors
            .iter()
            .map(|s| s.as_str())
            .collect();

        for element in self.extract_by_selectors(&code_selectors) {
            if let Some(code_text) = Self::extract_text_from_element(&element) {
                if !code_text.trim().is_empty() {
                    let title = Some(format!("Example {}", examples.len() + 1));
                    let is_runnable = element
                        .parent()
                        .and_then(|p| p.value().as_element())
                        .map(|e| e.classes().any(|c| c == "example-wrap"))
                        .unwrap_or(false);

                    examples.push(CodeExample {
                        title,
                        code: code_text.trim().to_string(),
                        language: "rust".to_string(),
                        is_runnable,
                    });
                }
            }
        }

        examples
    }

    /// Extract version from text using regex patterns
    fn extract_version_from_text(&self, text: &str) -> Option<String> {
        use regex::Regex;

        // Pattern for semantic versions (1.2.3, 0.1.0-alpha, etc.)
        let version_regex = Regex::new(r"\b(\d+\.\d+\.\d+(?:[-+][a-zA-Z0-9.-]*)?)\b").ok()?;

        if let Some(captures) = version_regex.captures(text) {
            return captures.get(1).map(|m| m.as_str().to_string());
        }

        // Pattern for shorter versions (1.2, 0.1, etc.)
        let short_version_regex = Regex::new(r"\b(\d+\.\d+)\b").ok()?;

        if let Some(captures) = short_version_regex.captures(text) {
            return captures.get(1).map(|m| m.as_str().to_string());
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_parser_creation() {
        let html = "<html><body><h1>Test</h1></body></html>";
        let parser = HtmlParser::new(html);
        assert!(!parser.document().html().is_empty());
    }

    #[test]
    fn test_extract_text_by_selectors() {
        let html = r#"
            <html>
                <body>
                    <h1>Main Title</h1>
                    <div class="content">Content Text</div>
                </body>
            </html>
        "#;

        let parser = HtmlParser::new(html);

        let text = parser.extract_text_by_selectors(&["h1"]);
        assert_eq!(text, Some("Main Title".to_string()));

        let text = parser.extract_text_by_selectors(&[".content"]);
        assert_eq!(text, Some("Content Text".to_string()));

        let text = parser.extract_text_by_selectors(&[".nonexistent"]);
        assert_eq!(text, None);
    }

    #[test]
    fn test_is_api_item_href() {
        assert!(HtmlParser::is_api_item_href_static("struct.Foo.html"));
        assert!(HtmlParser::is_api_item_href_static("trait.Bar.html"));
        assert!(HtmlParser::is_api_item_href_static("enum.Baz.html"));
        assert!(HtmlParser::is_api_item_href_static("fn.function.html"));
        assert!(HtmlParser::is_api_item_href_static("macro.my_macro.html"));
        assert!(HtmlParser::is_api_item_href_static("module/index.html"));

        assert!(!HtmlParser::is_api_item_href_static("https://external.com"));
        assert!(!HtmlParser::is_api_item_href_static("//example.com"));
        assert!(!HtmlParser::is_api_item_href_static("#anchor"));
        assert!(!HtmlParser::is_api_item_href_static("../parent/index.html"));
        assert!(!HtmlParser::is_api_item_href_static("index.html"));
    }

    #[test]
    fn test_extract_version() {
        let html = r#"
            <html>
                <head>
                    <title>serde 1.0.136 - Docs.rs</title>
                </head>
                <body>
                    <div class="version">v1.0.136</div>
                </body>
            </html>
        "#;

        let parser = HtmlParser::new(html);
        let version = parser.extract_version();
        assert_eq!(version, Some("1.0.136".to_string()));
    }

    #[test]
    fn test_extract_description() {
        let html = r#"
            <html>
                <body>
                    <div class="crate-description">A fast serialization framework</div>
                </body>
            </html>
        "#;

        let parser = HtmlParser::new(html);
        let description = parser.extract_description();
        assert_eq!(
            description,
            Some("A fast serialization framework".to_string())
        );
    }

    #[test]
    fn test_extract_api_links() {
        let html = r#"
            <html>
                <body>
                    <nav>
                        <a href="struct.Serialize.html">Serialize</a>
                        <a href="https://external.com">External</a>
                        <a href="trait.Deserialize.html">Deserialize</a>
                    </nav>
                </body>
            </html>
        "#;

        let parser = HtmlParser::new(html);
        let links = parser.extract_api_links();

        assert_eq!(links.len(), 2);
        assert!(links.contains(&("Serialize".to_string(), "struct.Serialize.html".to_string())));
        assert!(links.contains(&(
            "Deserialize".to_string(),
            "trait.Deserialize.html".to_string()
        )));
    }
}
