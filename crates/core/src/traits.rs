use crate::Result;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// Trait for all request types in the API
pub trait Request: Debug + Send + Sync {
    /// The response type associated with this request
    type Response: Response;

    /// Validate the request parameters
    fn validate(&self) -> Result<()>;

    /// Get a unique cache key for this request
    fn cache_key(&self) -> Option<String> {
        None
    }
}

/// Trait for all response types in the API
pub trait Response: Debug + Send + Sync + Serialize + for<'de> Deserialize<'de> {
    /// Get the cache TTL for this response in seconds
    fn cache_ttl(&self) -> Option<u64> {
        None
    }
}

/// Trait for types that can be cached
pub trait Cacheable: Serialize + for<'de> Deserialize<'de> {
    /// Generate a unique cache key
    fn cache_key(&self) -> String;

    /// Get the TTL (time-to-live) in seconds
    fn ttl_seconds(&self) -> u64;

    /// Optional cache tags for invalidation
    fn cache_tags(&self) -> Vec<String> {
        Vec::new()
    }
}

/// Builder trait for creating requests
pub trait RequestBuilder {
    type Request: Request;

    /// Build the request, consuming the builder
    fn build(self) -> Result<Self::Request>;
}

/// Trait for paginated requests
pub trait PaginatedRequest: Request {
    /// Get the limit for this request
    fn limit(&self) -> usize;

    /// Get the offset for this request
    fn offset(&self) -> usize {
        0
    }
}

/// Trait for versioned requests
pub trait VersionedRequest: Request {
    /// Get the version for this request
    fn version(&self) -> Option<&str>;
}
