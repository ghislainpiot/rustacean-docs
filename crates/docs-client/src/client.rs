use reqwest::{header, Client, ClientBuilder, Response};
use rustacean_docs_core::{error::ErrorContext, Result};
use std::time::Duration;
use tracing::{debug, trace, warn};

/// Configuration for the HTTP client
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// User agent string for requests
    pub user_agent: String,
    /// Request timeout in seconds
    pub timeout: Duration,
    /// Connection timeout in seconds
    pub connect_timeout: Duration,
    /// Maximum number of redirects to follow
    pub max_redirects: usize,
    /// Whether to use gzip compression
    pub gzip: bool,
    /// Pool idle timeout
    pub pool_idle_timeout: Duration,
    /// Maximum idle connections per host
    pub pool_max_idle_per_host: usize,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            user_agent: "rustacean-docs-mcp/0.1.0".to_string(),
            timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            max_redirects: 5,
            gzip: true,
            pool_idle_timeout: Duration::from_secs(90),
            pool_max_idle_per_host: 10,
        }
    }
}

/// HTTP client for interacting with docs.rs and related APIs
#[derive(Debug, Clone)]
pub struct DocsClient {
    client: Client,
    config: ClientConfig,
    base_url: String,
}

impl DocsClient {
    /// Create a new client with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(ClientConfig::default())
    }

    /// Create a new client with custom configuration
    pub fn with_config(config: ClientConfig) -> Result<Self> {
        let client = Self::build_client(&config, false)?;

        debug!(
            user_agent = %config.user_agent,
            timeout_secs = config.timeout.as_secs(),
            connect_timeout_secs = config.connect_timeout.as_secs(),
            max_redirects = config.max_redirects,
            gzip = config.gzip,
            "Created HTTP client with configuration"
        );

        Ok(Self {
            client,
            config,
            base_url: "https://docs.rs".to_string(),
        })
    }

    /// Build an HTTP client with the given configuration
    fn build_client(config: &ClientConfig, allow_http: bool) -> Result<Client> {
        let mut headers = header::HeaderMap::new();

        // Set user agent
        headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_str(&config.user_agent).map_err(|e| {
                rustacean_docs_core::Error::internal(format!("Invalid user agent string: {e}"))
            })?,
        );

        // Set accept header to prefer JSON when available
        headers.insert(
            header::ACCEPT,
            header::HeaderValue::from_static("application/json, text/html, */*"),
        );

        // Note: We don't manually set Accept-Encoding header here.
        // reqwest automatically handles gzip compression when we don't set it manually,
        // and will automatically decompress responses for us.

        // Build the client with configuration
        let mut client_builder = ClientBuilder::new()
            .default_headers(headers)
            .timeout(config.timeout)
            .connect_timeout(config.connect_timeout)
            .redirect(reqwest::redirect::Policy::limited(config.max_redirects))
            .pool_idle_timeout(config.pool_idle_timeout)
            .pool_max_idle_per_host(config.pool_max_idle_per_host);

        // Only use HTTPS for security (unless explicitly allowing HTTP for testing)
        if !allow_http {
            client_builder = client_builder.https_only(true);
        }

        // Add gzip support conditionally (reqwest enables it by default, disable if needed)
        if !config.gzip {
            client_builder = client_builder.no_gzip();
        }

        client_builder
            .build()
            .context("Failed to build HTTP client")
    }

    /// Get a reference to the client configuration
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// Get the base URL being used
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Set a custom base URL (useful for testing)
    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    /// Create a test client that allows HTTP (for testing with mock servers)
    #[cfg(test)]
    pub fn test_client() -> Result<Self> {
        let config = ClientConfig::default();
        let client = Self::build_client(&config, true)?; // Allow HTTP for testing

        Ok(Self {
            client,
            config,
            base_url: "https://docs.rs".to_string(),
        })
    }

    /// Perform a GET request to the specified path
    pub async fn get(&self, path: &str) -> Result<Response> {
        let url = format!("{}{}", self.base_url, path);

        trace!(url = %url, "Making GET request");

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to send GET request")?;

        let status = response.status();

        if status.is_success() {
            debug!(
                url = %url,
                status = %status,
                "GET request successful"
            );
            Ok(response)
        } else if status.is_client_error() {
            warn!(
                url = %url,
                status = %status,
                "GET request failed with client error"
            );
            Err(rustacean_docs_core::Error::http_request(
                format!("Client error: {status}"),
                Some(status.as_u16()),
            ))
        } else if status.is_server_error() {
            warn!(
                url = %url,
                status = %status,
                "GET request failed with server error"
            );
            Err(rustacean_docs_core::Error::http_request(
                format!("Server error: {status}"),
                Some(status.as_u16()),
            ))
        } else {
            warn!(
                url = %url,
                status = %status,
                "GET request failed with unexpected status"
            );
            Err(rustacean_docs_core::Error::http_request(
                format!("Unexpected status: {status}"),
                Some(status.as_u16()),
            ))
        }
    }

    /// Perform a GET request and return the response as text
    pub async fn get_text(&self, path: &str) -> Result<String> {
        let response = self.get(path).await?;
        let text = response
            .text()
            .await
            .context("Failed to read response as text")?;

        trace!(
            path = %path,
            text_length = text.len(),
            "Retrieved text response"
        );

        Ok(text)
    }

    /// Perform a GET request and return the response as JSON
    pub async fn get_json<T>(&self, path: &str) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let response = self.get(path).await?;
        let json = response
            .json::<T>()
            .await
            .context("Failed to parse response as JSON")?;

        trace!(
            path = %path,
            type_name = std::any::type_name::<T>(),
            "Retrieved JSON response"
        );

        Ok(json)
    }

    /// Get a reference to the internal reqwest client
    pub fn inner_client(&self) -> &Client {
        &self.client
    }

    /// Check if the client can connect to the base URL
    pub async fn health_check(&self) -> Result<bool> {
        trace!("Performing health check");

        match self.get("/").await {
            Ok(_) => {
                debug!("Health check passed");
                Ok(true)
            }
            Err(e) => {
                warn!(error = %e, "Health check failed");
                Ok(false)
            }
        }
    }
}

impl Default for DocsClient {
    fn default() -> Self {
        Self::new().expect("Failed to create default HTTP client")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_client_config_default() {
        let config = ClientConfig::default();
        assert_eq!(config.user_agent, "rustacean-docs-mcp/0.1.0");
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.max_redirects, 5);
        assert!(config.gzip);
        assert_eq!(config.pool_idle_timeout, Duration::from_secs(90));
        assert_eq!(config.pool_max_idle_per_host, 10);
    }

    #[test]
    fn test_client_config_custom() {
        let config = ClientConfig {
            user_agent: "test-agent/1.0".to_string(),
            timeout: Duration::from_secs(60),
            connect_timeout: Duration::from_secs(5),
            max_redirects: 3,
            gzip: false,
            pool_idle_timeout: Duration::from_secs(120),
            pool_max_idle_per_host: 5,
        };

        assert_eq!(config.user_agent, "test-agent/1.0");
        assert_eq!(config.timeout, Duration::from_secs(60));
        assert_eq!(config.connect_timeout, Duration::from_secs(5));
        assert_eq!(config.max_redirects, 3);
        assert!(!config.gzip);
        assert_eq!(config.pool_idle_timeout, Duration::from_secs(120));
        assert_eq!(config.pool_max_idle_per_host, 5);
    }

    #[test]
    fn test_docs_client_creation() {
        let client = DocsClient::new();
        assert!(client.is_ok());

        let client = client.unwrap();
        assert_eq!(client.base_url(), "https://docs.rs");
        assert_eq!(client.config().user_agent, "rustacean-docs-mcp/0.1.0");
    }

    #[test]
    fn test_docs_client_with_custom_config() {
        let config = ClientConfig {
            user_agent: "custom-agent/2.0".to_string(),
            timeout: Duration::from_secs(45),
            ..Default::default()
        };

        let client = DocsClient::with_config(config.clone());
        assert!(client.is_ok());

        let client = client.unwrap();
        assert_eq!(client.config().user_agent, "custom-agent/2.0");
        assert_eq!(client.config().timeout, Duration::from_secs(45));
    }

    #[test]
    fn test_docs_client_with_base_url() {
        let client = DocsClient::new().unwrap();
        let custom_client = client.with_base_url("https://test.example.com".to_string());
        assert_eq!(custom_client.base_url(), "https://test.example.com");
    }

    #[test]
    fn test_client_config_invalid_user_agent() {
        let config = ClientConfig {
            user_agent: "invalid\x00agent".to_string(), // Contains null byte
            ..Default::default()
        };

        let client = DocsClient::with_config(config);
        assert!(client.is_err());
    }

    #[test]
    fn test_default_client() {
        let client = DocsClient::default();
        assert_eq!(client.base_url(), "https://docs.rs");
        assert_eq!(client.config().user_agent, "rustacean-docs-mcp/0.1.0");
    }

    // Integration tests with mock server
    #[cfg(feature = "integration-tests")]
    mod integration_tests {
        use super::*;
        use mockito::Server;
        use serde_json::json;

        #[tokio::test]
        async fn test_get_request_success() {
            let mut server = Server::new_async().await;
            let mock = server
                .mock("GET", "/test")
                .with_status(200)
                .with_header("content-type", "text/plain")
                .with_body("Hello, world!")
                .create_async()
                .await;

            let client = DocsClient::test_client()
                .unwrap()
                .with_base_url(server.url());

            let response = client.get("/test").await;
            assert!(response.is_ok());

            let response = response.unwrap();
            assert_eq!(response.status(), 200);

            mock.assert_async().await;
        }

        #[tokio::test]
        async fn test_get_text_success() {
            let mut server = Server::new_async().await;
            let mock = server
                .mock("GET", "/text")
                .with_status(200)
                .with_header("content-type", "text/plain")
                .with_body("Test response")
                .create_async()
                .await;

            let client = DocsClient::test_client()
                .unwrap()
                .with_base_url(server.url());

            let text = client.get_text("/text").await;
            assert!(text.is_ok());
            assert_eq!(text.unwrap(), "Test response");

            mock.assert_async().await;
        }

        #[tokio::test]
        async fn test_get_json_success() {
            let mut server = Server::new_async().await;
            let response_json = json!({
                "name": "test",
                "version": "1.0.0"
            });

            let mock = server
                .mock("GET", "/json")
                .with_status(200)
                .with_header("content-type", "application/json")
                .with_body(response_json.to_string())
                .create_async()
                .await;

            let client = DocsClient::test_client()
                .unwrap()
                .with_base_url(server.url());

            let result: serde_json::Value = client.get_json("/json").await.unwrap();
            assert_eq!(result["name"], "test");
            assert_eq!(result["version"], "1.0.0");

            mock.assert_async().await;
        }

        #[tokio::test]
        async fn test_get_request_client_error() {
            let mut server = Server::new_async().await;
            let mock = server
                .mock("GET", "/notfound")
                .with_status(404)
                .create_async()
                .await;

            let client = DocsClient::test_client()
                .unwrap()
                .with_base_url(server.url());

            let response = client.get("/notfound").await;
            assert!(response.is_err());

            let error = response.unwrap_err();
            match error {
                rustacean_docs_core::Error::HttpRequest { status, .. } => {
                    assert_eq!(status, Some(404));
                }
                _ => panic!("Expected HttpRequest error, got: {:?}", error),
            }

            mock.assert_async().await;
        }

        #[tokio::test]
        async fn test_get_request_server_error() {
            let mut server = Server::new_async().await;
            let mock = server
                .mock("GET", "/error")
                .with_status(500)
                .create_async()
                .await;

            let client = DocsClient::test_client()
                .unwrap()
                .with_base_url(server.url());

            let response = client.get("/error").await;
            assert!(response.is_err());

            let error = response.unwrap_err();
            match error {
                rustacean_docs_core::Error::HttpRequest { status, .. } => {
                    assert_eq!(status, Some(500));
                }
                _ => panic!("Expected HttpRequest error, got: {:?}", error),
            }

            mock.assert_async().await;
        }

        #[tokio::test]
        async fn test_client_headers() {
            let mut server = Server::new_async().await;
            let mock = server
                .mock("GET", "/headers")
                .match_header("user-agent", "rustacean-docs-mcp/0.1.0")
                .match_header("accept", "application/json, text/html, */*")
                .match_header("accept-encoding", "gzip, deflate")
                .with_status(200)
                .with_body("OK")
                .create_async()
                .await;

            let client = DocsClient::test_client()
                .unwrap()
                .with_base_url(server.url());

            let response = client.get("/headers").await;
            assert!(response.is_ok());

            mock.assert_async().await;
        }

        #[tokio::test]
        async fn test_custom_user_agent() {
            let mut server = Server::new_async().await;
            let mock = server
                .mock("GET", "/custom")
                .match_header("user-agent", "custom-agent/2.0")
                .with_status(200)
                .with_body("OK")
                .create_async()
                .await;

            // Create a test client with custom config
            let config = ClientConfig {
                user_agent: "custom-agent/2.0".to_string(),
                ..Default::default()
            };

            let mut headers = header::HeaderMap::new();
            headers.insert(
                header::USER_AGENT,
                header::HeaderValue::from_str(&config.user_agent).unwrap(),
            );
            headers.insert(
                header::ACCEPT,
                header::HeaderValue::from_static("application/json, text/html, */*"),
            );
            headers.insert(
                header::ACCEPT_ENCODING,
                header::HeaderValue::from_static("gzip, deflate"),
            );

            let reqwest_client = ClientBuilder::new()
                .default_headers(headers)
                .timeout(config.timeout)
                .connect_timeout(config.connect_timeout)
                .redirect(reqwest::redirect::Policy::limited(config.max_redirects))
                .pool_idle_timeout(config.pool_idle_timeout)
                .pool_max_idle_per_host(config.pool_max_idle_per_host)
                .build()
                .unwrap();

            let client = DocsClient {
                client: reqwest_client,
                config,
                base_url: server.url(),
            };

            let response = client.get("/custom").await;
            assert!(response.is_ok());

            mock.assert_async().await;
        }

        #[tokio::test]
        async fn test_health_check_success() {
            let mut server = Server::new_async().await;
            let mock = server
                .mock("GET", "/")
                .with_status(200)
                .with_body("docs.rs")
                .create_async()
                .await;

            let client = DocsClient::test_client()
                .unwrap()
                .with_base_url(server.url());

            let health = client.health_check().await;
            assert!(health.is_ok());
            assert!(health.unwrap());

            mock.assert_async().await;
        }

        #[tokio::test]
        async fn test_health_check_failure() {
            let mut server = Server::new_async().await;
            let mock = server
                .mock("GET", "/")
                .with_status(500)
                .create_async()
                .await;

            let client = DocsClient::test_client()
                .unwrap()
                .with_base_url(server.url());

            let health = client.health_check().await;
            assert!(health.is_ok());
            assert!(!health.unwrap()); // Health check should return false, not error

            mock.assert_async().await;
        }
    }
}
