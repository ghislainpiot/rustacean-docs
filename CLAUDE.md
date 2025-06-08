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