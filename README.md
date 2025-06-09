# Rustacean Docs

> **MCP Server for Rust Documentation Access** - Provide AI assistants with real-time access to Rust crate documentation, metadata, and ecosystem information.

[![Build Status](https://img.shields.io/github/actions/workflow/status/your-username/rustacean-docs/ci.yml?branch=main)](https://github.com/your-username/rustacean-docs/actions)
[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)](https://github.com/your-username/rustacean-docs)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-green.svg)](https://github.com/your-username/rustacean-docs#license)
[![Rust](https://img.shields.io/badge/rust-nightly-orange.svg)](https://rustup.rs/)
[![Docker](https://img.shields.io/badge/docker-ready-blue.svg)](https://hub.docker.com/)

A high-performance **Model Context Protocol (MCP) server** that enables AI assistants like Claude to seamlessly access and search the Rust ecosystem. Get real-time crate documentation, dependency analysis, and ecosystem insights directly in your AI conversations.

## ✨ Features

- 🔍 **Smart Crate Search** - Find crates by name, keywords, or functionality
- 📚 **Comprehensive Documentation** - Access complete crate docs with examples
- 🎯 **Precise Item Lookup** - Get documentation for specific functions, structs, traits
- 📊 **Rich Metadata** - Dependencies, licenses, download stats, and version info
- 🚀 **Recent Releases** - Track the latest updates across the Rust ecosystem
- ⚡ **Intelligent Caching** - Multi-tiered performance optimization (memory + disk)
- 🛠️ **Debug CLI** - Test tools without MCP protocol overhead
- 🐳 **Docker Ready** - Easy deployment with docker-compose
- 🔧 **Robust Error Handling** - Graceful handling of network issues and parsing errors

## 🚀 Quick Start

### Option 1: Docker (Recommended)

```bash
# Start the MCP server
docker-compose up -d

# Server runs on http://localhost:8000
# Ready for MCP client connections
```

### Option 2: Debug CLI Tool

```bash
# Build the CLI tool
cargo build --bin rustacean-docs-cli

# Search for crates
./target/debug/rustacean-docs-cli run search_crate '{"query": "async", "limit": 3}'

# Get crate documentation
./target/debug/rustacean-docs-cli run get_crate_docs '{"crate_name": "tokio"}'

# List all available tools
./target/debug/rustacean-docs-cli list
```

### Option 3: MCP Integration with Claude

Add to your Claude Code MCP configuration:

```json
{
  "mcpServers": {
    "rustacean-docs": {
      "command": "docker",
      "args": ["run", "--rm", "-p", "8000:8000", "rustacean-docs"],
      "env": {}
    }
  }
}
```

## 🛠️ What This Solves

**The Problem**: AI assistants need structured, real-time access to Rust documentation to provide accurate coding assistance, but parsing docs.rs manually is inefficient and error-prone.

**The Solution**: A dedicated MCP server that acts as an intelligent bridge between AI assistants and the Rust ecosystem, providing:

- **Instant crate discovery** for finding the right libraries
- **Complete documentation access** without browser context switching  
- **Dependency analysis** for understanding project requirements
- **Version compatibility** information for informed decision-making
- **Performance insights** through download statistics and community adoption

**Use Cases**:
- 💬 **AI Pair Programming** - Get crate suggestions and usage examples
- 🎓 **Learning Rust** - Explore the ecosystem with guided documentation
- 🔍 **Project Discovery** - Find libraries that solve specific problems
- 📝 **Code Review** - Understand dependencies and their capabilities
- 🏗️ **Architecture Planning** - Evaluate crates for system design

## 📚 Available Tools

### Core Documentation Tools

#### `search_crate`
Find Rust crates by name or descriptive keywords.

```bash
# Search by keywords
rustacean-docs-cli run search_crate '{"query": "web framework", "limit": 5}'

# Find specific crate
rustacean-docs-cli run search_crate '{"query": "axum"}'
```

**Parameters:**
- `query` (string, required): Search terms or crate name
- `limit` (integer, optional): Max results (default: 10, max: 100)

#### `get_crate_docs`
Fetch comprehensive documentation for a specific crate.

```bash
# Get latest version docs
rustacean-docs-cli run get_crate_docs '{"crate_name": "serde"}'

# Get specific version
rustacean-docs-cli run get_crate_docs '{"crate_name": "tokio", "version": "1.0.0"}'
```

**Parameters:**
- `crate_name` (string, required): Exact crate name
- `version` (string, optional): Specific version (defaults to latest)

#### `get_item_docs`
Get detailed documentation for specific items (functions, structs, traits, enums, modules).

```bash
# Find by item name
rustacean-docs-cli run get_item_docs '{"crate_name": "serde", "item_path": "Serialize"}'

# Use full path
rustacean-docs-cli run get_item_docs '{"crate_name": "tokio", "item_path": "runtime/struct.Runtime.html"}'
```

**Parameters:**
- `crate_name` (string, required): Crate containing the item
- `item_path` (string, required): Item name or full path
- `version` (string, optional): Crate version

### Metadata & Analysis Tools

#### `get_crate_metadata`
Comprehensive crate information including dependencies, licensing, and ecosystem data.

```bash
rustacean-docs-cli run get_crate_metadata '{"crate_name": "reqwest"}'
```

#### `list_recent_releases`
Track recently updated crates to stay current with ecosystem changes.

```bash
rustacean-docs-cli run list_recent_releases '{"limit": 20}'
```

### Cache Management Tools

#### `get_cache_stats`
Monitor cache performance and hit rates.

```bash
rustacean-docs-cli run get_cache_stats '{}'
```

#### `clear_cache`
Clear all cached data for fresh retrieval.

```bash
rustacean-docs-cli run clear_cache '{}'
```

#### `cache_maintenance`
Optimize cache performance and cleanup expired entries.

```bash
rustacean-docs-cli run cache_maintenance '{}'
```

## 🏗️ Architecture

This project uses a **multi-crate workspace** architecture for modularity and maintainability:

```
rustacean-docs/
├── crates/
│   ├── core/           # Shared models, errors, utilities
│   ├── docs-client/    # HTTP client for docs.rs and crates.io APIs
│   ├── cache/          # Multi-tiered caching (memory + disk)
│   ├── mcp-server/     # MCP protocol implementation + tools
│   └── integration-tests/ # End-to-end testing
├── Dockerfile          # Container deployment
└── docker-compose.yml  # Easy orchestration
```

### Key Components

- **`rustacean_docs_core`**: Common data models, error types, and utilities
- **`rustacean_docs_client`**: HTTP client with retry logic, rate limiting, and HTML parsing
- **`rustacean_docs_cache`**: LRU memory cache + persistent disk cache for performance
- **`rustacean_docs_mcp_server`**: MCP protocol server with 8 specialized tools

### Caching Strategy

- **Memory Cache**: Fast access for frequently requested data
- **Disk Cache**: Persistent storage for larger datasets
- **Smart Invalidation**: Automatic cleanup and maintenance
- **Performance Monitoring**: Built-in metrics and statistics

## 🔧 Development

### Prerequisites

This project uses **devenv** for reproducible development environments:

```bash
# Install devenv (requires Nix)
curl -fsSL https://get.devenv.sh | sh

# Enter development environment
devenv shell

# Available in the dev environment:
# - Rust nightly toolchain
# - Node.js with npm
# - Git with pre-commit hooks
```

### Alternative Setup

```bash
# Standard Rust development
cargo build
cargo test
cargo clippy
cargo fmt

# Run integration tests
cargo test -p integration-tests

# Build CLI tool
cargo build --bin rustacean-docs-cli
```

### Testing

```bash
# Run all tests with coverage
cargo test

# Run specific crate tests
cargo test -p rustacean-docs-core
cargo test -p rustacean-docs-client

# Integration tests (requires network)
cargo test -p integration-tests

# Test CLI tool
cargo build --bin rustacean-docs-cli
./target/debug/rustacean-docs-cli run search_crate '{"query": "test", "limit": 1}'
```

### Debugging

Use the CLI tool for development and debugging:

```bash
# Enable debug logging
rustacean-docs-cli --log-level debug run get_crate_docs '{"crate_name": "tokio"}'

# Check cache performance
rustacean-docs-cli run get_cache_stats '{}'

# Export results for analysis
rustacean-docs-cli --format json run search_crate '{"query": "web"}' > results.json

# Get tool parameter schemas
rustacean-docs-cli schema get_crate_docs
```

## 🐛 Troubleshooting

### Common Issues

**HTML Parsing Failures**
- **Symptoms**: Missing summaries, duplicate examples, malformed item names
- **Cause**: Outdated CSS selectors not matching current docs.rs structure
- **Fix**: Update selectors in `crates/docs-client/src/html_parser.rs`

**Cache Performance Issues**
- **Symptoms**: Slow response times, high memory usage
- **Diagnosis**: `rustacean-docs-cli run get_cache_stats '{}'`
- **Fix**: `rustacean-docs-cli run cache_maintenance '{}'`

**Network Timeouts**
- **Symptoms**: Requests failing for popular crates
- **Cause**: Rate limiting or network issues
- **Fix**: Check retry configuration in `crates/docs-client/src/retry.rs`

**Docker Connectivity**
- **Symptoms**: MCP client cannot connect to server
- **Fix**: Ensure port 8000 is accessible and not blocked by firewall

### Performance Monitoring

```bash
# Monitor cache hit rates
rustacean-docs-cli run get_cache_stats '{}'

# Clear cache if performance degrades
rustacean-docs-cli run clear_cache '{}'

# Run maintenance for optimal performance
rustacean-docs-cli run cache_maintenance '{}'
```

## 🚦 Current Status & Roadmap

### ✅ Completed
- Core MCP server implementation
- All 8 essential tools
- Multi-tiered caching system
- Docker deployment
- Debug CLI tool
- Integration test suite

### 🔄 In Progress
- HTML parsing improvements
- Enhanced error handling
- Performance optimizations
- Documentation completeness

### 📋 Planned
- WebSocket transport support
- Advanced search filters
- Dependency graph visualization
- Bulk operations
- Metrics dashboard
- Rate limiting improvements

### Known Limitations
- Some docs.rs HTML parsing edge cases
- Limited to publicly available crates
- Network-dependent (no offline mode)
- Memory usage scales with cache size

## 🤝 Contributing

1. **Fork the repository**
2. **Create a feature branch**: `git checkout -b feature/amazing-feature`
3. **Follow the development setup** above
4. **Run tests**: `cargo test && cargo clippy`
5. **Commit changes**: Use conventional commits (e.g., `feat: add new tool`)
6. **Submit a Pull Request**

### Development Principles
- Follow idiomatic Rust practices
- Keep functions small and single-purpose
- Add tests for new functionality
- Update documentation for API changes
- Use the CLI tool for testing

## 📄 License

This project is licensed under either of:

- **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- **MIT License** ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## 🙏 Acknowledgments

- **docs.rs** team for providing comprehensive Rust documentation
- **crates.io** for the package registry and API
- **MCP Protocol** for enabling AI assistant integrations
- **Rust Community** for building an amazing ecosystem

---

**Questions?** Open an issue or start a discussion. We're here to help!

**Star ⭐ this repo** if you find it useful for your Rust development workflow.