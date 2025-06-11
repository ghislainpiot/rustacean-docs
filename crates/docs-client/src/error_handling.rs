use reqwest::Response;
use rustacean_docs_core::{ErrorBuilder, ErrorContext, Result};
use tracing::warn;
use url::Url;

/// Standard HTTP response handling for all endpoints
pub(crate) async fn handle_http_response(response: Response, context: &str) -> Result<Response> {
    let status = response.status();
    let url = response.url().clone();

    if status.is_success() {
        Ok(response)
    } else {
        let body = response.text().await.unwrap_or_default();
        warn!(
            url = %url,
            status = %status,
            context = %context,
            "HTTP request failed"
        );

        let error = match status.as_u16() {
            404 => ErrorBuilder::docs()
                .crate_not_found(rustacean_docs_core::CrateName::new("unknown").unwrap()),
            429 => ErrorBuilder::network().rate_limit(None),
            _ => ErrorBuilder::network().http_request(
                format!("{context}: HTTP {status}: {body}"),
                Some(status.as_u16()),
            ),
        };
        Err(error)
    }
}

/// Standard JSON parsing with context
pub(crate) async fn parse_json_response<T>(response: Response, context: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    response.json::<T>().await.map_err(|e| {
        ErrorBuilder::docs().parse_error(format!("Failed to parse JSON response: {context}: {e}"))
    })
}

/// Build docs.rs URL with standard error handling
pub(crate) fn build_docs_url(crate_name: &str, version: &str) -> Result<Url> {
    Url::parse(&format!(
        "https://docs.rs/{crate_name}/{version}/{crate_name}/"
    ))
    .context("Failed to construct docs.rs URL")
}

/// Build docs.rs URL for specific item with standard error handling
pub(crate) fn build_item_docs_url(crate_name: &str, version: &str, item_path: &str) -> Result<Url> {
    Url::parse(&format!(
        "https://docs.rs/{crate_name}/{version}/{crate_name}/{item_path}"
    ))
    .context("Failed to construct item docs URL")
}

/// Build basic docs.rs URL (no version, for documentation links)
pub(crate) fn build_basic_docs_url(crate_name: &str) -> Option<Url> {
    Url::parse(&format!("https://docs.rs/{crate_name}")).ok()
}

/// Standard cache error handling - logs but doesn't fail the operation
#[allow(dead_code)]
pub(crate) fn handle_cache_error(error: impl std::fmt::Display, operation: &str) {
    warn!(
        error = %error,
        operation = %operation,
        "Cache operation failed, continuing without cache"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    use reqwest::Client;

    #[tokio::test]
    async fn test_handle_http_response_success() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", "/test")
            .with_status(200)
            .with_body("success")
            .create_async()
            .await;

        let client = Client::new();
        let response = client
            .get(format!("{}/test", server.url()))
            .send()
            .await
            .unwrap();

        let result = handle_http_response(response, "test operation").await;
        assert!(result.is_ok());

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_handle_http_response_404() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", "/test")
            .with_status(404)
            .with_body("not found")
            .create_async()
            .await;

        let client = Client::new();
        let response = client
            .get(format!("{}/test", server.url()))
            .send()
            .await
            .unwrap();

        let result = handle_http_response(response, "test operation").await;
        assert!(result.is_err());

        match result.unwrap_err() {
            rustacean_docs_core::Error::Docs(docs_err) => {
                if let rustacean_docs_core::error::DocsError::CrateNotFound { .. } = docs_err {
                    // Expected
                } else {
                    panic!("Expected CrateNotFound error, got: {:?}", docs_err);
                }
            }
            other => panic!("Expected CrateNotFound error, got: {other:?}"),
        }

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_handle_http_response_rate_limit() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", "/test")
            .with_status(429)
            .with_body("rate limited")
            .create_async()
            .await;

        let client = Client::new();
        let response = client
            .get(format!("{}/test", server.url()))
            .send()
            .await
            .unwrap();

        let result = handle_http_response(response, "test operation").await;
        assert!(result.is_err());

        match result.unwrap_err() {
            rustacean_docs_core::Error::Network(network_err) => {
                if let rustacean_docs_core::error::NetworkError::RateLimit { .. } = network_err {
                    // Expected
                } else {
                    panic!("Expected RateLimit error, got: {:?}", network_err);
                }
            }
            other => panic!("Expected RateLimit error, got: {other:?}"),
        }

        mock.assert_async().await;
    }

    #[test]
    fn test_build_docs_url() {
        let url = build_docs_url("serde", "1.0.0").unwrap();
        assert_eq!(url.as_str(), "https://docs.rs/serde/1.0.0/serde/");
    }

    #[test]
    fn test_build_item_docs_url() {
        let url = build_item_docs_url("serde", "1.0.0", "trait.Serialize.html").unwrap();
        assert_eq!(
            url.as_str(),
            "https://docs.rs/serde/1.0.0/serde/trait.Serialize.html"
        );
    }
}
