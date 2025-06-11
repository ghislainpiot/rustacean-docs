use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ItemPath(String);

impl ItemPath {
    pub fn new(path: impl Into<String>) -> Result<Self, ItemPathError> {
        let path = path.into();
        Self::validate(&path)?;
        Ok(Self(path))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_module(&self) -> bool {
        self.0.ends_with("/index.html") || !self.0.contains('.')
    }

    pub fn is_struct(&self) -> bool {
        self.0.contains("/struct.") || self.0.starts_with("struct.")
    }

    pub fn is_trait(&self) -> bool {
        self.0.contains("/trait.") || self.0.starts_with("trait.")
    }

    pub fn is_enum(&self) -> bool {
        self.0.contains("/enum.") || self.0.starts_with("enum.")
    }

    pub fn is_function(&self) -> bool {
        self.0.contains("/fn.") || self.0.starts_with("fn.")
    }

    pub fn item_name(&self) -> Option<&str> {
        if let Some(pos) = self.0.rfind('/') {
            let after_slash = &self.0[pos + 1..];
            if let Some(dot_pos) = after_slash.find('.') {
                let name = &after_slash[dot_pos + 1..];
                return name.strip_suffix(".html").or(Some(name));
            }
        } else if let Some(dot_pos) = self.0.find('.') {
            let name = &self.0[dot_pos + 1..];
            return name.strip_suffix(".html").or(Some(name));
        }

        // For simple names without prefix
        if !self.0.contains('/') && !self.0.contains('.') {
            return Some(&self.0);
        }

        None
    }

    fn validate(path: &str) -> Result<(), ItemPathError> {
        if path.is_empty() {
            return Err(ItemPathError::Empty);
        }

        // Basic validation - no spaces or special characters
        for ch in path.chars() {
            match ch {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' | '.' | '/' | ':' => {}
                _ => return Err(ItemPathError::InvalidCharacter(ch)),
            }
        }

        // Cannot have double slashes
        if path.contains("//") {
            return Err(ItemPathError::InvalidFormat(
                "contains double slashes".to_string(),
            ));
        }

        Ok(())
    }
}

impl fmt::Display for ItemPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for ItemPath {
    type Err = ItemPathError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl From<ItemPath> for String {
    fn from(path: ItemPath) -> Self {
        path.0
    }
}

impl AsRef<str> for ItemPath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ItemPathError {
    #[error("Item path cannot be empty")]
    Empty,
    #[error("Invalid character '{0}' in item path")]
    InvalidCharacter(char),
    #[error("Invalid path format: {0}")]
    InvalidFormat(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_paths() {
        assert!(ItemPath::new("HashMap").is_ok());
        assert!(ItemPath::new("struct.HashMap.html").is_ok());
        assert!(ItemPath::new("collections/struct.HashMap.html").is_ok());
        assert!(ItemPath::new("std::collections::HashMap").is_ok());
        assert!(ItemPath::new("trait.Iterator.html").is_ok());
        assert!(ItemPath::new("fn.main.html").is_ok());
    }

    #[test]
    fn test_invalid_paths() {
        assert!(ItemPath::new("").is_err());
        assert!(ItemPath::new("my path").is_err());
        assert!(ItemPath::new("path//double").is_err());
        assert!(ItemPath::new("path@invalid").is_err());
    }

    #[test]
    fn test_item_type_detection() {
        let struct_path = ItemPath::new("struct.HashMap.html").unwrap();
        assert!(struct_path.is_struct());

        let trait_path = ItemPath::new("trait.Iterator.html").unwrap();
        assert!(trait_path.is_trait());

        let fn_path = ItemPath::new("fn.main.html").unwrap();
        assert!(fn_path.is_function());

        let enum_path = ItemPath::new("enum.Result.html").unwrap();
        assert!(enum_path.is_enum());
    }

    #[test]
    fn test_item_name_extraction() {
        let path = ItemPath::new("struct.HashMap.html").unwrap();
        assert_eq!(path.item_name(), Some("HashMap"));

        let path = ItemPath::new("collections/struct.HashMap.html").unwrap();
        assert_eq!(path.item_name(), Some("HashMap"));

        let path = ItemPath::new("HashMap").unwrap();
        assert_eq!(path.item_name(), Some("HashMap"));
    }
}
