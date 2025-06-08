use crate::client::DocsClient;
use rustacean_docs_cache::memory::MemoryCache;
use rustacean_docs_core::{
    error::ErrorContext,
    models::docs::{
        CodeExample, CrateCategories, CrateDocsRequest, CrateDocsResponse, CrateItem, CrateRelease,
        CrateSummary, ItemDocsRequest, ItemDocsResponse, ItemKind, RecentReleasesRequest,
        RecentReleasesResponse, Visibility,
    },
    Result,
};
use scraper::{Html, Selector};
use std::{hash::Hash, sync::Arc, time::Duration};
use tracing::{debug, trace};
use url::Url;

/// Cache key for crate documentation requests
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CrateDocsCacheKey {
    crate_name: String,
    version: Option<String>,
}

impl CrateDocsCacheKey {
    fn new(request: &CrateDocsRequest) -> Self {
        Self {
            crate_name: request.crate_name.clone(),
            version: request.version.clone(),
        }
    }
}

/// Cache key for item documentation requests
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ItemDocsCacheKey {
    crate_name: String,
    item_path: String,
    version: Option<String>,
}

impl ItemDocsCacheKey {
    fn new(request: &ItemDocsRequest) -> Self {
        Self {
            crate_name: request.crate_name.clone(),
            item_path: request.item_path.clone(),
            version: request.version.clone(),
        }
    }
}

/// Cache key for recent releases requests
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecentReleasesCacheKey {
    limit: usize,
}

impl RecentReleasesCacheKey {
    fn new(request: &RecentReleasesRequest) -> Self {
        Self {
            limit: request.limit(),
        }
    }
}

/// Documentation service that combines HTTP client with caching
pub struct DocsService {
    client: DocsClient,
    crate_docs_cache: Arc<MemoryCache<CrateDocsCacheKey, CrateDocsResponse>>,
    item_docs_cache: Arc<MemoryCache<ItemDocsCacheKey, ItemDocsResponse>>,
    releases_cache: Arc<MemoryCache<RecentReleasesCacheKey, RecentReleasesResponse>>,
}

impl DocsService {
    /// Create a new documentation service with cache
    pub fn new(client: DocsClient, cache_capacity: usize, cache_ttl: Duration) -> Self {
        let crate_docs_cache = Arc::new(MemoryCache::new(cache_capacity, cache_ttl));
        let item_docs_cache = Arc::new(MemoryCache::new(cache_capacity, cache_ttl));
        let releases_cache = Arc::new(MemoryCache::new(cache_capacity / 10, cache_ttl / 12)); // Shorter TTL for releases

        debug!(
            cache_capacity = cache_capacity,
            cache_ttl_secs = cache_ttl.as_secs(),
            "Created documentation service with cache"
        );

        Self {
            client,
            crate_docs_cache,
            item_docs_cache,
            releases_cache,
        }
    }

    /// Get comprehensive crate documentation with caching
    pub async fn get_crate_docs(&self, request: CrateDocsRequest) -> Result<CrateDocsResponse> {
        let cache_key = CrateDocsCacheKey::new(&request);

        // Try to get from cache first
        if let Some(cached_response) = self.crate_docs_cache.get(&cache_key).await {
            trace!(
                crate_name = %request.crate_name,
                version = ?request.version,
                "Crate docs cache hit"
            );
            return Ok(cached_response);
        }

        trace!(
            crate_name = %request.crate_name,
            version = ?request.version,
            "Crate docs cache miss, fetching from docs.rs"
        );

        // Cache miss - fetch from docs.rs
        let response = self.client.get_crate_docs(request).await?;

        // Store in cache for future requests
        self.crate_docs_cache
            .insert(cache_key, response.clone())
            .await;

        debug!(
            crate_name = %response.name,
            version = %response.version,
            item_count = response.items.len(),
            "Crate documentation fetched and cached"
        );

        Ok(response)
    }

    /// Get specific item documentation with caching
    pub async fn get_item_docs(&self, request: ItemDocsRequest) -> Result<ItemDocsResponse> {
        let cache_key = ItemDocsCacheKey::new(&request);

        // Try to get from cache first
        if let Some(cached_response) = self.item_docs_cache.get(&cache_key).await {
            trace!(
                crate_name = %request.crate_name,
                item_path = %request.item_path,
                version = ?request.version,
                "Item docs cache hit"
            );
            return Ok(cached_response);
        }

        trace!(
            crate_name = %request.crate_name,
            item_path = %request.item_path,
            version = ?request.version,
            "Item docs cache miss, fetching from docs.rs"
        );

        // Cache miss - fetch from docs.rs
        let response = self.client.get_item_docs(request).await?;

        // Store in cache for future requests
        self.item_docs_cache
            .insert(cache_key, response.clone())
            .await;

        debug!(
            crate_name = %response.crate_name,
            item_name = %response.name,
            "Item documentation fetched and cached"
        );

        Ok(response)
    }

    /// Get recent releases with caching
    pub async fn get_recent_releases(
        &self,
        request: RecentReleasesRequest,
    ) -> Result<RecentReleasesResponse> {
        let cache_key = RecentReleasesCacheKey::new(&request);

        // Try to get from cache first
        if let Some(cached_response) = self.releases_cache.get(&cache_key).await {
            trace!(limit = request.limit(), "Recent releases cache hit");
            return Ok(cached_response);
        }

        trace!(
            limit = request.limit(),
            "Recent releases cache miss, fetching from docs.rs"
        );

        // Cache miss - fetch from docs.rs
        let response = self.client.get_recent_releases(request).await?;

        // Store in cache for future requests
        self.releases_cache
            .insert(cache_key, response.clone())
            .await;

        debug!(
            release_count = response.releases.len(),
            "Recent releases fetched and cached"
        );

        Ok(response)
    }

    /// Get cache statistics for all caches
    pub async fn cache_stats(
        &self,
    ) -> (
        rustacean_docs_core::models::metadata::CacheLayerStats,
        rustacean_docs_core::models::metadata::CacheLayerStats,
        rustacean_docs_core::models::metadata::CacheLayerStats,
    ) {
        let crate_stats = self.crate_docs_cache.stats().await;
        let item_stats = self.item_docs_cache.stats().await;
        let releases_stats = self.releases_cache.stats().await;
        (crate_stats, item_stats, releases_stats)
    }

    /// Clear all documentation caches
    pub async fn clear_cache(&self) -> (usize, usize, usize) {
        let crate_cleared = self.crate_docs_cache.clear().await;
        let item_cleared = self.item_docs_cache.clear().await;
        let releases_cleared = self.releases_cache.clear().await;
        (crate_cleared, item_cleared, releases_cleared)
    }

    /// Clean up expired cache entries in all caches
    pub async fn cleanup_expired(&self) -> (usize, usize, usize) {
        let crate_expired = self.crate_docs_cache.cleanup_expired().await;
        let item_expired = self.item_docs_cache.cleanup_expired().await;
        let releases_expired = self.releases_cache.cleanup_expired().await;
        (crate_expired, item_expired, releases_expired)
    }
}

impl DocsClient {
    /// Get comprehensive crate documentation from docs.rs
    pub async fn get_crate_docs(&self, request: CrateDocsRequest) -> Result<CrateDocsResponse> {
        let version_path = if let Some(ref version) = request.version {
            format!("/{version}")
        } else {
            String::new()
        };

        let path = format!("/{}{version_path}/", request.crate_name);

        trace!(
            crate_name = %request.crate_name,
            version = ?request.version,
            path = %path,
            "Fetching crate documentation from docs.rs"
        );

        let html_content = self.get_text(&path).await?;
        let parsed_docs =
            parse_crate_documentation(&html_content, &request.crate_name, &request.version)?;

        debug!(
            crate_name = %request.crate_name,
            item_count = parsed_docs.items.len(),
            "Successfully parsed crate documentation"
        );

        Ok(parsed_docs)
    }

    /// Get specific item documentation from docs.rs
    pub async fn get_item_docs(&self, request: ItemDocsRequest) -> Result<ItemDocsResponse> {
        let version_path = if let Some(ref version) = request.version {
            format!("/{version}")
        } else {
            String::new()
        };

        // Try to resolve the item path - it might be a simple name or a full path
        let resolved_path = resolve_item_path(&request.item_path);
        let path = format!("/{}{version_path}/{resolved_path}", request.crate_name);

        trace!(
            crate_name = %request.crate_name,
            item_path = %request.item_path,
            resolved_path = %resolved_path,
            version = ?request.version,
            path = %path,
            "Fetching item documentation from docs.rs"
        );

        let html_content = self.get_text(&path).await?;
        let parsed_docs = parse_item_documentation(
            &html_content,
            &request.crate_name,
            &request.item_path,
            &request.version,
        )?;

        debug!(
            crate_name = %request.crate_name,
            item_name = %parsed_docs.name,
            "Successfully parsed item documentation"
        );

        Ok(parsed_docs)
    }

    /// Get recent releases from docs.rs homepage
    pub async fn get_recent_releases(
        &self,
        request: RecentReleasesRequest,
    ) -> Result<RecentReleasesResponse> {
        trace!(
            limit = request.limit(),
            "Fetching recent releases from docs.rs homepage"
        );

        let html_content = self.get_text("/").await?;
        let releases = parse_recent_releases(&html_content, request.limit())?;

        debug!(
            release_count = releases.len(),
            "Successfully parsed recent releases"
        );

        Ok(RecentReleasesResponse { releases })
    }
}

/// Parse comprehensive crate documentation from HTML content
fn parse_crate_documentation(
    html: &str,
    crate_name: &str,
    version: &Option<String>,
) -> Result<CrateDocsResponse> {
    let document = Html::parse_document(html);

    // Extract version from page if not provided
    let actual_version = version.clone().unwrap_or_else(|| {
        extract_version_from_page(&document).unwrap_or_else(|| "latest".to_string())
    });

    // Extract crate description
    let description = extract_crate_description(&document);

    // Parse navigation structure to get items
    let items = parse_navigation_items(&document)?;

    // Generate summary from parsed items
    let summary = generate_crate_summary(&items, description.clone());

    // Categorize items
    let categories = categorize_items(&items);

    // Extract code examples
    let examples = extract_code_examples(&document);

    // Generate docs URL
    let docs_url = Some(
        Url::parse(&format!("https://docs.rs/{crate_name}/{actual_version}"))
            .context("Failed to construct docs.rs URL")?,
    );

    Ok(CrateDocsResponse {
        name: crate_name.to_string(),
        version: actual_version,
        summary,
        categories,
        items,
        examples,
        docs_url,
    })
}

/// Parse specific item documentation from HTML content
fn parse_item_documentation(
    html: &str,
    crate_name: &str,
    item_path: &str,
    version: &Option<String>,
) -> Result<ItemDocsResponse> {
    let document = Html::parse_document(html);

    // Extract item name from the page
    let name = extract_item_name(&document, item_path);

    // Determine item kind
    let kind = extract_item_kind(&document);

    // Extract signature
    let signature = extract_item_signature(&document);

    // Extract description
    let description = extract_item_description(&document);

    // Extract code examples
    let examples = extract_code_examples(&document);

    // Extract related items
    let related_items = extract_related_items(&document);

    // Generate docs URL
    let actual_version = version.clone().unwrap_or_else(|| "latest".to_string());
    let docs_url = Some(
        Url::parse(&format!(
            "https://docs.rs/{crate_name}/{actual_version}/{item_path}"
        ))
        .context("Failed to construct item docs URL")?,
    );

    Ok(ItemDocsResponse {
        crate_name: crate_name.to_string(),
        item_path: item_path.to_string(),
        name,
        kind,
        signature,
        description,
        examples,
        docs_url,
        related_items,
    })
}

/// Parse recent releases from docs.rs homepage
fn parse_recent_releases(html: &str, limit: usize) -> Result<Vec<CrateRelease>> {
    let document = Html::parse_document(html);
    let mut releases = Vec::new();

    // Look for recent releases section (this is a simplified implementation)
    // In practice, we'd need to analyze the actual docs.rs homepage structure
    let releases_selector = Selector::parse(".recent-releases .release-item").map_err(|e| {
        rustacean_docs_core::Error::documentation_parse(format!("Invalid CSS selector: {e}"))
    })?;

    for element in document.select(&releases_selector).take(limit) {
        if let Some(release) = extract_release_info(&element) {
            releases.push(release);
        }
    }

    // If no releases found in structured format, try alternative parsing
    if releases.is_empty() {
        releases = extract_releases_fallback(&document, limit)?;
    }

    Ok(releases)
}

/// Extract version from the documentation page
fn extract_version_from_page(document: &Html) -> Option<String> {
    // Try to find version in the page title or header
    let version_selector = Selector::parse(".version, .crate-version, h1 .version").ok()?;

    for element in document.select(&version_selector) {
        if let Some(text) = element.text().next() {
            let version = text.trim().trim_start_matches('v');
            if !version.is_empty() {
                return Some(version.to_string());
            }
        }
    }

    None
}

/// Extract crate description from the documentation
fn extract_crate_description(document: &Html) -> Option<String> {
    // Try various selectors for crate description
    let description_selectors = [
        ".docblock.item-decl p",
        ".crate-description",
        ".docblock p:first-child",
        "meta[name='description']",
    ];

    for selector_str in &description_selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            if let Some(element) = document.select(&selector).next() {
                let text = if selector_str.contains("meta") {
                    element.value().attr("content").map(|s| s.to_string())
                } else {
                    Some(element.text().collect::<String>().trim().to_string())
                };

                if let Some(desc) = text {
                    if !desc.is_empty() {
                        return Some(desc);
                    }
                }
            }
        }
    }

    None
}

/// Parse navigation items to extract crate structure
fn parse_navigation_items(document: &Html) -> Result<Vec<CrateItem>> {
    let mut items = Vec::new();

    // Parse sidebar navigation
    let nav_selectors = [
        ".sidebar .block ul li a",
        ".sidebar-links a",
        ".item-table .item-left a",
    ];

    for selector_str in &nav_selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            for element in document.select(&selector) {
                if let Some(item) = extract_nav_item(&element) {
                    items.push(item);
                }
            }
        }
    }

    // Remove duplicates
    items.sort_by(|a, b| a.name.cmp(&b.name));
    items.dedup_by(|a, b| a.name == b.name && a.kind == b.kind);

    Ok(items)
}

/// Extract a navigation item from an HTML element
fn extract_nav_item(element: &scraper::ElementRef) -> Option<CrateItem> {
    let name = element.text().collect::<String>().trim().to_string();
    if name.is_empty() {
        return None;
    }

    let path = element.value().attr("href").unwrap_or("").to_string();

    // Determine item kind from the link or surrounding context
    let kind = infer_item_kind(&path, &name);

    // Extract any summary from title attribute
    let summary = element.value().attr("title").map(|s| s.to_string());

    Some(CrateItem {
        name,
        kind,
        summary,
        path: path.clone(),
        visibility: Visibility::Public, // Assume public for items in navigation
        is_async: false,                // Would need more analysis to determine
        signature: None,
        docs_path: Some(path),
    })
}

/// Infer item kind from path or name
fn infer_item_kind(path: &str, _name: &str) -> ItemKind {
    if path.contains("struct.") {
        ItemKind::Struct
    } else if path.contains("trait.") {
        ItemKind::Trait
    } else if path.contains("enum.") {
        ItemKind::Enum
    } else if path.contains("fn.") {
        ItemKind::Function
    } else if path.contains("macro.") {
        ItemKind::Macro
    } else if path.contains("constant.") {
        ItemKind::Constant
    } else if path.contains("type.") {
        ItemKind::TypeAlias
    } else if path.contains("union.") {
        ItemKind::Union
    } else if path.ends_with("/index.html") || path.contains("/index.html") {
        ItemKind::Module
    } else {
        // Default to function for unknown types
        ItemKind::Function
    }
}

/// Generate crate summary from parsed items
fn generate_crate_summary(items: &[CrateItem], description: Option<String>) -> CrateSummary {
    let mut module_count = 0;
    let mut struct_count = 0;
    let mut trait_count = 0;
    let mut function_count = 0;
    let mut enum_count = 0;

    for item in items {
        match item.kind {
            ItemKind::Module => module_count += 1,
            ItemKind::Struct => struct_count += 1,
            ItemKind::Trait => trait_count += 1,
            ItemKind::Function | ItemKind::Method => function_count += 1,
            ItemKind::Enum => enum_count += 1,
            _ => {}
        }
    }

    CrateSummary {
        description,
        module_count,
        struct_count,
        trait_count,
        function_count,
        enum_count,
        features: Vec::new(), // Would need to parse from Cargo.toml or features page
    }
}

/// Categorize items by type
fn categorize_items(items: &[CrateItem]) -> CrateCategories {
    let mut core_types = Vec::new();
    let mut traits = Vec::new();
    let mut modules = Vec::new();
    let mut functions = Vec::new();
    let mut macros = Vec::new();
    let mut constants = Vec::new();

    for item in items {
        match item.kind {
            ItemKind::Struct | ItemKind::Enum | ItemKind::TypeAlias | ItemKind::Union => {
                core_types.push(item.name.clone());
            }
            ItemKind::Trait => {
                traits.push(item.name.clone());
            }
            ItemKind::Module => {
                modules.push(item.name.clone());
            }
            ItemKind::Function | ItemKind::Method => {
                functions.push(item.name.clone());
            }
            ItemKind::Macro => {
                macros.push(item.name.clone());
            }
            ItemKind::Constant => {
                constants.push(item.name.clone());
            }
        }
    }

    CrateCategories {
        core_types,
        traits,
        modules,
        functions,
        macros,
        constants,
    }
}

/// Extract code examples from documentation
fn extract_code_examples(document: &Html) -> Vec<CodeExample> {
    let mut examples = Vec::new();

    // Look for code blocks in the documentation
    let code_selectors = [
        "pre.rust code",
        ".example-wrap pre code",
        ".docblock pre.rust",
    ];

    for selector_str in &code_selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            for (index, element) in document.select(&selector).enumerate() {
                let code = element.text().collect::<String>();
                if !code.trim().is_empty() {
                    let title = Some(format!("Example {}", index + 1));
                    let is_runnable = element
                        .parent()
                        .and_then(|p| p.value().as_element())
                        .map(|e| e.classes().any(|c| c == "example-wrap"))
                        .unwrap_or(false);

                    examples.push(CodeExample {
                        title,
                        code: code.trim().to_string(),
                        language: "rust".to_string(),
                        is_runnable,
                    });
                }
            }
        }
    }

    examples
}

/// Extract item name from documentation page
fn extract_item_name(document: &Html, fallback: &str) -> String {
    // Try to find item name in page title or main heading
    let name_selectors = [
        "h1.fqn .in-band",
        "h1 .struct, h1 .trait, h1 .enum, h1 .fn",
        "h1",
        "title",
    ];

    for selector_str in &name_selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            if let Some(element) = document.select(&selector).next() {
                let text = element.text().collect::<String>().trim().to_string();
                if !text.is_empty() && !text.to_lowercase().contains("documentation") {
                    // Extract just the item name, removing module path
                    if let Some(name) = text.split("::").last() {
                        return name.trim().to_string();
                    }
                    return text;
                }
            }
        }
    }

    // Fallback to extracting name from path
    fallback
        .split('/')
        .next_back()
        .unwrap_or(fallback)
        .to_string()
}

/// Extract item kind from documentation page
fn extract_item_kind(document: &Html) -> ItemKind {
    // Look for kind indicators in the page
    let kind_selectors = [
        "h1 .struct",
        "h1 .trait",
        "h1 .enum",
        "h1 .fn",
        "h1 .macro",
        "h1 .constant",
        "h1 .type",
        "h1 .union",
    ];

    for selector_str in &kind_selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            if document.select(&selector).next().is_some() {
                return match selector_str {
                    s if s.contains("struct") => ItemKind::Struct,
                    s if s.contains("trait") => ItemKind::Trait,
                    s if s.contains("enum") => ItemKind::Enum,
                    s if s.contains("fn") => ItemKind::Function,
                    s if s.contains("macro") => ItemKind::Macro,
                    s if s.contains("constant") => ItemKind::Constant,
                    s if s.contains("type") => ItemKind::TypeAlias,
                    s if s.contains("union") => ItemKind::Union,
                    _ => ItemKind::Function,
                };
            }
        }
    }

    ItemKind::Function // Default fallback
}

/// Extract item signature from documentation page
fn extract_item_signature(document: &Html) -> Option<String> {
    let signature_selectors = [".item-decl pre.rust", ".docblock.item-decl", ".signature"];

    for selector_str in &signature_selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            if let Some(element) = document.select(&selector).next() {
                let signature = element.text().collect::<String>().trim().to_string();
                if !signature.is_empty() {
                    return Some(signature);
                }
            }
        }
    }

    None
}

/// Extract item description from documentation page
fn extract_item_description(document: &Html) -> Option<String> {
    let description_selectors = [
        ".docblock:not(.item-decl) p:first-child",
        ".top-doc .docblock p:first-child",
        ".docblock p",
    ];

    for selector_str in &description_selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            if let Some(element) = document.select(&selector).next() {
                let description = element.text().collect::<String>().trim().to_string();
                if !description.is_empty() {
                    return Some(description);
                }
            }
        }
    }

    None
}

/// Extract related items from documentation page
fn extract_related_items(document: &Html) -> Vec<String> {
    let mut related = Vec::new();

    // Look for implementation blocks and related links
    let related_selectors = [
        ".impl-items .method a",
        ".trait-implementations a",
        ".implementors a",
    ];

    for selector_str in &related_selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            for element in document.select(&selector) {
                let text = element.text().collect::<String>().trim().to_string();
                if !text.is_empty() {
                    related.push(text);
                }
            }
        }
    }

    related.sort();
    related.dedup();
    related
}

/// Extract release information from HTML element
fn extract_release_info(_element: &scraper::ElementRef) -> Option<CrateRelease> {
    // This would need to be implemented based on actual docs.rs homepage structure
    // For now, return None as a placeholder
    None
}

/// Fallback method to extract releases when structured parsing fails
fn extract_releases_fallback(_document: &Html, _limit: usize) -> Result<Vec<CrateRelease>> {
    // Placeholder implementation - would need to analyze actual docs.rs homepage
    Ok(Vec::new())
}

/// Resolve item path for different formats
fn resolve_item_path(item_path: &str) -> String {
    // If it's already a full path, use as-is
    if item_path.contains('.') && item_path.contains("html") {
        return item_path.to_string();
    }

    // If it's a simple name, try to construct a path
    // This is a simplified heuristic - in practice we'd need more sophisticated resolution
    if item_path.chars().next().is_some_and(|c| c.is_uppercase()) {
        // Likely a type (struct, enum, trait)
        format!("struct.{item_path}.html")
    } else {
        // Likely a function
        format!("fn.{item_path}.html")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_crate_docs_cache_key() {
        let request1 = CrateDocsRequest::new("tokio");
        let request2 = CrateDocsRequest::with_version("tokio", "1.0.0");
        let request3 = CrateDocsRequest::new("serde");

        let key1 = CrateDocsCacheKey::new(&request1);
        let key2 = CrateDocsCacheKey::new(&request2);
        let key3 = CrateDocsCacheKey::new(&request3);

        assert_ne!(key1, key2); // Different versions
        assert_ne!(key1, key3); // Different crates

        // Keys should be hashable
        use std::collections::HashMap;
        let mut map = HashMap::new();
        map.insert(key1.clone(), "value1");
        map.insert(key2, "value2");
        map.insert(key3, "value3");

        assert_eq!(map.get(&key1), Some(&"value1"));
    }

    #[test]
    fn test_item_docs_cache_key() {
        let request1 = ItemDocsRequest::new("tokio", "spawn");
        let request2 = ItemDocsRequest::with_version("tokio", "spawn", "1.0.0");
        let request3 = ItemDocsRequest::new("tokio", "join");

        let key1 = ItemDocsCacheKey::new(&request1);
        let key2 = ItemDocsCacheKey::new(&request2);
        let key3 = ItemDocsCacheKey::new(&request3);

        assert_ne!(key1, key2); // Different versions
        assert_ne!(key1, key3); // Different items

        // Keys should be hashable
        use std::collections::HashMap;
        let mut map = HashMap::new();
        map.insert(key1.clone(), "value1");
        map.insert(key2, "value2");
        map.insert(key3, "value3");

        assert_eq!(map.get(&key1), Some(&"value1"));
    }

    #[test]
    fn test_recent_releases_cache_key() {
        let request1 = RecentReleasesRequest::new();
        let request2 = RecentReleasesRequest::with_limit(10);
        let request3 = RecentReleasesRequest::with_limit(20);

        let key1 = RecentReleasesCacheKey::new(&request1);
        let key2 = RecentReleasesCacheKey::new(&request2);
        let key3 = RecentReleasesCacheKey::new(&request3);

        assert_eq!(key1, key3); // Same limit (default is 20)
        assert_ne!(key1, key2); // Different limits (20 vs 10)
    }

    #[test]
    fn test_infer_item_kind() {
        assert_eq!(infer_item_kind("struct.Foo.html", "Foo"), ItemKind::Struct);
        assert_eq!(infer_item_kind("trait.Bar.html", "Bar"), ItemKind::Trait);
        assert_eq!(infer_item_kind("enum.Baz.html", "Baz"), ItemKind::Enum);
        assert_eq!(infer_item_kind("fn.qux.html", "qux"), ItemKind::Function);
        assert_eq!(infer_item_kind("macro.quux.html", "quux"), ItemKind::Macro);
        assert_eq!(
            infer_item_kind("constant.CONST.html", "CONST"),
            ItemKind::Constant
        );
        assert_eq!(
            infer_item_kind("type.Type.html", "Type"),
            ItemKind::TypeAlias
        );
        assert_eq!(
            infer_item_kind("union.Union.html", "Union"),
            ItemKind::Union
        );
        assert_eq!(
            infer_item_kind("module/index.html", "module"),
            ItemKind::Module
        );
        assert_eq!(infer_item_kind("unknown", "unknown"), ItemKind::Function);
    }

    #[test]
    fn test_resolve_item_path() {
        // Full paths should be preserved
        assert_eq!(resolve_item_path("struct.Foo.html"), "struct.Foo.html");
        assert_eq!(resolve_item_path("fn.bar.html"), "fn.bar.html");

        // Simple names should be resolved
        assert_eq!(resolve_item_path("Foo"), "struct.Foo.html"); // Uppercase -> struct
        assert_eq!(resolve_item_path("bar"), "fn.bar.html"); // Lowercase -> function
    }

    #[test]
    fn test_generate_crate_summary() {
        let items = vec![
            CrateItem {
                name: "Foo".to_string(),
                kind: ItemKind::Struct,
                summary: None,
                path: "struct.Foo.html".to_string(),
                visibility: Visibility::Public,
                is_async: false,
                signature: None,
                docs_path: None,
            },
            CrateItem {
                name: "Bar".to_string(),
                kind: ItemKind::Trait,
                summary: None,
                path: "trait.Bar.html".to_string(),
                visibility: Visibility::Public,
                is_async: false,
                signature: None,
                docs_path: None,
            },
            CrateItem {
                name: "baz".to_string(),
                kind: ItemKind::Function,
                summary: None,
                path: "fn.baz.html".to_string(),
                visibility: Visibility::Public,
                is_async: false,
                signature: None,
                docs_path: None,
            },
        ];

        let summary = generate_crate_summary(&items, Some("Test crate".to_string()));

        assert_eq!(summary.description, Some("Test crate".to_string()));
        assert_eq!(summary.struct_count, 1);
        assert_eq!(summary.trait_count, 1);
        assert_eq!(summary.function_count, 1);
        assert_eq!(summary.module_count, 0);
        assert_eq!(summary.enum_count, 0);
    }

    #[test]
    fn test_categorize_items() {
        let items = vec![
            CrateItem {
                name: "Foo".to_string(),
                kind: ItemKind::Struct,
                summary: None,
                path: "struct.Foo.html".to_string(),
                visibility: Visibility::Public,
                is_async: false,
                signature: None,
                docs_path: None,
            },
            CrateItem {
                name: "Bar".to_string(),
                kind: ItemKind::Trait,
                summary: None,
                path: "trait.Bar.html".to_string(),
                visibility: Visibility::Public,
                is_async: false,
                signature: None,
                docs_path: None,
            },
            CrateItem {
                name: "mod1".to_string(),
                kind: ItemKind::Module,
                summary: None,
                path: "mod1/index.html".to_string(),
                visibility: Visibility::Public,
                is_async: false,
                signature: None,
                docs_path: None,
            },
        ];

        let categories = categorize_items(&items);

        assert_eq!(categories.core_types, vec!["Foo"]);
        assert_eq!(categories.traits, vec!["Bar"]);
        assert_eq!(categories.modules, vec!["mod1"]);
        assert!(categories.functions.is_empty());
        assert!(categories.macros.is_empty());
        assert!(categories.constants.is_empty());
    }

    #[tokio::test]
    async fn test_docs_service_creation() {
        let client = DocsClient::new().unwrap();
        let service = DocsService::new(client, 100, Duration::from_secs(3600));

        let (crate_stats, item_stats, releases_stats) = service.cache_stats().await;

        assert_eq!(crate_stats.size, 0);
        assert_eq!(crate_stats.capacity, 100);
        assert_eq!(item_stats.size, 0);
        assert_eq!(item_stats.capacity, 100);
        assert_eq!(releases_stats.size, 0);
        assert_eq!(releases_stats.capacity, 10); // releases cache is smaller
    }

    #[tokio::test]
    async fn test_docs_service_cache_operations() {
        let client = DocsClient::new().unwrap();
        let service = DocsService::new(client, 10, Duration::from_secs(60));

        // Test cache clear
        let (crate_cleared, item_cleared, releases_cleared) = service.clear_cache().await;
        assert_eq!(crate_cleared, 0); // Empty caches
        assert_eq!(item_cleared, 0);
        assert_eq!(releases_cleared, 0);

        // Test cleanup expired
        let (crate_expired, item_expired, releases_expired) = service.cleanup_expired().await;
        assert_eq!(crate_expired, 0); // No expired entries
        assert_eq!(item_expired, 0);
        assert_eq!(releases_expired, 0);
    }

    // Additional tests would be added for HTML parsing functions with mock HTML content
    #[test]
    fn test_extract_version_from_page() {
        let html = r#"
            <html>
                <head><title>tokio 1.35.0</title></head>
                <body>
                    <h1><span class="version">1.35.0</span></h1>
                </body>
            </html>
        "#;
        let document = Html::parse_document(html);
        let version = extract_version_from_page(&document);
        assert_eq!(version, Some("1.35.0".to_string()));
    }

    #[test]
    fn test_extract_crate_description() {
        let html = r#"
            <html>
                <body>
                    <div class="docblock item-decl">
                        <p>A runtime for writing reliable, asynchronous applications</p>
                    </div>
                </body>
            </html>
        "#;
        let document = Html::parse_document(html);
        let description = extract_crate_description(&document);
        assert_eq!(
            description,
            Some("A runtime for writing reliable, asynchronous applications".to_string())
        );
    }
}
