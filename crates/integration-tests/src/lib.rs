//! Integration tests library for Rustacean Docs MCP Server
//!
//! This crate contains shared utilities and helpers for integration testing.

pub mod common;

// Re-export commonly used types for tests
pub use rustacean_docs_cache::MemoryCache;
pub use rustacean_docs_client::DocsClient;
pub use rustacean_docs_core::models::search::{CrateSearchResult, SearchRequest};
pub use rustacean_docs_mcp_server::tools::{search::SearchTool, ToolHandler};
pub use serde_json::{json, Value};
pub use std::sync::Arc;
pub use std::time::Duration;
pub use tokio::sync::RwLock;
