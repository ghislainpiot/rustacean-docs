use thiserror::Error;

/// Network and HTTP-related errors
#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("HTTP request failed: {message}")]
    HttpRequest {
        message: String,
        status: Option<u16>,
    },

    #[error("Network error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("Connection timeout")]
    Timeout,

    #[error("Rate limit exceeded")]
    RateLimit { retry_after: Option<u64> },

    #[error("DNS resolution failed for {host}")]
    DnsResolution { host: String },

    #[error("Connection refused to {host}:{port}")]
    ConnectionRefused { host: String, port: u16 },
}

impl NetworkError {
    pub fn http_request(message: impl Into<String>, status: Option<u16>) -> Self {
        Self::HttpRequest {
            message: message.into(),
            status,
        }
    }

    pub fn rate_limit(retry_after: Option<u64>) -> Self {
        Self::RateLimit { retry_after }
    }

    pub fn dns_resolution(host: impl Into<String>) -> Self {
        Self::DnsResolution { host: host.into() }
    }

    pub fn connection_refused(host: impl Into<String>, port: u16) -> Self {
        Self::ConnectionRefused {
            host: host.into(),
            port,
        }
    }

    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            NetworkError::HttpRequest { status: Some(s), .. } if *s >= 500,
        ) || matches!(
            self,
            NetworkError::Timeout
                | NetworkError::ConnectionRefused { .. }
                | NetworkError::DnsResolution { .. }
        )
    }

    /// Check if this error is temporary
    pub fn is_temporary(&self) -> bool {
        matches!(
            self,
            NetworkError::Timeout
                | NetworkError::RateLimit { .. }
                | NetworkError::ConnectionRefused { .. }
        )
    }
}
