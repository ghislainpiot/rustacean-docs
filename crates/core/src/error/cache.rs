use thiserror::Error;

/// Cache-related errors
#[derive(Error, Debug)]
pub enum CacheError {
    #[error("Cache operation failed: {operation}")]
    OperationFailed { operation: String },

    #[error("Cache corruption detected: {details}")]
    Corruption { details: String },

    #[error("Disk cache error: {path} - {reason}")]
    DiskError { path: String, reason: String },

    #[error("Cache entry expired")]
    Expired,

    #[error("Cache full: {current_size} / {max_size} bytes")]
    Full { current_size: u64, max_size: u64 },

    #[error("Invalid cache key: {key}")]
    InvalidKey { key: String },

    #[error("Cache serialization failed")]
    SerializationError(#[from] serde_json::Error),
}

impl CacheError {
    pub fn operation_failed(operation: impl Into<String>) -> Self {
        Self::OperationFailed {
            operation: operation.into(),
        }
    }

    pub fn corruption(details: impl Into<String>) -> Self {
        Self::Corruption {
            details: details.into(),
        }
    }

    pub fn disk_error(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::DiskError {
            path: path.into(),
            reason: reason.into(),
        }
    }

    pub fn full(current_size: u64, max_size: u64) -> Self {
        Self::Full {
            current_size,
            max_size,
        }
    }

    pub fn invalid_key(key: impl Into<String>) -> Self {
        Self::InvalidKey { key: key.into() }
    }

    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            CacheError::OperationFailed { .. } | CacheError::DiskError { .. } | CacheError::Expired
        )
    }

    /// Check if this error is temporary
    pub fn is_temporary(&self) -> bool {
        matches!(
            self,
            CacheError::OperationFailed { .. } | CacheError::Full { .. }
        )
    }
}
