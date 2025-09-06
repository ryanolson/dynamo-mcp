# Dynamo MCP Server

A high-performance Model Context Protocol (MCP) server for Dynamo documentation and operations.

## Purpose

This MCP server provides Claude and other AI assistants with access to Dynamo's comprehensive documentation, enabling them to:

- Search and retrieve Dynamo architecture documentation
- Access configuration guides and best practices
- Browse versioned documentation across releases
- Manage multiple repository versions efficiently

## Features

- **Fast indexing** - Indexes documentation in milliseconds using Rust's performance
- **Version management** - Switch between releases, branches, and commits seamlessly
- **Git worktree support** - Efficient version switching without full clones
- **GitHub integration** - Automatically fetches latest releases
- **Low memory footprint** - ~5MB memory usage vs ~30MB for Python version

## Available Tools

### search_docs
Search through Dynamo documentation with full-text search capabilities.

### list_versions
List available versions (branches, tags, releases) for repositories.

### switch_version
Switch to a different version of documentation dynamically.

### refresh_repos
Fetch latest updates from GitHub repositories.

### bootstrap_status
Check installation status of Dynamo tools and dependencies.

## Configuration

The server supports the following environment variables:
- `DYNAMO_USE_LOCAL` - Use local repository checkouts instead of GitHub releases
- `DYNAMO_VERSION` - Override default Dynamo repository version
- `DYNAMO_DOTFILES_VERSION` - Override default dotfiles repository version

## Architecture

The server uses Git worktrees for efficient version management:
- Bare repositories cached in `~/.cache/dynamo-mcp/bare/`
- Worktrees created in `~/.cache/dynamo-mcp/worktrees/`
- Automatic cleanup of old worktrees to manage disk space

## Performance

| Metric | Python | Rust |
|--------|--------|------|
| Startup time | ~500ms | ~50ms |
| Memory usage | ~30MB | ~5MB |
| Document indexing | ~100ms | ~10ms |
| Search latency | ~20ms | ~2ms |