use crate::types::{CrateName, ItemPath, Version};
use thiserror::Error;

/// Documentation-related errors
#[derive(Error, Debug)]
pub enum DocsError {
    #[error("Failed to parse documentation: {reason}")]
    ParseError { reason: String },

    #[error("Crate not found: {crate_name}")]
    CrateNotFound { crate_name: CrateName },

    #[error("Item not found: {item_path} in crate {crate_name}")]
    ItemNotFound {
        crate_name: CrateName,
        item_path: ItemPath,
    },

    #[error("Invalid version: {version}")]
    InvalidVersion { version: String },

    #[error("Version not found: {version} for crate {crate_name}")]
    VersionNotFound {
        crate_name: CrateName,
        version: Version,
    },

    #[error("Documentation format not supported: {format}")]
    UnsupportedFormat { format: String },

    #[error("Documentation extraction failed: {details}")]
    ExtractionFailed { details: String },
}

impl DocsError {
    pub fn parse_error(reason: impl Into<String>) -> Self {
        Self::ParseError {
            reason: reason.into(),
        }
    }

    pub fn crate_not_found(crate_name: CrateName) -> Self {
        Self::CrateNotFound { crate_name }
    }

    pub fn item_not_found(crate_name: CrateName, item_path: ItemPath) -> Self {
        Self::ItemNotFound {
            crate_name,
            item_path,
        }
    }

    pub fn invalid_version(version: impl Into<String>) -> Self {
        Self::InvalidVersion {
            version: version.into(),
        }
    }

    pub fn version_not_found(crate_name: CrateName, version: Version) -> Self {
        Self::VersionNotFound {
            crate_name,
            version,
        }
    }

    pub fn unsupported_format(format: impl Into<String>) -> Self {
        Self::UnsupportedFormat {
            format: format.into(),
        }
    }

    pub fn extraction_failed(details: impl Into<String>) -> Self {
        Self::ExtractionFailed {
            details: details.into(),
        }
    }
}
