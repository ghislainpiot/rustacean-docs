mod builder;
mod cache;
mod config;
mod context;
mod docs;
mod network;
mod protocol;

pub use builder::ErrorBuilder;
pub use cache::CacheError;
pub use config::ConfigError;
pub use context::{ErrorContext, StructuredContext};
pub use docs::DocsError;
pub use network::NetworkError;
pub use protocol::ProtocolError;

use thiserror::Error;

/// Main error type that encompasses all domain-specific errors
#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] NetworkError),

    #[error(transparent)]
    Docs(#[from] DocsError),

    #[error(transparent)]
    Cache(#[from] CacheError),

    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    Protocol(#[from] ProtocolError),

    #[error("Serialization error")]
    Serialization(#[from] serde_json::Error),

    #[error("URL parsing error")]
    UrlParse(#[from] url::ParseError),

    #[error("I/O error")]
    Io(#[from] std::io::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl Error {
    /// Check if this error is recoverable (can retry the operation)
    pub fn is_recoverable(&self) -> bool {
        match self {
            Error::Network(e) => e.is_recoverable(),
            Error::Cache(e) => e.is_recoverable(),
            Error::Io(_) => true,
            _ => false,
        }
    }

    /// Check if this error indicates a temporary issue
    pub fn is_temporary(&self) -> bool {
        match self {
            Error::Network(e) => e.is_temporary(),
            Error::Cache(e) => e.is_temporary(),
            _ => false,
        }
    }

    /// Get error category for logging and metrics
    pub fn category(&self) -> ErrorCategory {
        match self {
            Error::Network(_) => ErrorCategory::Network,
            Error::Docs(_) => ErrorCategory::Documentation,
            Error::Cache(_) => ErrorCategory::Cache,
            Error::Config(_) => ErrorCategory::Configuration,
            Error::Protocol(_) => ErrorCategory::Protocol,
            Error::Serialization(_) | Error::UrlParse(_) => ErrorCategory::Data,
            Error::Io(_) => ErrorCategory::Io,
            Error::Internal(_) => ErrorCategory::Internal,
        }
    }
}

/// Error categories for classification and metrics
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCategory {
    Network,
    Documentation,
    Cache,
    Data,
    Io,
    Configuration,
    Protocol,
    Internal,
}

impl std::fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorCategory::Network => write!(f, "network"),
            ErrorCategory::Documentation => write!(f, "documentation"),
            ErrorCategory::Cache => write!(f, "cache"),
            ErrorCategory::Data => write!(f, "data"),
            ErrorCategory::Io => write!(f, "io"),
            ErrorCategory::Configuration => write!(f, "configuration"),
            ErrorCategory::Protocol => write!(f, "protocol"),
            ErrorCategory::Internal => write!(f, "internal"),
        }
    }
}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, Error>;
