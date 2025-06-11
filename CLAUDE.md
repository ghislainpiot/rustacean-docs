# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Environment

This project uses **devenv** for development environment management with Nix. The development environment includes:

- Rust nightly toolchain via Fenix
- JavaScript/Node.js with npm
- Git

### Environment Setup

```bash
# Enter the development environment (if direnv is not auto-loading)
devenv shell

# Run tests
devenv test

# Available scripts
hello          # Prints greeting message
claude         # Launches Claude Code CLI
```

### Development Commands

The project is configured with Rust nightly channel. Standard Rust development commands should work:

```bash
cargo build
cargo test
cargo run
```

For JavaScript/npm projects (if any are added):
```bash
npm install
npm test
npm run build
```

## Project Structure

This is a new Rust project focused on documentation (based on the "rustacean-docs" name). The devenv configuration suggests it may involve both Rust and JavaScript components.

## Environment Notes

- Uses direnv for automatic environment loading (.envrc)
- Configured with 20s timeout warning for direnv
- Allows unfree packages in Nix configuration
- Git pre-commit hooks are available but not currently enabled

## Development Principles

- Follow idiomatic Rust practices
- This project uses conventional commits. Make sure to commit changes once a task is done. Do not credit yourself in the commit message.
- Keep functions small and single purposed
- Try to avoid modifying Cargo.toml by hand if possible, use the CLI (especially for adding new dependencies)

## Code Commit Guidelines

- Before committing code, make sure to:
  - Run all tests with coverage
  - Ensure coverage of your feature is good enough
  - Run formatting and clippy to clean the code

## Memory Principles

- Keep references to tasks or Claude Code out of the code and commit messages
- When fixing a bug, try to make a minimal test reproducing it to avoid future regressions
- When changing tests, be very intentional about it. The goal is to have a working product, not passing tests that are incorrect.

## Debugging MCP Tools

The project includes a CLI tool (`rustacean-docs-cli`) for debugging MCP tools without the protocol overhead:

### Building the CLI
```bash
cargo build --bin rustacean-docs-cli
```

### Common Debugging Commands

```bash
# List all available tools
rustacean-docs-cli list

# Test search functionality
rustacean-docs-cli run search_crate '{"query": "serde", "limit": 3}'

# Check cache performance
rustacean-docs-cli run get_cache_stats '{}'

# Clear cache if needed
rustacean-docs-cli run clear_cache '{}'

# Get tool parameter schema
rustacean-docs-cli schema search_crate

# Debug with verbose logging
rustacean-docs-cli --log-level debug run get_crate_docs '{"crate_name": "tokio"}'

# Export results as JSON for analysis
rustacean-docs-cli --format json run list_recent_releases '{"limit": 10}' > releases.json
```

### Debugging Tips

1. Use `--log-level debug` or `--log-level trace` to see detailed execution logs
2. Use `--format json` to get structured output for programmatic analysis
3. Run `get_cache_stats` regularly to monitor cache behavior and performance
4. Use `clear_cache` if you suspect stale data issues
5. Test with various parameter combinations to ensure robust error handling

### Output Formats

- `pretty` (default) - Human-readable formatted output
- `json` - Compact JSON for programmatic processing
- `raw` - Plain text output

See `crates/mcp-server/CLI_USAGE.md` for comprehensive documentation.