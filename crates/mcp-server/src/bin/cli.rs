use anyhow::Result;
use clap::{Parser, Subcommand};
use rustacean_docs_mcp_server::{Config, RustaceanDocsHandler};
use serde_json::Value;
use tracing_subscriber;

#[derive(Parser)]
#[command(
    name = "rustacean-docs-cli",
    about = "CLI tool for debugging Rustacean Docs MCP tools",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, help = "Output format", default_value = "pretty")]
    format: OutputFormat,

    #[arg(short, long, help = "Set log level", default_value = "warn")]
    log_level: String,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "List all available tools")]
    List,

    #[command(about = "Run a specific tool")]
    Run {
        #[arg(help = "Tool name (e.g., search_crate)")]
        tool: String,

        #[arg(help = "Tool parameters as JSON string")]
        params: Option<String>,
    },

    #[command(about = "Show parameter schema for a tool")]
    Schema {
        #[arg(help = "Tool name")]
        tool: String,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum OutputFormat {
    Json,
    Pretty,
    Raw,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = match cli.log_level.as_str() {
        "trace" => tracing::Level::TRACE,
        "debug" => tracing::Level::DEBUG,
        "info" => tracing::Level::INFO,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => tracing::Level::WARN,
    };

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_target(false)
        .init();

    // Load configuration and create handler
    let config = Config::load()?;
    let handler = RustaceanDocsHandler::new(config).await?;

    match cli.command {
        Commands::List => list_tools(&handler, cli.format),
        Commands::Run { tool, params } => run_tool(&handler, &tool, params, cli.format).await,
        Commands::Schema { tool } => show_schema(&handler, &tool, cli.format),
    }
}

fn list_tools(handler: &RustaceanDocsHandler, format: OutputFormat) -> Result<()> {
    let tools = handler.get_available_tools();
    
    match format {
        OutputFormat::Json => {
            let json = serde_json::json!({
                "tools": tools.iter().map(|tool| {
                    serde_json::json!({
                        "name": tool.name,
                        "description": tool.description,
                    })
                }).collect::<Vec<_>>()
            });
            println!("{}", serde_json::to_string(&json)?);
        }
        OutputFormat::Pretty => {
            println!("Available tools:\n");
            for tool in tools {
                println!("  {} - {}", tool.name, tool.description);
            }
        }
        OutputFormat::Raw => {
            for tool in tools {
                println!("{}", tool.name);
            }
        }
    }

    Ok(())
}

async fn run_tool(
    handler: &RustaceanDocsHandler,
    tool_name: &str,
    params: Option<String>,
    format: OutputFormat,
) -> Result<()> {
    // Parse parameters
    let params_json: Value = match params {
        Some(p) => serde_json::from_str(&p)?,
        None => serde_json::json!({}),
    };

    // Execute the tool
    let result = handler.execute_tool_directly(tool_name, params_json).await?;

    // Format output
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string(&result)?);
        }
        OutputFormat::Pretty => {
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        OutputFormat::Raw => {
            if let Some(text) = result.as_str() {
                println!("{}", text);
            } else {
                println!("{}", result);
            }
        }
    }

    Ok(())
}

fn show_schema(handler: &RustaceanDocsHandler, tool_name: &str, format: OutputFormat) -> Result<()> {
    let schema = handler.get_tool_schema(tool_name)?;

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string(&schema)?);
        }
        OutputFormat::Pretty => {
            println!("Schema for tool '{}':\n", tool_name);
            println!("{}", serde_json::to_string_pretty(&schema)?);
        }
        OutputFormat::Raw => {
            println!("{}", schema);
        }
    }

    Ok(())
}