use std::fmt;
use thiserror::Error;

/// Comprehensive error types for the Rustacean Docs MCP Server
#[derive(Error, Debug)]
pub enum Error {
    // Network and HTTP errors
    #[error("HTTP request failed: {message}")]
    HttpRequest {
        message: String,
        status: Option<u16>,
    },

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Connection timeout")]
    Timeout,

    #[error("Rate limit exceeded")]
    RateLimit,

    // Documentation and parsing errors
    #[error("Failed to parse documentation: {reason}")]
    DocumentationParse { reason: String },

    #[error("Crate not found: {crate_name}")]
    CrateNotFound { crate_name: String },

    #[error("Item not found: {item_path} in crate {crate_name}")]
    ItemNotFound {
        crate_name: String,
        item_path: String,
    },

    #[error("Invalid version format: {version}")]
    InvalidVersion { version: String },

    // Cache errors
    #[error("Cache operation failed: {operation}")]
    Cache { operation: String },

    #[error("Cache corruption detected: {details}")]
    CacheCorruption { details: String },

    #[error("Disk cache error: {path} - {reason}")]
    DiskCache { path: String, reason: String },

    // Serialization and data errors
    #[error("JSON serialization failed")]
    Serialization(#[from] serde_json::Error),

    #[error("URL parsing failed")]
    UrlParse(#[from] url::ParseError),

    // I/O errors
    #[error("I/O operation failed")]
    Io(#[from] std::io::Error),

    // Configuration errors
    #[error("Invalid configuration: {field} - {reason}")]
    Configuration { field: String, reason: String },

    // MCP protocol errors
    #[error("MCP protocol error: {message}")]
    McpProtocol { message: String },

    #[error("Invalid tool input: {tool_name} - {reason}")]
    InvalidInput { tool_name: String, reason: String },

    // Generic errors
    #[error("Internal error: {message}")]
    Internal { message: String },

    #[error("Operation not supported: {operation}")]
    NotSupported { operation: String },
}

impl Error {
    /// Create a new HTTP request error with optional status code
    pub fn http_request(message: impl Into<String>, status: Option<u16>) -> Self {
        Self::HttpRequest {
            message: message.into(),
            status,
        }
    }

    /// Create a new documentation parsing error
    pub fn documentation_parse(reason: impl Into<String>) -> Self {
        Self::DocumentationParse {
            reason: reason.into(),
        }
    }

    /// Create a new crate not found error
    pub fn crate_not_found(crate_name: impl Into<String>) -> Self {
        Self::CrateNotFound {
            crate_name: crate_name.into(),
        }
    }

    /// Create a new item not found error
    pub fn item_not_found(crate_name: impl Into<String>, item_path: impl Into<String>) -> Self {
        Self::ItemNotFound {
            crate_name: crate_name.into(),
            item_path: item_path.into(),
        }
    }

    /// Create a new invalid version error
    pub fn invalid_version(version: impl Into<String>) -> Self {
        Self::InvalidVersion {
            version: version.into(),
        }
    }

    /// Create a new cache operation error
    pub fn cache_operation(operation: impl Into<String>) -> Self {
        Self::Cache {
            operation: operation.into(),
        }
    }

    /// Create a new cache corruption error
    pub fn cache_corruption(details: impl Into<String>) -> Self {
        Self::CacheCorruption {
            details: details.into(),
        }
    }

    /// Create a new disk cache error
    pub fn disk_cache(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::DiskCache {
            path: path.into(),
            reason: reason.into(),
        }
    }

    /// Create a new configuration error
    pub fn configuration(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Configuration {
            field: field.into(),
            reason: reason.into(),
        }
    }

    /// Create a new MCP protocol error
    pub fn mcp_protocol(message: impl Into<String>) -> Self {
        Self::McpProtocol {
            message: message.into(),
        }
    }

    /// Create a new invalid input error
    pub fn invalid_input(tool_name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidInput {
            tool_name: tool_name.into(),
            reason: reason.into(),
        }
    }

    /// Create a new internal error
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    /// Create a new not supported error
    pub fn not_supported(operation: impl Into<String>) -> Self {
        Self::NotSupported {
            operation: operation.into(),
        }
    }

    /// Check if this error is recoverable (can retry the operation)
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Error::HttpRequest { .. }
                | Error::Network(_)
                | Error::Timeout
                | Error::Cache { .. }
                | Error::DiskCache { .. }
                | Error::Io(_)
        )
    }

    /// Check if this error indicates a temporary issue
    pub fn is_temporary(&self) -> bool {
        matches!(
            self,
            Error::Network(_) | Error::Timeout | Error::RateLimit | Error::Cache { .. }
        )
    }

    /// Get error category for logging and metrics
    pub fn category(&self) -> ErrorCategory {
        match self {
            Error::HttpRequest { .. } | Error::Network(_) | Error::Timeout | Error::RateLimit => {
                ErrorCategory::Network
            }
            Error::DocumentationParse { .. }
            | Error::CrateNotFound { .. }
            | Error::ItemNotFound { .. }
            | Error::InvalidVersion { .. } => ErrorCategory::Documentation,
            Error::Cache { .. } | Error::CacheCorruption { .. } | Error::DiskCache { .. } => {
                ErrorCategory::Cache
            }
            Error::Serialization(_) | Error::UrlParse(_) => ErrorCategory::Data,
            Error::Io(_) => ErrorCategory::Io,
            Error::Configuration { .. } => ErrorCategory::Configuration,
            Error::McpProtocol { .. } | Error::InvalidInput { .. } => ErrorCategory::Protocol,
            Error::Internal { .. } | Error::NotSupported { .. } => ErrorCategory::Internal,
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

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

/// Extension trait for adding context to errors
pub trait ErrorContext<T> {
    /// Add context to an error
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;

    /// Add context to an error with a static string
    fn context(self, msg: &'static str) -> Result<T>;
}

impl<T, E> ErrorContext<T> for std::result::Result<T, E>
where
    E: Into<Error>,
{
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| {
            let original_error = e.into();
            Error::internal(format!("{}: {}", f(), original_error))
        })
    }

    fn context(self, msg: &'static str) -> Result<T> {
        self.with_context(|| msg.to_string())
    }
}

impl<T> ErrorContext<T> for Option<T> {
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.ok_or_else(|| Error::internal(f()))
    }

    fn context(self, msg: &'static str) -> Result<T> {
        self.with_context(|| msg.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_error_constructors() {
        let err = Error::http_request("Connection failed", Some(500));
        assert!(matches!(
            err,
            Error::HttpRequest {
                status: Some(500),
                ..
            }
        ));

        let err = Error::crate_not_found("serde");
        assert!(matches!(err, Error::CrateNotFound { .. }));
        assert_eq!(err.to_string(), "Crate not found: serde");

        let err = Error::item_not_found("serde", "Serialize");
        assert!(matches!(err, Error::ItemNotFound { .. }));
        assert_eq!(err.to_string(), "Item not found: Serialize in crate serde");

        let err = Error::invalid_version("1.0.invalid");
        assert!(matches!(err, Error::InvalidVersion { .. }));

        let err = Error::cache_operation("write");
        assert!(matches!(err, Error::Cache { .. }));

        let err = Error::configuration("timeout", "must be positive");
        assert!(matches!(err, Error::Configuration { .. }));
    }

    #[test]
    fn test_error_categorization() {
        let network_err = Error::http_request("timeout", None);
        assert_eq!(network_err.category(), ErrorCategory::Network);
        assert!(network_err.is_recoverable());
        assert!(!network_err.is_temporary()); // HTTP errors are not always temporary

        let doc_err = Error::crate_not_found("missing");
        assert_eq!(doc_err.category(), ErrorCategory::Documentation);
        assert!(!doc_err.is_recoverable());
        assert!(!doc_err.is_temporary());

        let cache_err = Error::cache_operation("flush");
        assert_eq!(cache_err.category(), ErrorCategory::Cache);
        assert!(cache_err.is_recoverable());
        assert!(cache_err.is_temporary());

        let config_err = Error::configuration("port", "invalid");
        assert_eq!(config_err.category(), ErrorCategory::Configuration);
        assert!(!config_err.is_recoverable());
        assert!(!config_err.is_temporary());
    }

    #[test]
    fn test_error_from_conversions() {
        // Test serde_json::Error conversion
        let json_err = serde_json::from_str::<i32>("invalid json").unwrap_err();
        let our_err: Error = json_err.into();
        assert!(matches!(our_err, Error::Serialization(_)));
        assert_eq!(our_err.category(), ErrorCategory::Data);

        // Test io::Error conversion
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let our_err: Error = io_err.into();
        assert!(matches!(our_err, Error::Io(_)));
        assert_eq!(our_err.category(), ErrorCategory::Io);

        // Test url::ParseError conversion
        let url_err = url::Url::parse("not a url").unwrap_err();
        let our_err: Error = url_err.into();
        assert!(matches!(our_err, Error::UrlParse(_)));
        assert_eq!(our_err.category(), ErrorCategory::Data);
    }

    #[test]
    fn test_error_context_trait() {
        // Test with Result
        let result: Result<i32> = Err(Error::internal("base error"));
        let with_context = result.context("operation failed");
        assert!(with_context.is_err());
        let err = with_context.unwrap_err();
        assert!(err.to_string().contains("operation failed"));
        assert!(err.to_string().contains("base error"));

        // Test with Option
        let option: Option<i32> = None;
        let with_context = option.context("value was None");
        assert!(with_context.is_err());
        let err = with_context.unwrap_err();
        assert!(err.to_string().contains("value was None"));

        // Test with_context closure
        let result: Result<i32> = Err(Error::Timeout);
        let with_context = result.with_context(|| format!("failed at step {}", 42));
        assert!(with_context.is_err());
        let err = with_context.unwrap_err();
        assert!(err.to_string().contains("failed at step 42"));
    }

    #[test]
    fn test_error_category_display() {
        assert_eq!(ErrorCategory::Network.to_string(), "network");
        assert_eq!(ErrorCategory::Documentation.to_string(), "documentation");
        assert_eq!(ErrorCategory::Cache.to_string(), "cache");
        assert_eq!(ErrorCategory::Data.to_string(), "data");
        assert_eq!(ErrorCategory::Io.to_string(), "io");
        assert_eq!(ErrorCategory::Configuration.to_string(), "configuration");
        assert_eq!(ErrorCategory::Protocol.to_string(), "protocol");
        assert_eq!(ErrorCategory::Internal.to_string(), "internal");
    }

    #[test]
    fn test_error_recovery_classification() {
        // Recoverable errors
        let recoverable_errors = vec![
            Error::http_request("timeout", None),
            Error::Timeout,
            Error::cache_operation("write"),
            Error::disk_cache("/tmp/cache", "permission denied"),
            Error::Io(io::Error::new(io::ErrorKind::TimedOut, "timeout")),
        ];

        for err in recoverable_errors {
            assert!(err.is_recoverable(), "Error should be recoverable: {err}");
        }

        // Non-recoverable errors
        let non_recoverable_errors = vec![
            Error::crate_not_found("missing"),
            Error::invalid_version("bad"),
            Error::configuration("port", "invalid"),
            Error::mcp_protocol("invalid request"),
        ];

        for err in non_recoverable_errors {
            assert!(
                !err.is_recoverable(),
                "Error should not be recoverable: {err}"
            );
        }
    }

    #[test]
    fn test_temporary_error_classification() {
        // Temporary errors
        let temporary_errors = vec![
            Error::Timeout,
            Error::RateLimit,
            Error::cache_operation("flush"),
        ];

        for err in temporary_errors {
            assert!(err.is_temporary(), "Error should be temporary: {err}");
        }

        // Non-temporary errors
        let permanent_errors = vec![
            Error::crate_not_found("missing"),
            Error::invalid_version("bad"),
            Error::configuration("port", "invalid"),
            Error::cache_corruption("data malformed"),
        ];

        for err in permanent_errors {
            assert!(!err.is_temporary(), "Error should not be temporary: {err}");
        }
    }
}
