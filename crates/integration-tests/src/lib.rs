//! Integration tests library for Rustacean Docs MCP Server
//!
//! This crate contains shared utilities and helpers for integration testing.

#![allow(clippy::uninlined_format_args)]
#![allow(clippy::useless_vec)]

pub mod common;

// Re-export commonly used types for tests
pub use rustacean_docs_cache::{Cache, MemoryCache};
pub use rustacean_docs_client::DocsClient;
pub use rustacean_docs_core::models::search::{CrateSearchResult, SearchRequest};
pub use rustacean_docs_mcp_server::tools::{search::SearchTool, ToolHandler};
pub use serde_json::{json, Value};
pub use std::sync::Arc;
pub use std::time::Duration;
pub use tokio::sync::RwLock;
