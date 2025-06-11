use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CrateName(String);

impl CrateName {
    pub fn new(name: impl Into<String>) -> Result<Self, CrateNameError> {
        let name = name.into();
        Self::validate(&name)?;
        Ok(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(name: &str) -> Result<(), CrateNameError> {
        if name.is_empty() {
            return Err(CrateNameError::Empty);
        }

        if name.len() > 64 {
            return Err(CrateNameError::TooLong(name.len()));
        }

        // Crate names must start with a letter
        if !name.chars().next().unwrap().is_alphabetic() {
            return Err(CrateNameError::InvalidStart);
        }

        // Crate names can only contain alphanumeric characters, - and _
        for ch in name.chars() {
            if !ch.is_alphanumeric() && ch != '-' && ch != '_' {
                return Err(CrateNameError::InvalidCharacter(ch));
            }
        }

        // Cannot end with - or _
        if name.ends_with('-') || name.ends_with('_') {
            return Err(CrateNameError::InvalidEnd);
        }

        Ok(())
    }
}

impl fmt::Display for CrateName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for CrateName {
    type Err = CrateNameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl From<CrateName> for String {
    fn from(name: CrateName) -> Self {
        name.0
    }
}

impl AsRef<str> for CrateName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CrateNameError {
    #[error("Crate name cannot be empty")]
    Empty,
    #[error("Crate name is too long ({0} > 64 characters)")]
    TooLong(usize),
    #[error("Crate name must start with a letter")]
    InvalidStart,
    #[error("Crate name cannot end with '-' or '_'")]
    InvalidEnd,
    #[error("Invalid character '{0}' in crate name")]
    InvalidCharacter(char),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_crate_names() {
        assert!(CrateName::new("serde").is_ok());
        assert!(CrateName::new("tokio").is_ok());
        assert!(CrateName::new("async-trait").is_ok());
        assert!(CrateName::new("my_crate").is_ok());
        assert!(CrateName::new("a123").is_ok());
    }

    #[test]
    fn test_invalid_crate_names() {
        assert!(CrateName::new("").is_err());
        assert!(CrateName::new("123crate").is_err());
        assert!(CrateName::new("-crate").is_err());
        assert!(CrateName::new("crate-").is_err());
        assert!(CrateName::new("crate_").is_err());
        assert!(CrateName::new("my.crate").is_err());
        assert!(CrateName::new("my crate").is_err());

        let long_name = "a".repeat(65);
        assert!(CrateName::new(long_name).is_err());
    }

    #[test]
    fn test_display() {
        let name = CrateName::new("serde").unwrap();
        assert_eq!(name.to_string(), "serde");
    }

    #[test]
    fn test_from_str() {
        let name: CrateName = "tokio".parse().unwrap();
        assert_eq!(name.as_str(), "tokio");
    }
}
