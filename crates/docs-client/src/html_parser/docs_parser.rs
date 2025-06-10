use crate::{
    config::{ApiItemPatterns, HtmlParsingConfig},
    error_handling::{build_docs_url, build_item_docs_url},
    html_parser::HtmlParser,
};
use rustacean_docs_core::{
    models::docs::{
        CrateCategories, CrateDocsResponse, CrateItem, CrateRelease, CrateSummary,
        ItemDocsResponse, ItemKind, Visibility,
    },
    resolve_version, Result,
};
use scraper::{Html, Selector};
use tracing::trace;

/// Parse comprehensive crate documentation from HTML content
pub fn parse_crate_documentation(
    html: &str,
    crate_name: &str,
    version: &Option<String>,
) -> Result<CrateDocsResponse> {
    let html_config = HtmlParsingConfig::default();
    let api_patterns = ApiItemPatterns::default();
    let parser = HtmlParser::with_config(html, html_config, api_patterns);

    // Extract version from page if not provided
    let actual_version = resolve_version(version.clone().or_else(|| parser.extract_version()));

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
pub fn parse_item_documentation(
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
pub fn parse_recent_releases(html: &str, limit: usize) -> Result<Vec<CrateRelease>> {
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

/// Parse navigation items to extract crate structure
fn parse_navigation_items(parser: &HtmlParser) -> Result<Vec<CrateItem>> {
    let api_links = parser.extract_api_links();
    let summaries = extract_item_summaries_from_page(parser);
    let mut items = Vec::new();

    for (text, href) in api_links {
        let mut item = create_crate_item_from_link(text, href.clone());

        // Try to find a summary for this item
        if let Some(summary) = summaries.get(&item.name).or_else(|| summaries.get(&href)) {
            item.summary = Some(summary.clone());
        }

        trace!(name = %item.name, kind = ?item.kind, path = %item.path, summary = ?item.summary, "Extracted API item");
        items.push(item);
    }

    // Remove duplicates based on name and kind
    items.sort_by(|a, b| a.name.cmp(&b.name).then(a.kind.cmp(&b.kind)));
    items.dedup_by(|a, b| a.name == b.name && a.kind == b.kind);

    Ok(items)
}

/// Extract item summaries from the documentation page
fn extract_item_summaries_from_page(
    parser: &HtmlParser,
) -> std::collections::HashMap<String, String> {
    let mut summaries = std::collections::HashMap::new();

    // Look for item table entries where dt contains the link and dd contains the description
    let item_table_selector = Selector::parse(".item-table").ok();
    if let Some(selector) = item_table_selector {
        for table in parser.document().select(&selector) {
            // Find dt/dd pairs
            let dt_selector = Selector::parse("dt").unwrap();
            let dd_selector = Selector::parse("dd").unwrap();

            let dts: Vec<_> = table.select(&dt_selector).collect();
            let dds: Vec<_> = table.select(&dd_selector).collect();

            // Match dt and dd elements
            for (dt, dd) in dts.iter().zip(dds.iter()) {
                if let Some(link_element) = dt.select(&Selector::parse("a").unwrap()).next() {
                    let name = link_element.text().collect::<String>().trim().to_string();
                    let href = link_element.value().attr("href").unwrap_or("").to_string();
                    let summary = dd.text().collect::<String>().trim().to_string();

                    if !name.is_empty() && !summary.is_empty() {
                        let clean_name = normalize_item_name(&name);
                        summaries.insert(clean_name.clone(), summary.clone());
                        summaries.insert(href, summary);
                    }
                }
            }
        }
    }

    summaries
}

/// Create a CrateItem from extracted link text and href
fn create_crate_item_from_link(name: String, path: String) -> CrateItem {
    // Clean up the item name by removing module paths and normalizing text
    let clean_name = normalize_item_name(&name);

    // Determine item kind from the link
    let kind = infer_item_kind(&path, &clean_name);

    CrateItem {
        name: clean_name,
        kind,
        summary: None, // Could be enhanced to extract from title attributes
        path: path.clone(),
        visibility: Visibility::Public, // Assume public for items in navigation
        is_async: false,                // Would need more analysis to determine
        signature: None,
        docs_path: Some(path),
    }
}

/// Normalize item name by removing module prefixes and cleaning text
fn normalize_item_name(name: &str) -> String {
    // Remove leading/trailing whitespace
    let trimmed = name.trim();

    // Remove module path prefixes (e.g., "reqwest::Client" -> "Client")
    let without_module = if let Some(last_part) = trimmed.split("::").last() {
        last_part
    } else {
        trimmed
    };

    // Remove any remaining unwanted characters and normalize
    let normalized = without_module
        .trim()
        .replace('\u{200B}', "") // Remove zero-width spaces
        .replace("_\u{200B}", "_") // Clean up word breaks in rustdoc
        .replace("\u{200B}_", "_")
        .replace("wbr", ""); // Remove any leftover wbr tags

    // If the name contains description-like text, try to extract just the identifier
    if normalized.contains(' ') && !normalized.starts_with(char::is_uppercase) {
        // This might be a description, try to extract the first word as the identifier
        if let Some(first_word) = normalized.split_whitespace().next() {
            if is_valid_rust_identifier(first_word) {
                return first_word.to_string();
            }
        }
    }

    normalized
}

/// Check if a string looks like a valid Rust identifier
fn is_valid_rust_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    let mut chars = s.chars();
    let first = chars.next().unwrap();

    // First character must be alphabetic or underscore
    if !first.is_alphabetic() && first != '_' {
        return false;
    }

    // Remaining characters must be alphanumeric or underscore
    chars.all(|c| c.is_alphanumeric() || c == '_')
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
    let name_selectors: Vec<&str> = html_config
        .item_name_selectors
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
    let kind_selectors: Vec<&str> = html_config
        .item_kind_selectors
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
    let signature_selectors: Vec<&str> = html_config
        .signature_selectors
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
    let related_selectors: Vec<&str> = html_config
        .related_items_selectors
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
