# Rustacean Docs CLI Tool

A command-line interface for debugging and testing the Rustacean Docs MCP tools without the MCP protocol overhead.

## Installation

```bash
cargo build --bin rustacean-docs-cli
```

## Usage

### List Available Tools

```bash
# Pretty formatted output (default)
rustacean-docs-cli list

# JSON output
rustacean-docs-cli --format json list

# Raw output (just tool names)
rustacean-docs-cli --format raw list
```

### Run a Tool

```bash
# Search for crates
rustacean-docs-cli run search_crate '{"query": "tokio", "limit": 5}'

# Get crate documentation
rustacean-docs-cli run get_crate_docs '{"crate_name": "serde"}'

# Get specific item documentation
rustacean-docs-cli run get_item_docs '{"crate_name": "serde", "item_path": "Serialize"}'

# Get crate metadata
rustacean-docs-cli run get_crate_metadata '{"crate_name": "tokio"}'

# List recent releases
rustacean-docs-cli run list_recent_releases '{"limit": 10}'

# Get cache statistics
rustacean-docs-cli run get_cache_stats '{}'

# Clear cache
rustacean-docs-cli run clear_cache '{}'

# Run cache maintenance
rustacean-docs-cli run cache_maintenance '{}'
```

### Show Tool Schema

```bash
# Show parameter schema for a tool
rustacean-docs-cli schema search_crate

# JSON format
rustacean-docs-cli --format json schema get_crate_docs
```

## Output Formats

- `pretty` (default) - Human-readable formatted output
- `json` - Compact JSON output
- `raw` - Raw text output

## Logging

Control log verbosity with the `--log-level` flag:

```bash
rustacean-docs-cli --log-level debug run search_crate '{"query": "async"}'
```

Available levels: `trace`, `debug`, `info`, `warn`, `error`

## Examples

### Search for async crates with debug logging
```bash
rustacean-docs-cli --log-level debug run search_crate '{"query": "async", "limit": 3}'
```

### Get documentation for a specific struct
```bash
rustacean-docs-cli run get_item_docs '{"crate_name": "tokio", "item_path": "runtime/struct.Runtime.html"}'
```

### Export tool list as JSON
```bash
rustacean-docs-cli --format json list > tools.json
```

### Check cache performance
```bash
rustacean-docs-cli --format pretty run get_cache_stats '{}'
```

## Debugging Tips

1. Use `--log-level debug` to see detailed execution logs
2. Use `--format json` for programmatic processing
3. Run `get_cache_stats` to monitor cache behavior
4. Use `clear_cache` if you suspect stale data issues