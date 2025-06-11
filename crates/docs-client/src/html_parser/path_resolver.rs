use crate::endpoints::docs_modules::service::DocsService;
use rustacean_docs_core::{
    models::docs::CrateDocsRequest,
    types::{CrateName, Version},
    Result,
};

/// Resolve item path for different formats with intelligent fallback
pub async fn resolve_item_path_with_fallback(
    docs_service: &DocsService,
    crate_name: &str,
    item_path: &str,
    version: &Option<Version>,
) -> Result<String> {
    // If it's already a full path, use as-is
    if item_path.contains('.') && item_path.contains("html") {
        return Ok(item_path.to_string());
    }

    // If it's a simple name, try to find it by fetching the crate docs first
    let crate_docs_request = CrateDocsRequest {
        crate_name: CrateName::new(crate_name).map_err(|_| {
            rustacean_docs_core::error::ErrorBuilder::internal("Invalid crate name")
        })?,
        version: version.clone(),
    };

    match docs_service.get_crate_docs(crate_docs_request).await {
        Ok(docs) => {
            // Try to find exact match first
            for item in &docs.items {
                if item.name == item_path {
                    return Ok(item.path.clone());
                }
            }

            // Try case-insensitive match
            let lower_item_path = item_path.to_lowercase();
            for item in &docs.items {
                if item.name.to_lowercase() == lower_item_path {
                    return Ok(item.path.clone());
                }
            }

            // Try fuzzy matching (partial matches)
            let mut best_matches = Vec::new();
            for item in &docs.items {
                if item.name.to_lowercase().contains(&lower_item_path)
                    || lower_item_path.contains(&item.name.to_lowercase())
                {
                    best_matches.push(item);
                }
            }

            // If we found matches, return the best one (exact substring match preferred)
            if !best_matches.is_empty() {
                // Prefer exact substring matches
                for item in &best_matches {
                    if item.name.to_lowercase() == lower_item_path {
                        return Ok(item.path.clone());
                    }
                }
                // Return first fuzzy match
                return Ok(best_matches[0].path.clone());
            }
        }
        Err(_) => {
            // Fallback to heuristic approach if we can't fetch crate docs
        }
    }

    // Last resort: use heuristic approach
    resolve_item_path_heuristic(item_path)
}

/// Fallback heuristic for resolving item paths
fn resolve_item_path_heuristic(item_path: &str) -> Result<String> {
    // Generate possible paths to try in order of likelihood
    let possible_paths = generate_possible_item_paths(item_path);

    // For now, return the most likely path
    // In a future enhancement, we could try each path until one works
    Ok(possible_paths[0].clone())
}

/// Generate possible item paths for a given name
fn generate_possible_item_paths(item_name: &str) -> Vec<String> {
    let mut paths = Vec::new();

    // If item_name starts with uppercase, it's likely a type
    if item_name.chars().next().is_some_and(|c| c.is_uppercase()) {
        // Order by likelihood for types
        paths.push(format!("trait.{item_name}.html")); // Most common for public APIs
        paths.push(format!("struct.{item_name}.html"));
        paths.push(format!("enum.{item_name}.html"));
        paths.push(format!("type.{item_name}.html"));
        paths.push(format!("union.{item_name}.html"));
        paths.push(format!("macro.{item_name}.html"));
        paths.push(format!("derive.{item_name}.html"));
        paths.push(format!("constant.{item_name}.html"));
    } else {
        // Lowercase names are likely functions
        paths.push(format!("fn.{item_name}.html"));
        paths.push(format!("macro.{item_name}.html")); // Some macros are lowercase
        paths.push(format!("constant.{item_name}.html")); // Constants might be lowercase

        // Also try if it might be a module
        paths.push(format!("{item_name}/index.html"));
    }

    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_possible_item_paths() {
        // Test uppercase names (likely types)
        let paths = generate_possible_item_paths("Serialize");
        assert!(paths.contains(&"trait.Serialize.html".to_string()));
        assert!(paths.contains(&"struct.Serialize.html".to_string()));
        assert!(paths[0] == "trait.Serialize.html"); // trait should be first (most likely)

        // Test lowercase names (likely functions)
        let paths = generate_possible_item_paths("spawn");
        assert!(paths.contains(&"fn.spawn.html".to_string()));
        assert!(paths.contains(&"macro.spawn.html".to_string()));
        assert!(paths[0] == "fn.spawn.html"); // function should be first

        // Test module-like names
        let paths = generate_possible_item_paths("fs");
        assert!(paths.contains(&"fs/index.html".to_string()));
    }

    #[test]
    fn test_resolve_item_path_heuristic() {
        // Test basic heuristic resolution
        let result = resolve_item_path_heuristic("Serialize").unwrap();
        assert_eq!(result, "trait.Serialize.html");

        let result = resolve_item_path_heuristic("spawn").unwrap();
        assert_eq!(result, "fn.spawn.html");
    }
}
