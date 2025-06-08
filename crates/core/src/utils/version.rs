/// Version handling utilities for consistent version resolution across all endpoints.
/// Default version to use when no version is specified
pub const DEFAULT_VERSION: &str = "latest";

/// Resolve an optional version string to a concrete version
///
/// Returns "latest" if the version is None or an empty string.
/// Otherwise returns the provided version.
pub fn resolve_version(version: Option<String>) -> String {
    version
        .map(|v| normalize_version(&v))
        .unwrap_or_else(|| DEFAULT_VERSION.to_string())
}

/// Normalize a version string to ensure consistent handling
///
/// Returns "latest" if the version is empty or only whitespace.
/// Otherwise returns the trimmed version string.
pub fn normalize_version(version: &str) -> String {
    let trimmed = version.trim();
    if trimmed.is_empty() {
        DEFAULT_VERSION.to_string()
    } else {
        trimmed.to_string()
    }
}

/// Check if a version string represents the latest version
pub fn is_latest_version(version: &str) -> bool {
    let normalized = normalize_version(version);
    normalized == DEFAULT_VERSION
}

/// Create an optional version string from a concrete version
///
/// Returns None if the version is "latest", otherwise returns Some(version).
/// This is useful for converting back to optional forms.
pub fn to_optional_version(version: &str) -> Option<String> {
    if is_latest_version(version) {
        None
    } else {
        Some(version.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_version() {
        // None should default to latest
        assert_eq!(resolve_version(None), "latest");

        // Some with value should return that value
        assert_eq!(resolve_version(Some("1.0.0".to_string())), "1.0.0");

        // Some with empty string should default to latest
        assert_eq!(resolve_version(Some("".to_string())), "latest");

        // Some with whitespace should default to latest
        assert_eq!(resolve_version(Some("   ".to_string())), "latest");
    }

    #[test]
    fn test_normalize_version() {
        // Normal version strings should be returned as-is (trimmed)
        assert_eq!(normalize_version("1.0.0"), "1.0.0");
        assert_eq!(normalize_version("  1.2.3  "), "1.2.3");

        // Empty strings should become latest
        assert_eq!(normalize_version(""), "latest");
        assert_eq!(normalize_version("   "), "latest");

        // Latest should remain latest
        assert_eq!(normalize_version("latest"), "latest");
    }

    #[test]
    fn test_is_latest_version() {
        // latest variations
        assert!(is_latest_version("latest"));
        assert!(is_latest_version("  latest  "));
        assert!(is_latest_version(""));
        assert!(is_latest_version("   "));

        // Non-latest versions
        assert!(!is_latest_version("1.0.0"));
        assert!(!is_latest_version("0.1.0"));
        assert!(!is_latest_version("2.0.0-beta.1"));
    }

    #[test]
    fn test_to_optional_version() {
        // Latest should become None
        assert_eq!(to_optional_version("latest"), None);
        assert_eq!(to_optional_version(""), None);
        assert_eq!(to_optional_version("   "), None);

        // Specific versions should remain Some
        assert_eq!(to_optional_version("1.0.0"), Some("1.0.0".to_string()));
        assert_eq!(to_optional_version("0.2.1"), Some("0.2.1".to_string()));
    }

    #[test]
    fn test_round_trip_conversion() {
        // Test that resolve_version and to_optional_version are inverses
        let versions = vec![
            None,
            Some("latest".to_string()),
            Some("1.0.0".to_string()),
            Some("".to_string()),
        ];

        for version in versions {
            let resolved = resolve_version(version.clone());
            let back_to_optional = to_optional_version(&resolved);

            // The round trip should preserve the semantic meaning
            match (version.as_ref(), back_to_optional.as_ref()) {
                (None, None) => (),                            // None -> latest -> None ✓
                (Some(v), None) if is_latest_version(v) => (), // "latest"/"" -> latest -> None ✓
                (Some(v), Some(back)) if v == back => (),      // "1.0.0" -> "1.0.0" -> "1.0.0" ✓
                _ => panic!("Round trip failed for version: {version:?}"),
            }
        }
    }
}
