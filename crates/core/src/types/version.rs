use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Version(String);

impl Version {
    pub fn new(version: impl Into<String>) -> Result<Self, VersionError> {
        let version = version.into();
        Self::validate(&version)?;
        Ok(Self(version))
    }

    pub fn latest() -> Self {
        Self("latest".to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(version: &str) -> Result<(), VersionError> {
        if version.is_empty() {
            return Err(VersionError::Empty);
        }

        if version == "latest" {
            return Ok(());
        }

        // Basic semver validation
        let parts: Vec<&str> = version.split('.').collect();
        if parts.is_empty() || parts.len() > 4 {
            return Err(VersionError::InvalidFormat(version.to_string()));
        }

        // Check that each part is numeric (except for pre-release)
        for (i, part) in parts.iter().enumerate() {
            if i < 3 {
                // Major, minor, patch should be numeric
                if part.parse::<u32>().is_err() {
                    // Check if it contains pre-release info
                    let numeric_part = part.split('-').next().unwrap_or("");
                    if numeric_part.parse::<u32>().is_err() {
                        return Err(VersionError::InvalidFormat(version.to_string()));
                    }
                }
            }
        }

        Ok(())
    }
}

impl Default for Version {
    fn default() -> Self {
        Self::latest()
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Version {
    type Err = VersionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl From<Version> for String {
    fn from(version: Version) -> Self {
        version.0
    }
}

impl AsRef<str> for Version {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VersionError {
    #[error("Version cannot be empty")]
    Empty,
    #[error("Invalid version format: {0}")]
    InvalidFormat(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_versions() {
        assert!(Version::new("1.0.0").is_ok());
        assert!(Version::new("1.0.0-alpha").is_ok());
        assert!(Version::new("2.1.3-beta.1").is_ok());
        assert!(Version::new("latest").is_ok());
        assert!(Version::new("0.1.0").is_ok());
    }

    #[test]
    fn test_invalid_versions() {
        assert!(Version::new("").is_err());
        assert!(Version::new("abc").is_err());
        assert!(Version::new("1.a.0").is_err());
        assert!(Version::new("1.2.3.4.5").is_err());
    }

    #[test]
    fn test_default() {
        assert_eq!(Version::default().as_str(), "latest");
    }

    #[test]
    fn test_display() {
        let version = Version::new("1.2.3").unwrap();
        assert_eq!(version.to_string(), "1.2.3");
    }

    #[test]
    fn test_from_str() {
        let version: Version = "1.2.3".parse().unwrap();
        assert_eq!(version.as_str(), "1.2.3");
    }
}
