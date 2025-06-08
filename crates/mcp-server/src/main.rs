use anyhow::Result;

use rustacean_docs_mcp_server::{Config, McpServer};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("rustacean_docs=debug,info")
        .init();

    // Load server configuration
    let config = Config::load()?;

    // Create and initialize the MCP server
    let mut server = McpServer::new(config)?;
    server.initialize().await?;

    // Display server information
    let info = server.get_server_info();
    println!("Server Info: {}", serde_json::to_string_pretty(&info)?);

    // For now, just demonstrate that the server can be created and initialized
    println!("MCP server initialized successfully!");

    // In a real implementation, this would start the MCP protocol listener
    // For now, we'll just shutdown cleanly
    server.shutdown().await?;

    Ok(())
}
