use rustacean_docs_core::models::docs::CodeExample;
use scraper::{Html, Selector, ElementRef};
use tracing::trace;

/// Centralized HTML parser utility for docs.rs content
pub struct HtmlParser {
    document: Html,
}

impl HtmlParser {
    /// Create a new HTML parser from HTML content
    pub fn new(html: &str) -> Self {
        Self {
            document: Html::parse_document(html),
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

    /// Extract version from page using common patterns
    pub fn extract_version(&self) -> Option<String> {
        let version_selectors = [
            ".version",
            ".crate-version", 
            "h1 .version",
            ".nav-version",
            "[data-version]",
        ];

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

    /// Extract description using common selectors
    pub fn extract_description(&self) -> Option<String> {
        let description_selectors = [
            ".crate-description",
            ".docblock p:first-child",
            ".top-doc .docblock p:first-child",
            "meta[name='description']",
            ".description",
            ".summary",
        ];

        for selector_str in &description_selectors {
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
        
        let nav_selectors = [
            "nav a[href]",
            ".sidebar a[href]",
            ".item-table dt a[href]",
            ".docblock a[href]",
        ];

        for element in self.extract_by_selectors(&nav_selectors) {
            if let Some(href) = Self::extract_href_from_element(&element) {
                trace!(href = %href, "Found link in navigation");
                
                if Self::is_api_item_href(&href) {
                    if let Some(text) = Self::extract_text_from_element(&element) {
                        trace!(href = %href, text = %text, "Link matches API item pattern");
                        links.push((text, href));
                    }
                }
            }
        }

        links
    }

    /// Check if an href points to an actual API item
    pub fn is_api_item_href(href: &str) -> bool {
        // Skip external links, anchors, and non-API paths
        if href.starts_with("http") || href.starts_with("//") || href.starts_with('#') {
            return false;
        }

        // API item patterns from docs.rs
        href.contains("trait.") && href.ends_with(".html")
            || href.contains("struct.") && href.ends_with(".html")
            || href.contains("enum.") && href.ends_with(".html")
            || href.contains("fn.") && href.ends_with(".html")
            || href.contains("macro.") && href.ends_with(".html")
            || href.contains("derive.") && href.ends_with(".html")
            || href.contains("constant.") && href.ends_with(".html")
            || href.contains("type.") && href.ends_with(".html")
            || href.contains("union.") && href.ends_with(".html")
            || (href.ends_with("/index.html") && !href.starts_with("../") && href != "index.html")
    }

    /// Extract code examples from the documentation
    pub fn extract_code_examples(&self) -> Vec<CodeExample> {
        let mut examples = Vec::new();

        // Look for code blocks in the documentation
        let code_selectors = [
            "pre.rust code",
            ".example-wrap pre code",
            ".docblock pre.rust",
        ];

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
        assert!(HtmlParser::is_api_item_href("struct.Foo.html"));
        assert!(HtmlParser::is_api_item_href("trait.Bar.html"));
        assert!(HtmlParser::is_api_item_href("enum.Baz.html"));
        assert!(HtmlParser::is_api_item_href("fn.function.html"));
        assert!(HtmlParser::is_api_item_href("macro.my_macro.html"));
        assert!(HtmlParser::is_api_item_href("module/index.html"));
        
        assert!(!HtmlParser::is_api_item_href("https://external.com"));
        assert!(!HtmlParser::is_api_item_href("//example.com"));
        assert!(!HtmlParser::is_api_item_href("#anchor"));
        assert!(!HtmlParser::is_api_item_href("../parent/index.html"));
        assert!(!HtmlParser::is_api_item_href("index.html"));
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
        assert_eq!(description, Some("A fast serialization framework".to_string()));
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
        assert!(links.contains(&("Deserialize".to_string(), "trait.Deserialize.html".to_string())));
    }
}