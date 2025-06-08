//! Integration tests for the Rustacean Docs MCP Server
//!
//! This module contains integration tests that verify the complete
//! functionality of the MCP server, including:
//!
//! - Search flow from tool invocation to response
//! - Cache hit/miss scenarios and TTL behavior  
//! - MCP protocol compliance and schema validation
//! - Full workflow testing with cache integration

// Import all integration test modules
mod cache_integration;
mod full_workflow;
mod mcp_protocol;
mod search_flow;

// Re-export commonly used test utilities
pub use cache_integration::*;
pub use full_workflow::*;
pub use mcp_protocol::*;
pub use search_flow::*;
