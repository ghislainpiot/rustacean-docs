use super::{CacheError, ConfigError, DocsError, Error, NetworkError, ProtocolError};
use crate::types::{CrateName, ItemPath, Version};

/// Builder for creating errors with a fluent API
pub struct ErrorBuilder;

impl ErrorBuilder {
    /// Network errors
    pub fn network() -> NetworkErrorBuilder {
        NetworkErrorBuilder
    }

    /// Documentation errors
    pub fn docs() -> DocsErrorBuilder {
        DocsErrorBuilder
    }

    /// Cache errors
    pub fn cache() -> CacheErrorBuilder {
        CacheErrorBuilder
    }

    /// Configuration errors
    pub fn config() -> ConfigErrorBuilder {
        ConfigErrorBuilder
    }

    /// Protocol errors
    pub fn protocol() -> ProtocolErrorBuilder {
        ProtocolErrorBuilder
    }

    /// Internal error
    pub fn internal(message: impl Into<String>) -> Error {
        Error::Internal(message.into())
    }
}

pub struct NetworkErrorBuilder;

impl NetworkErrorBuilder {
    pub fn http_request(self, message: impl Into<String>, status: Option<u16>) -> Error {
        NetworkError::http_request(message, status).into()
    }

    pub fn timeout(self) -> Error {
        NetworkError::Timeout.into()
    }

    pub fn rate_limit(self, retry_after: Option<u64>) -> Error {
        NetworkError::rate_limit(retry_after).into()
    }

    pub fn dns_resolution(self, host: impl Into<String>) -> Error {
        NetworkError::dns_resolution(host).into()
    }

    pub fn connection_refused(self, host: impl Into<String>, port: u16) -> Error {
        NetworkError::connection_refused(host, port).into()
    }
}

pub struct DocsErrorBuilder;

impl DocsErrorBuilder {
    pub fn parse_error(self, reason: impl Into<String>) -> Error {
        DocsError::parse_error(reason).into()
    }

    pub fn crate_not_found(self, crate_name: CrateName) -> Error {
        DocsError::crate_not_found(crate_name).into()
    }

    pub fn item_not_found(self, crate_name: CrateName, item_path: ItemPath) -> Error {
        DocsError::item_not_found(crate_name, item_path).into()
    }

    pub fn invalid_version(self, version: impl Into<String>) -> Error {
        DocsError::invalid_version(version).into()
    }

    pub fn version_not_found(self, crate_name: CrateName, version: Version) -> Error {
        DocsError::version_not_found(crate_name, version).into()
    }
}

pub struct CacheErrorBuilder;

impl CacheErrorBuilder {
    pub fn operation_failed(self, operation: impl Into<String>) -> Error {
        CacheError::operation_failed(operation).into()
    }

    pub fn corruption(self, details: impl Into<String>) -> Error {
        CacheError::corruption(details).into()
    }

    pub fn disk_error(self, path: impl Into<String>, reason: impl Into<String>) -> Error {
        CacheError::disk_error(path, reason).into()
    }

    pub fn expired(self) -> Error {
        CacheError::Expired.into()
    }

    pub fn full(self, current_size: u64, max_size: u64) -> Error {
        CacheError::full(current_size, max_size).into()
    }
}

pub struct ConfigErrorBuilder;

impl ConfigErrorBuilder {
    pub fn invalid_field(self, field: impl Into<String>, reason: impl Into<String>) -> Error {
        ConfigError::invalid_field(field, reason).into()
    }

    pub fn missing_field(self, field: impl Into<String>) -> Error {
        ConfigError::missing_field(field).into()
    }

    pub fn out_of_range(
        self,
        field: impl Into<String>,
        value: impl Into<String>,
        expected: impl Into<String>,
    ) -> Error {
        ConfigError::out_of_range(field, value, expected).into()
    }
}

pub struct ProtocolErrorBuilder;

impl ProtocolErrorBuilder {
    pub fn invalid_input(self, tool_name: impl Into<String>, reason: impl Into<String>) -> Error {
        ProtocolError::invalid_input(tool_name, reason).into()
    }

    pub fn tool_not_found(self, tool_name: impl Into<String>) -> Error {
        ProtocolError::tool_not_found(tool_name).into()
    }

    pub fn not_supported(self, operation: impl Into<String>) -> Error {
        ProtocolError::not_supported(operation).into()
    }
}
