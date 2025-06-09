use anyhow::Result;

use rust_mcp_sdk::{
    mcp_server::{hyper_server, HyperServerOptions},
    schema::{
        Implementation, InitializeResult, ServerCapabilities, ServerCapabilitiesTools,
        LATEST_PROTOCOL_VERSION,
    },
};

use rustacean_docs_mcp_server::{Config, RustaceanDocsHandler};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("rustacean_docs=debug,info")
        .init();

    // Load server configuration
    let config = Config::load()?;

    // Create server details for MCP initialization
    let server_details = InitializeResult {
        server_info: Implementation {
            name: config.server.name.clone(),
            version: config.server.version.clone(),
        },
        capabilities: ServerCapabilities {
            tools: Some(ServerCapabilitiesTools::default()),
            ..Default::default()
        },
        instructions: Some("MCP server for accessing Rust documentation from docs.rs".to_string()),
        meta: None,
        protocol_version: LATEST_PROTOCOL_VERSION.into(),
    };

    // // Create stdio transport
    // let transport = rust_mcp_sdk::StdioTransport::new(Default::default())
    //     .map_err(|e| anyhow::anyhow!("Failed to create transport: {}", e))?;

    // Create our handler
    let handler = RustaceanDocsHandler::new(config).await?;

    // // Create and start the MCP server
    // let server = server_runtime::create_server(server_details, transport, handler);
    let server = hyper_server::create_server(
        server_details,
        handler,
        HyperServerOptions {
            host: "0.0.0.0".to_string(),
            port: 8000,
            ping_interval: Duration::from_secs(5),
            ..Default::default()
        },
    );
    // // eprintln!("Starting Rustacean Docs MCP Server...");
    server
        .start()
        .await
        .map_err(|e| anyhow::anyhow!("Server error: {}", e))?;

    Ok(())
}
