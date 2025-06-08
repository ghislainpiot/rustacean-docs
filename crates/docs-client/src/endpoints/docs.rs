use crate::{
    client::DocsClient, 
    html_parser::HtmlParser,
    error_handling::{build_docs_url, build_item_docs_url},
    config::{HtmlParsingConfig, ApiItemPatterns}
};
use rustacean_docs_cache::memory::MemoryCache;
use rustacean_docs_core::{
    models::docs::{
        CrateCategories, CrateDocsRequest, CrateDocsResponse, CrateItem, CrateRelease,
        CrateSummary, ItemDocsRequest, ItemDocsResponse, ItemKind, RecentReleasesRequest,
        RecentReleasesResponse, Visibility,
    },
    resolve_version, Result,
};
use scraper::{Html, Selector};
use std::{hash::Hash, sync::Arc, time::Duration};
use tracing::{debug, trace};

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
        let actual_version = resolve_version(request.version.clone());
        let version_path = format!("/{actual_version}");

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
        let actual_version = resolve_version(request.version.clone());
        let version_path = format!("/{actual_version}");

        // Try to resolve the item path - it might be a simple name or a full path
        let resolved_path = resolve_item_path(&request.item_path);
        let path = format!(
            "/{}{version_path}/{}/{resolved_path}",
            request.crate_name, request.crate_name
        );

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
    let html_config = HtmlParsingConfig::default();
    let api_patterns = ApiItemPatterns::default();
    let parser = HtmlParser::with_config(html, html_config, api_patterns);

    // Extract version from page if not provided
    let actual_version = resolve_version(
        version
            .clone()
            .or_else(|| parser.extract_version()),
    );

    // Extract crate description
    let description = parser.extract_description();

    // Parse navigation structure to get items
    let items = parse_navigation_items(&parser)?;

    // Generate summary from parsed items
    let summary = generate_crate_summary(&items, description.clone());

    // Categorize items
    let categories = categorize_items(&items);

    // Extract code examples
    let examples = parser.extract_code_examples();

    // Generate docs URL
    let docs_url = Some(build_docs_url(crate_name, &actual_version)?);

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
    let html_config = HtmlParsingConfig::default();
    let api_patterns = ApiItemPatterns::default();
    let parser = HtmlParser::with_config(html, html_config, api_patterns);
    let document = parser.document();

    // Extract item name from the page
    let name = extract_item_name(document, item_path);

    // Determine item kind
    let kind = extract_item_kind(document);

    // Extract signature
    let signature = extract_item_signature(document);

    // Extract description
    let description = extract_item_description(document);

    // Extract code examples
    let examples = parser.extract_code_examples();

    // Extract related items
    let related_items = extract_related_items(document);

    // Generate docs URL
    let actual_version = resolve_version(version.clone());
    let docs_url = Some(build_item_docs_url(crate_name, &actual_version, item_path)?);

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

/// Parse navigation items to extract crate structure
fn parse_navigation_items(parser: &HtmlParser) -> Result<Vec<CrateItem>> {
    let api_links = parser.extract_api_links();
    let mut items = Vec::new();

    for (text, href) in api_links {
        let item = create_crate_item_from_link(text, href);
        trace!(name = %item.name, kind = ?item.kind, path = %item.path, "Extracted API item");
        items.push(item);
    }

    // Remove duplicates
    items.sort_by(|a, b| a.name.cmp(&b.name));
    items.dedup_by(|a, b| a.name == b.name && a.kind == b.kind);

    Ok(items)
}

/// Create a CrateItem from extracted link text and href
fn create_crate_item_from_link(name: String, path: String) -> CrateItem {
    // Determine item kind from the link
    let kind = infer_item_kind(&path, &name);

    CrateItem {
        name,
        kind,
        summary: None, // Could be enhanced to extract from title attributes
        path: path.clone(),
        visibility: Visibility::Public, // Assume public for items in navigation
        is_async: false,                // Would need more analysis to determine
        signature: None,
        docs_path: Some(path),
    }
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
    } else if path.contains("macro.") || path.contains("derive.") {
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

/// Extract item name from documentation page
fn extract_item_name(document: &Html, fallback: &str) -> String {
    let html_config = HtmlParsingConfig::default();
    let name_selectors: Vec<&str> = html_config.item_name_selectors
        .iter()
        .map(|s| s.as_str())
        .collect();

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
    let html_config = HtmlParsingConfig::default();
    let kind_selectors: Vec<&str> = html_config.item_kind_selectors
        .iter()
        .map(|s| s.as_str())
        .collect();

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
    let html_config = HtmlParsingConfig::default();
    let signature_selectors: Vec<&str> = html_config.signature_selectors
        .iter()
        .map(|s| s.as_str())
        .collect();

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

    let html_config = HtmlParsingConfig::default();
    let related_selectors: Vec<&str> = html_config.related_items_selectors
        .iter()
        .map(|s| s.as_str())
        .collect();

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
fn extract_release_info(element: &scraper::ElementRef) -> Option<CrateRelease> {
    use chrono::Utc;

    // Extract the link href to get crate name and version
    let _href = element.value().attr("href")?;

    // Extract text content which should contain crate-version and description
    let text = element.text().collect::<String>();
    let lines: Vec<&str> = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();

    if lines.len() < 2 {
        return None;
    }

    // First line should be crate-version
    let crate_version = lines[0];
    let (name, version) = if let Some(dash_pos) = crate_version.rfind('-') {
        let name = &crate_version[..dash_pos];
        let version = &crate_version[dash_pos + 1..];
        (name.to_string(), version.to_string())
    } else {
        // If no version separator found, treat the whole thing as name
        (crate_version.to_string(), "latest".to_string())
    };

    // Second line should be description
    let description = if lines.len() > 1 && !lines[1].is_empty() {
        Some(lines[1].to_string())
    } else {
        None
    };

    // Try to find publication time in the text (e.g., "18 seconds ago", "2 hours ago")
    let published_at = if let Some(time_line) = lines.iter().find(|line| line.contains("ago")) {
        parse_relative_time(time_line).unwrap_or_else(Utc::now)
    } else {
        Utc::now() // Fallback to current time
    };

    // Generate docs URL
    let docs_url = build_docs_url(&name, &version).ok();

    Some(CrateRelease {
        name,
        version,
        description,
        published_at,
        docs_url,
    })
}

/// Parse relative time strings like "18 seconds ago", "2 hours ago" into DateTime
fn parse_relative_time(time_str: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    use chrono::{Duration, Utc};

    let time_str = time_str.trim().to_lowercase();
    let now = Utc::now();

    // Extract number and unit from strings like "18 seconds ago"
    let parts: Vec<&str> = time_str.split_whitespace().collect();
    if parts.len() < 3 {
        return None;
    }

    let amount: i64 = parts[0].parse().ok()?;
    let unit = parts[1];

    let duration = match unit {
        "second" | "seconds" => Duration::seconds(amount),
        "minute" | "minutes" => Duration::minutes(amount),
        "hour" | "hours" => Duration::hours(amount),
        "day" | "days" => Duration::days(amount),
        "week" | "weeks" => Duration::weeks(amount),
        _ => return None,
    };

    Some(now - duration)
}

/// Fallback method to extract releases when structured parsing fails
fn extract_releases_fallback(document: &Html, limit: usize) -> Result<Vec<CrateRelease>> {
    let mut releases = Vec::new();

    // Try alternative selectors for release information
    let fallback_selectors = [
        "a[href*='/crates/']",
        ".release a",
        "ul li a",
        ".content a[href^='/']",
    ];

    for selector_str in &fallback_selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            for element in document.select(&selector).take(limit * 2) {
                // Filter for elements that look like crate links
                if let Some(href) = element.value().attr("href") {
                    if href.contains("/crates/") || href.starts_with('/') && !href.starts_with("//")
                    {
                        if let Some(release) = extract_release_info_fallback(&element) {
                            releases.push(release);
                        }
                    }
                }

                if releases.len() >= limit {
                    break;
                }
            }

            if !releases.is_empty() {
                break;
            }
        }
    }

    // Remove duplicates based on name+version
    releases.sort_by(|a, b| a.name.cmp(&b.name).then(a.version.cmp(&b.version)));
    releases.dedup_by(|a, b| a.name == b.name && a.version == b.version);

    // Sort by publication date (newest first)
    releases.sort_by(|a, b| b.published_at.cmp(&a.published_at));

    // Limit to requested amount
    releases.truncate(limit);

    Ok(releases)
}

/// Fallback method to extract release information from any link element
fn extract_release_info_fallback(element: &scraper::ElementRef) -> Option<CrateRelease> {
    use chrono::Utc;

    let href = element.value().attr("href")?;
    let text = element.text().collect::<String>().trim().to_string();

    if text.is_empty() {
        return None;
    }

    // Try to extract crate name from href or text
    let name = if href.contains("/crates/") {
        // Extract from path like "/crates/serde/1.0.0"
        href.split("/crates/")
            .nth(1)?
            .split('/')
            .next()?
            .to_string()
    } else if href.starts_with('/') {
        // Extract from path like "/serde/1.0.0"
        href.trim_start_matches('/').split('/').next()?.to_string()
    } else {
        // Try to extract from text
        text.split_whitespace().next()?.to_string()
    };

    // Simple version extraction - look for version patterns in href or text
    let version = extract_version_from_text(&text)
        .or_else(|| extract_version_from_href(href))
        .unwrap_or_else(|| "latest".to_string());

    // Use text as description if it's not just the crate name
    let description = if text.len() > name.len() + 10 {
        Some(text)
    } else {
        None
    };

    let docs_url = build_docs_url(&name, &version).ok();

    Some(CrateRelease {
        name,
        version,
        description,
        published_at: Utc::now(), // Fallback timestamp
        docs_url,
    })
}

/// Extract version from text using patterns
fn extract_version_from_text(text: &str) -> Option<String> {
    use regex::Regex;

    // Look for patterns like "1.0.0", "v1.0.0", "version 1.0.0"
    let version_pattern = Regex::new(r"v?(\d+\.\d+\.\d+(?:-[a-zA-Z0-9.-]+)?)").ok()?;
    version_pattern
        .captures(text)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// Extract version from href path
fn extract_version_from_href(href: &str) -> Option<String> {
    // Extract version from paths like "/crate/1.0.0" or "/crate/latest"
    let parts: Vec<&str> = href.split('/').collect();
    if parts.len() >= 3 {
        let potential_version = parts[parts.len() - 1];
        if potential_version
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
        {
            return Some(potential_version.to_string());
        }
    }
    None
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

    // Tests with pre-downloaded HTML files
    fn load_test_html(filename: &str) -> String {
        std::fs::read_to_string(format!("test_fixtures/{}", filename))
            .unwrap_or_else(|_| panic!("Could not load test fixture: {}", filename))
    }

    #[test]
    fn test_parse_serde_crate_documentation() {
        let html = load_test_html("serde_docs.html");
        let result = parse_crate_documentation(&html, "serde", &None);

        assert!(result.is_ok());
        let docs = result.unwrap();

        // Verify basic crate info
        assert_eq!(docs.name, "serde");
        assert_eq!(docs.version, "1.0.219");

        // Verify items were parsed correctly
        assert!(!docs.items.is_empty());

        // Should have traits
        let trait_items: Vec<_> = docs
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::Trait)
            .collect();
        assert_eq!(trait_items.len(), 4);

        // Should have modules
        let module_items: Vec<_> = docs
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::Module)
            .collect();

        // Should have exactly 2 modules: de, ser (no self-reference)
        assert_eq!(module_items.len(), 2);

        // Should have macros
        let macro_items: Vec<_> = docs
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::Macro)
            .collect();
        assert_eq!(macro_items.len(), 3);

        // Verify specific items exist
        let trait_names: Vec<_> = trait_items.iter().map(|item| &item.name).collect();
        assert!(trait_names.contains(&&"Deserialize".to_string()));
        assert!(trait_names.contains(&&"Serialize".to_string()));
        assert!(trait_names.contains(&&"Deserializer".to_string()));
        assert!(trait_names.contains(&&"Serializer".to_string()));

        let module_names: Vec<_> = module_items.iter().map(|item| &item.name).collect();
        assert!(module_names.contains(&&"de".to_string()));
        assert!(module_names.contains(&&"ser".to_string()));

        // Verify categories (exact counts based on fixture)
        assert_eq!(docs.categories.traits.len(), 4);
        assert_eq!(docs.categories.modules.len(), 2); // de, ser only
        assert_eq!(docs.categories.macros.len(), 3);

        // Verify summary counts (exact)
        assert_eq!(docs.summary.trait_count, 4);
        assert_eq!(docs.summary.module_count, 2);
        assert_eq!(docs.summary.function_count, 0);
        assert_eq!(docs.summary.struct_count, 0);
        assert_eq!(docs.summary.enum_count, 0);
    }

    #[test]
    fn test_parse_tokio_crate_documentation() {
        let html = load_test_html("tokio_docs.html");
        let result = parse_crate_documentation(&html, "tokio", &None);

        assert!(result.is_ok());
        let docs = result.unwrap();

        // Verify basic crate info
        assert_eq!(docs.name, "tokio");
        assert_eq!(docs.version, "1.35.0");

        // Should have exactly 7 modules: fs, io, net, runtime, sync, task, time
        let module_items: Vec<_> = docs
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::Module)
            .collect();
        assert_eq!(module_items.len(), 7);

        // Should have exactly 2 structs: JoinError, JoinHandle
        let struct_items: Vec<_> = docs
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::Struct)
            .collect();
        assert_eq!(struct_items.len(), 2);

        // Should have exactly 2 functions: spawn, spawn_blocking
        let function_items: Vec<_> = docs
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::Function)
            .collect();
        assert_eq!(function_items.len(), 2);

        // Should have exactly 4 macros: join, pin, select, try_join (removed non-existent spawn macro)
        let macro_items: Vec<_> = docs
            .items
            .iter()
            .filter(|item| item.kind == ItemKind::Macro)
            .collect();
        assert_eq!(macro_items.len(), 4);

        // Verify specific items exist
        let module_names: Vec<_> = module_items.iter().map(|item| &item.name).collect();
        assert!(module_names.contains(&&"fs".to_string()));
        assert!(module_names.contains(&&"io".to_string()));
        assert!(module_names.contains(&&"net".to_string()));
        assert!(module_names.contains(&&"runtime".to_string()));
        assert!(module_names.contains(&&"sync".to_string()));
        assert!(module_names.contains(&&"task".to_string()));
        assert!(module_names.contains(&&"time".to_string()));

        let struct_names: Vec<_> = struct_items.iter().map(|item| &item.name).collect();
        assert!(struct_names.contains(&&"JoinError".to_string()));
        assert!(struct_names.contains(&&"JoinHandle".to_string()));

        let function_names: Vec<_> = function_items.iter().map(|item| &item.name).collect();
        assert!(function_names.contains(&&"spawn".to_string()));
        assert!(function_names.contains(&&"spawn_blocking".to_string()));

        let macro_names: Vec<_> = macro_items.iter().map(|item| &item.name).collect();
        assert!(macro_names.contains(&&"join".to_string()));
        assert!(macro_names.contains(&&"pin".to_string()));
        assert!(macro_names.contains(&&"select".to_string()));
        assert!(macro_names.contains(&&"try_join".to_string()));
    }

    #[test]
    fn test_parse_navigation_items_serde() {
        let html = load_test_html("serde_docs.html");
        let parser = HtmlParser::new(&html);
        let result = parse_navigation_items(&parser);

        assert!(result.is_ok());
        let items = result.unwrap();

        // Should find all the expected items
        assert!(!items.is_empty());

        // Check for specific items
        let item_names: Vec<_> = items.iter().map(|item| &item.name).collect();
        assert!(item_names.contains(&&"Deserialize".to_string()));
        assert!(item_names.contains(&&"Serialize".to_string()));
        assert!(item_names.contains(&&"de".to_string()));
        assert!(item_names.contains(&&"ser".to_string()));
        assert!(item_names.contains(&&"forward_to_deserialize_any".to_string()));
    }

    #[test]
    fn test_categorize_items_with_fixtures() {
        let html = load_test_html("tokio_docs.html");
        let parser = HtmlParser::new(&html);
        let items = parse_navigation_items(&parser).unwrap();
        let categories = categorize_items(&items);

        // Should have multiple categories
        assert!(!categories.modules.is_empty());
        assert!(!categories.functions.is_empty());
        assert!(!categories.macros.is_empty());
        assert!(!categories.core_types.is_empty()); // structs

        // Verify specific categorization
        assert!(categories.modules.contains(&"fs".to_string()));
        assert!(categories.modules.contains(&"io".to_string()));
        assert!(categories.functions.contains(&"spawn".to_string()));
        assert!(categories.macros.contains(&"join".to_string()));
        assert!(categories.core_types.contains(&"JoinHandle".to_string()));
    }

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
        let parser = HtmlParser::new(html);
        let version = parser.extract_version();
        assert_eq!(version, Some("1.35.0".to_string()));
    }

    #[test]
    fn test_extract_version_from_fixture() {
        let html = load_test_html("serde_docs.html");
        let parser = HtmlParser::new(&html);
        let version = parser.extract_version();
        assert_eq!(version, Some("1.0.219".to_string()));
    }

    // Edge case and error handling tests
    #[test]
    fn test_parse_empty_crate_documentation() {
        let html = load_test_html("empty_crate.html");
        let result = parse_crate_documentation(&html, "empty_crate", &None);

        assert!(result.is_ok());
        let docs = result.unwrap();

        // Verify basic info is still extracted
        assert_eq!(docs.name, "empty_crate");
        assert_eq!(docs.version, "0.1.0");

        // Should have no API items (self-references should be filtered out)
        assert!(docs.items.is_empty());

        // All counts should be zero
        assert_eq!(docs.summary.trait_count, 0);
        assert_eq!(docs.summary.module_count, 0);
        assert_eq!(docs.summary.function_count, 0);
        assert_eq!(docs.summary.struct_count, 0);
        assert_eq!(docs.summary.enum_count, 0);

        // Categories should be empty
        assert!(docs.categories.traits.is_empty());
        assert!(docs.categories.modules.is_empty());
        assert!(docs.categories.functions.is_empty());
        assert!(docs.categories.macros.is_empty());
        assert!(docs.categories.core_types.is_empty());
        assert!(docs.categories.constants.is_empty());
    }

    #[test]
    fn test_parse_malformed_documentation() {
        let html = load_test_html("malformed_docs.html");
        let result = parse_crate_documentation(&html, "malformed_crate", &None);

        // Should still succeed but filter out malformed items
        assert!(result.is_ok());
        let docs = result.unwrap();

        assert_eq!(docs.name, "malformed_crate");

        // Should only find valid items, filtering out:
        // - Items without href
        // - Items with external links
        // - Items with JavaScript hrefs
        // - Empty items
        let valid_items: Vec<_> = docs
            .items
            .iter()
            .filter(|item| !item.name.is_empty() && !item.path.is_empty())
            .collect();

        // Should have at least the ValidTrait
        assert!(!valid_items.is_empty());

        // Verify that only valid items are included
        let valid_item_names: Vec<_> = valid_items.iter().map(|item| &item.name).collect();
        assert!(valid_item_names.contains(&&"ValidTrait".to_string()));

        // Should not contain malformed items
        assert!(!valid_item_names.contains(&&"BrokenTrait".to_string()));
        assert!(!valid_item_names.contains(&&"JSTrait".to_string()));
        assert!(!valid_item_names.contains(&&"external_fn".to_string()));
    }

    #[test]
    fn test_is_api_item_href_edge_cases() {
        // Valid API item patterns
        assert!(HtmlParser::is_api_item_href_static("trait.Serialize.html"));
        assert!(HtmlParser::is_api_item_href_static("struct.HashMap.html"));
        assert!(HtmlParser::is_api_item_href_static("de/index.html"));

        // Invalid patterns that should be filtered
        assert!(!HtmlParser::is_api_item_href_static("javascript:void(0)"));
        assert!(!HtmlParser::is_api_item_href_static("https://example.com"));
        assert!(!HtmlParser::is_api_item_href_static("//external.com"));
        assert!(!HtmlParser::is_api_item_href_static("#anchor"));
        assert!(!HtmlParser::is_api_item_href_static("../parent/page.html"));
        assert!(!HtmlParser::is_api_item_href_static(""));

        // Edge cases
        assert!(!HtmlParser::is_api_item_href_static("all.html")); // "All Items" link
        assert!(!HtmlParser::is_api_item_href_static("help.html")); // Help link
        assert!(!HtmlParser::is_api_item_href_static("settings.html")); // Settings link
    }

    #[test]
    fn test_infer_item_kind_edge_cases() {
        // Standard cases
        assert_eq!(infer_item_kind("struct.Foo.html", "Foo"), ItemKind::Struct);
        assert_eq!(infer_item_kind("trait.Bar.html", "Bar"), ItemKind::Trait);
        assert_eq!(
            infer_item_kind("derive.Serialize.html", "Serialize"),
            ItemKind::Macro
        );

        // Edge cases
        assert_eq!(infer_item_kind("", "unknown"), ItemKind::Function); // Default fallback
        assert_eq!(infer_item_kind("unknown.html", "test"), ItemKind::Function); // Default fallback
        assert_eq!(
            infer_item_kind("some/weird/path", "test"),
            ItemKind::Function
        ); // Default fallback

        // Module variations
        assert_eq!(
            infer_item_kind("module/index.html", "module"),
            ItemKind::Module
        );
        assert_eq!(
            infer_item_kind("path/to/module/index.html", "module"),
            ItemKind::Module
        );
    }

    #[test]
    fn test_extract_nav_item_edge_cases() {
        let html = r#"
            <html>
                <body>
                    <nav>
                        <!-- Empty link text -->
                        <a href="trait.Empty.html" title="trait test::Empty"></a>
                        
                        <!-- No href attribute -->
                        <a title="trait test::NoHref">NoHref</a>
                        
                        <!-- Valid item -->
                        <a href="trait.Valid.html" title="trait test::Valid">Valid</a>
                    </nav>
                </body>
            </html>
        "#;
        let parser = HtmlParser::new(html);
        let api_links = parser.extract_api_links();

        // Should only extract valid API links (links with href that match API patterns and have text)
        assert_eq!(api_links.len(), 1);

        let (name, href) = &api_links[0];
        assert_eq!(name, "Valid");
        assert_eq!(href, "trait.Valid.html");
        
        // Test that the link is correctly identified as an API item
        assert!(HtmlParser::is_api_item_href_static(href));
    }

    #[test]
    fn test_generate_crate_summary_edge_cases() {
        // Empty items
        let empty_summary = generate_crate_summary(&[], None);
        assert_eq!(empty_summary.module_count, 0);
        assert_eq!(empty_summary.struct_count, 0);
        assert_eq!(empty_summary.trait_count, 0);
        assert_eq!(empty_summary.function_count, 0);
        assert_eq!(empty_summary.enum_count, 0);
        assert!(empty_summary.description.is_none());

        // Mixed items with description
        let mixed_items = vec![
            CrateItem {
                name: "TestStruct".to_string(),
                kind: ItemKind::Struct,
                summary: None,
                path: "struct.TestStruct.html".to_string(),
                visibility: Visibility::Public,
                is_async: false,
                signature: None,
                docs_path: None,
            },
            CrateItem {
                name: "test_fn".to_string(),
                kind: ItemKind::Function,
                summary: None,
                path: "fn.test_fn.html".to_string(),
                visibility: Visibility::Public,
                is_async: true, // Test async function
                signature: None,
                docs_path: None,
            },
        ];

        let mixed_summary =
            generate_crate_summary(&mixed_items, Some("Test description".to_string()));
        assert_eq!(mixed_summary.struct_count, 1);
        assert_eq!(mixed_summary.function_count, 1);
        assert_eq!(
            mixed_summary.description,
            Some("Test description".to_string())
        );
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
        let parser = HtmlParser::new(html);
        let description = parser.extract_description();
        assert_eq!(
            description,
            Some("A runtime for writing reliable, asynchronous applications".to_string())
        );
    }

    #[test]
    fn test_parse_relative_time() {
        use chrono::{Duration, Utc};

        let now = Utc::now();

        // Test various time formats
        assert!(parse_relative_time("5 seconds ago").is_some());
        assert!(parse_relative_time("10 minutes ago").is_some());
        assert!(parse_relative_time("2 hours ago").is_some());
        assert!(parse_relative_time("3 days ago").is_some());
        assert!(parse_relative_time("1 week ago").is_some());

        // Test parsing accuracy
        if let Some(parsed) = parse_relative_time("5 seconds ago") {
            let diff = now - parsed;
            assert!(diff >= Duration::seconds(4) && diff <= Duration::seconds(6));
        }

        if let Some(parsed) = parse_relative_time("2 hours ago") {
            let diff = now - parsed;
            assert!(diff >= Duration::hours(1) + Duration::minutes(59));
            assert!(diff <= Duration::hours(2) + Duration::minutes(1));
        }

        // Test invalid formats
        assert!(parse_relative_time("invalid").is_none());
        assert!(parse_relative_time("5 invalid ago").is_none());
        assert!(parse_relative_time("").is_none());
        assert!(parse_relative_time("ago").is_none());
    }

    #[test]
    fn test_extract_version_from_text() {
        // Test various version patterns
        assert_eq!(
            extract_version_from_text("tokio-1.35.0"),
            Some("1.35.0".to_string())
        );
        assert_eq!(
            extract_version_from_text("v1.2.3"),
            Some("1.2.3".to_string())
        );
        assert_eq!(
            extract_version_from_text("version 2.0.0"),
            Some("2.0.0".to_string())
        );
        assert_eq!(
            extract_version_from_text("serde 1.0.0-alpha1"),
            Some("1.0.0-alpha1".to_string())
        );
        assert_eq!(
            extract_version_from_text("some text 3.1.4 more text"),
            Some("3.1.4".to_string())
        );

        // Test no version found
        assert_eq!(extract_version_from_text("no version here"), None);
        assert_eq!(extract_version_from_text("just text"), None);
        assert_eq!(extract_version_from_text(""), None);
    }

    #[test]
    fn test_extract_version_from_href() {
        // Test various href patterns
        assert_eq!(
            extract_version_from_href("/crate/tokio/1.35.0"),
            Some("1.35.0".to_string())
        );
        assert_eq!(
            extract_version_from_href("/serde/2.0.0"),
            Some("2.0.0".to_string())
        );
        assert_eq!(
            extract_version_from_href("/crates/async-std/1.12.0"),
            Some("1.12.0".to_string())
        );

        // Test no version found
        assert_eq!(extract_version_from_href("/crate/tokio/latest"), None);
        assert_eq!(extract_version_from_href("/crate/tokio"), None);
        assert_eq!(extract_version_from_href("/"), None);
        assert_eq!(extract_version_from_href(""), None);
    }

    #[test]
    fn test_extract_release_info_fallback() {
        use scraper::{Html, Selector};

        let html = r#"
            <html>
                <body>
                    <a href="/tokio/1.35.0">tokio-1.35.0 - A runtime for async applications</a>
                </body>
            </html>
        "#;
        let document = Html::parse_document(html);
        let selector = Selector::parse("a").unwrap();

        if let Some(element) = document.select(&selector).next() {
            let release = extract_release_info_fallback(&element);
            assert!(release.is_some());

            let release = release.unwrap();
            assert_eq!(release.name, "tokio");
            assert_eq!(release.version, "1.35.0");
            assert!(release.description.is_some());
            assert!(release.description.unwrap().contains("runtime"));
            assert!(release.docs_url.is_some());
        }
    }

    #[test]
    fn test_extract_releases_fallback() {
        let html = r#"
            <html>
                <body>
                    <div class="content">
                        <ul>
                            <li><a href="/tokio/1.35.0">tokio-1.35.0 - Async runtime</a></li>
                            <li><a href="/serde/1.0.195">serde-1.0.195 - Serialization framework</a></li>
                            <li><a href="/reqwest/0.11.23">reqwest-0.11.23 - HTTP client</a></li>
                        </ul>
                    </div>
                </body>
            </html>
        "#;
        let document = Html::parse_document(html);
        let releases = extract_releases_fallback(&document, 2).unwrap();

        assert!(releases.len() <= 2);
        if !releases.is_empty() {
            let first_release = &releases[0];
            assert!(!first_release.name.is_empty());
            assert!(!first_release.version.is_empty());
        }
    }

    #[test]
    fn test_recent_releases_request_integration() {
        // Test request limit functionality
        let request1 = RecentReleasesRequest::new();
        assert_eq!(request1.limit(), 20);

        let request2 = RecentReleasesRequest::with_limit(5);
        assert_eq!(request2.limit(), 5);

        let request3 = RecentReleasesRequest::with_limit(150);
        assert_eq!(request3.limit(), 100); // Should be clamped to max
    }

    #[test]
    fn test_is_api_item_href() {
        // Test valid API item hrefs
        assert!(HtmlParser::is_api_item_href_static("trait.Serialize.html"));
        assert!(HtmlParser::is_api_item_href_static("struct.HashMap.html"));
        assert!(HtmlParser::is_api_item_href_static("enum.Option.html"));
        assert!(HtmlParser::is_api_item_href_static("fn.println.html"));
        assert!(HtmlParser::is_api_item_href_static("macro.vec.html"));
        assert!(HtmlParser::is_api_item_href_static("derive.Serialize.html"));
        assert!(HtmlParser::is_api_item_href_static("constant.PI.html"));
        assert!(HtmlParser::is_api_item_href_static("type.Result.html"));
        assert!(HtmlParser::is_api_item_href_static("union.MyUnion.html"));
        assert!(HtmlParser::is_api_item_href_static("de/index.html"));
        assert!(HtmlParser::is_api_item_href_static("ser/index.html"));

        // Test invalid hrefs
        assert!(!HtmlParser::is_api_item_href_static("#data-formats")); // Documentation sections
        assert!(!HtmlParser::is_api_item_href_static("#design"));
        assert!(!HtmlParser::is_api_item_href_static("https://example.com"));
        assert!(!HtmlParser::is_api_item_href_static("//external.com"));
        assert!(!HtmlParser::is_api_item_href_static("../parent/page.html"));
        assert!(!HtmlParser::is_api_item_href_static("javascript:void(0)"));
        assert!(!HtmlParser::is_api_item_href_static(""));
        assert!(!HtmlParser::is_api_item_href_static("just-text"));
    }
}
