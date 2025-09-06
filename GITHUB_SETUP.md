# GitHub Repository Setup

To push this repository to GitHub, follow these steps:

## 1. Create a new repository on GitHub

Visit https://github.com/new and create a repository with:
- Repository name: `dynamo-mcp-rust` (or your preferred name)
- Description: "High-performance Rust MCP server for Dynamo documentation"
- Public/Private: Your choice
- **DO NOT** initialize with README, .gitignore, or license (we already have these)

## 2. Add the remote origin

After creating the repository, run:

```bash
# Replace YOUR_USERNAME with your GitHub username
git remote add origin https://github.com/YOUR_USERNAME/dynamo-mcp-rust.git

# Or if using SSH:
git remote add origin git@github.com:YOUR_USERNAME/dynamo-mcp-rust.git
```

## 3. Push to GitHub

```bash
# Push the main branch
git push -u origin master

# Or if you prefer 'main' as the default branch:
git branch -M main
git push -u origin main
```

## 4. Verify the push

Your repository should now be live at:
`https://github.com/YOUR_USERNAME/dynamo-mcp-rust`

## 5. (Optional) Add GitHub Actions

Consider adding CI/CD workflows for:
- Rust formatting (`cargo fmt --check`)
- Linting (`cargo clippy`)
- Tests (`cargo test`)
- Release builds

## 6. (Optional) Configure for MCP

If you want others to use your MCP server, add installation instructions to the README:

```json
{
  "mcpServers": {
    "dynamo": {
      "command": "path/to/dynamo_mcp",
      "args": [],
      "env": {
        "DYNAMO_VERSION": "v1.0.0",
        "DYNAMO_USE_LOCAL": "true"
      }
    }
  }
}
```

## Repository Structure

```
dynamo-mcp-rust/
├── Cargo.toml          # Rust dependencies
├── Cargo.lock          # Locked dependencies
├── LICENSE             # MIT License
├── README.md           # Project documentation
├── SERVER_INFO.md      # MCP server information
├── .gitignore          # Git ignore rules
└── src/
    ├── main.rs         # Main server implementation
    ├── github.rs       # GitHub API client
    └── repo_manager.rs # Git worktree management
```