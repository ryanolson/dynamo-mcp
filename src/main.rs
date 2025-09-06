mod github;
mod repo_manager;

use anyhow::Result;
use jsonrpc_core::{IoHandler, Params, Value};
use jsonrpc_stdio_server::ServerBuilder;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use tracing::info;
use walkdir::WalkDir;

use repo_manager::RepoManager;

// Include SERVER_INFO.md at compile time
const SERVER_INFO: &str = include_str!("../SERVER_INFO.md");

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Document {
    id: String,
    title: String,
    path: String,
    content: String,
    category: String,
    repo: String,
}

struct DocumentIndex {
    documents: HashMap<String, Document>,
}

impl DocumentIndex {
    fn new() -> Self {
        Self {
            documents: HashMap::new(),
        }
    }

    fn index_from_manager(&mut self, repo_manager: &RepoManager) -> Result<()> {
        // Index dotfiles if available
        if let Some(dotfiles_path) = repo_manager.get_path("dynamo-dotfiles") {
            self.index_dotfiles(&dotfiles_path)?;
            info!("Indexed dotfiles from {:?}", dotfiles_path);
        }
        
        // Index dynamo if available
        if let Some(dynamo_path) = repo_manager.get_path("dynamo") {
            self.index_dynamo(&dynamo_path)?;
            info!("Indexed dynamo from {:?}", dynamo_path);
        }
        
        Ok(())
    }
    
    fn index_dotfiles(&mut self, base_path: &Path) -> Result<()> {
        let readme_path = base_path.join("README.md");
        if readme_path.exists() {
            self.add_document(
                "dotfiles-readme",
                "Dynamo Dotfiles Overview",
                readme_path,
                "getting_started",
                "dynamo_dotfiles",
            )?;
        }
        Ok(())
    }
    
    fn index_dynamo(&mut self, base_path: &Path) -> Result<()> {
        let docs_path = base_path.join("docs");
        
        // Index architecture docs
        let arch_path = docs_path.join("architecture");
        if arch_path.exists() {
            for entry in WalkDir::new(&arch_path).max_depth(2) {
                let entry = entry?;
                if entry.path().extension().and_then(|s| s.to_str()) == Some("md") {
                    let doc_id = format!("arch-{}", entry.file_name().to_string_lossy().replace(".md", ""));
                    let title = entry.file_name().to_string_lossy().replace('_', " ");
                    self.add_document(
                        &doc_id,
                        &title,
                        entry.path().to_path_buf(),
                        "architecture",
                        "dynamo",
                    )?;
                }
            }
        }
        
        // Index guides
        let guides_path = docs_path.join("guides");
        if guides_path.exists() {
            for entry in WalkDir::new(&guides_path).max_depth(3) {
                let entry = entry?;
                if entry.path().extension().and_then(|s| s.to_str()) == Some("md") {
                    let doc_id = format!("guide-{}", entry.file_name().to_string_lossy().replace(".md", ""));
                    let title = entry.file_name().to_string_lossy().replace('_', " ");
                    self.add_document(
                        &doc_id,
                        &title,
                        entry.path().to_path_buf(),
                        "guide",
                        "dynamo",
                    )?;
                }
            }
        }
        
        Ok(())
    }
    
    fn add_document(
        &mut self,
        id: &str,
        title: &str,
        path: PathBuf,
        category: &str,
        repo: &str,
    ) -> Result<()> {
        let content = fs::read_to_string(&path)?;
        let doc = Document {
            id: id.to_string(),
            title: title.to_string(),
            path: path.to_string_lossy().to_string(),
            content,
            category: category.to_string(),
            repo: repo.to_string(),
        };
        self.documents.insert(id.to_string(), doc);
        Ok(())
    }
    
    fn search(&self, query: &str) -> Vec<Document> {
        let query_lower = query.to_lowercase();
        self.documents
            .values()
            .filter(|doc| {
                doc.title.to_lowercase().contains(&query_lower) ||
                doc.content.to_lowercase().contains(&query_lower)
            })
            .cloned()
            .collect()
    }
}

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("dynamo_mcp=info")
        .with_writer(std::io::stderr)
        .init();
    
    info!("Starting Dynamo MCP Server (Rust)");
    
    // Setup repository manager
    let mut repo_manager = RepoManager::new()?;
    
    // Check environment variables for configuration
    let use_local = env::var("DYNAMO_USE_LOCAL").is_ok();
    let dynamo_version = env::var("DYNAMO_VERSION").ok();
    let dotfiles_version = env::var("DYNAMO_DOTFILES_VERSION").ok();
    
    // Setup repositories
    info!("Setting up repositories...");
    
    // Setup dynamo repository
    repo_manager.setup_repo(
        "dynamo",
        "ai-dynamo",
        "dynamo",
        dynamo_version.as_deref(),
        use_local,
    )?;
    
    // Setup dynamo-dotfiles repository
    repo_manager.setup_repo(
        "dynamo-dotfiles",
        "ryanolson",
        "dynamo-dotfiles",
        dotfiles_version.as_deref(),
        use_local,
    )?;
    
    // Index documents
    let mut index = DocumentIndex::new();
    index.index_from_manager(&repo_manager)?;
    info!("Indexed {} documents", index.documents.len());
    
    // Create JSON-RPC handler
    let mut io = IoHandler::new();
    
    // Wrap index and repo_manager in Arc<Mutex> for thread-safe sharing
    let index_clone = Arc::new(Mutex::new(index));
    let repo_manager_clone = Arc::new(Mutex::new(repo_manager));
    
    // Handle initialize
    io.add_method("initialize", |_params: Params| async {
        Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "resources": {"listChanged": false},
                "tools": {},
            },
            "serverInfo": {
                "name": "dynamo-mcp-rust",
                "version": "0.1.0",
                "instructions": SERVER_INFO
            }
        }))
    });
    
    // Handle resources/list
    let index_for_resources = index_clone.clone();
    io.add_method("resources/list", move |_params: Params| {
        let index = index_for_resources.clone();
        async move {
            let index = index.lock().unwrap();
            let mut resources = Vec::new();
            for doc in index.documents.values() {
                resources.push(json!({
                    "uri": format!("dynamo://docs/{}", doc.id),
                    "name": doc.title,
                    "description": format!("{} documentation from {}", doc.category, doc.repo),
                    "mimeType": "text/markdown"
                }));
            }
            Ok(json!({ "resources": resources }))
        }
    });
    
    // Handle resources/read
    let index_for_read = index_clone.clone();
    io.add_method("resources/read", move |params: Params| {
        let index = index_for_read.clone();
        async move {
            let params: serde_json::Map<String, Value> = params.parse()?;
            let uri = params.get("uri")
                .and_then(|v| v.as_str())
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("uri required"))?;
            
            let index = index.lock().unwrap();
            if let Some(doc_id) = uri.strip_prefix("dynamo://docs/") {
                if let Some(doc) = index.documents.get(doc_id) {
                    return Ok(json!({
                        "contents": [{
                            "uri": uri,
                            "mimeType": "text/markdown",
                            "text": doc.content
                        }]
                    }));
                }
            }
            
            Err(jsonrpc_core::Error::invalid_params("Resource not found"))
        }
    });
    
    // Handle tools/list
    io.add_method("tools/list", |_params: Params| async {
        Ok(json!({
            "tools": [
                {
                    "name": "search_docs",
                    "description": "Search Dynamo documentation",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": {"type": "string", "description": "Search query"},
                            "limit": {"type": "integer", "description": "Max results"}
                        },
                        "required": ["query"]
                    }
                },
                {
                    "name": "list_versions",
                    "description": "List available versions for a repository",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "repo": {"type": "string", "description": "Repository name (dynamo or dynamo-dotfiles)"}
                        },
                        "required": ["repo"]
                    }
                },
                {
                    "name": "switch_version",
                    "description": "Switch repository to a different version",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "repo": {"type": "string", "description": "Repository name (dynamo or dynamo-dotfiles)"},
                            "version": {"type": "string", "description": "Version to switch to (branch/tag/commit)"}
                        },
                        "required": ["repo", "version"]
                    }
                },
                {
                    "name": "refresh_repos",
                    "description": "Fetch latest updates from GitHub",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "bootstrap_status",
                    "description": "Check installation status",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                }
            ]
        }))
    });
    
    // Handle tools/call
    let index_for_tools = index_clone.clone();
    let repo_manager_for_tools = repo_manager_clone.clone();
    io.add_method("tools/call", move |params: Params| {
        let index = index_for_tools.clone();
        let repo_manager = repo_manager_for_tools.clone();
        async move {
            let params: serde_json::Map<String, Value> = params.parse()?;
            let name = params.get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("name required"))?;
            let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            
            match name {
                "search_docs" => {
                    let query = arguments.get("query")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let limit = arguments.get("limit")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(10) as usize;
                    
                    let index = index.lock().unwrap();
                    let results = index.search(query);
                    let results: Vec<_> = results.into_iter()
                        .take(limit)
                        .enumerate()
                        .map(|(i, doc)| json!({
                            "rank": i + 1,
                            "id": doc.id,
                            "title": doc.title,
                            "category": doc.category,
                            "repo": doc.repo,
                            "preview": doc.content.lines().take(2).collect::<Vec<_>>().join("\n")
                        }))
                        .collect();
                    
                    Ok(json!({
                        "content": [{
                            "type": "text",
                            "text": serde_json::to_string_pretty(&results).unwrap()
                        }]
                    }))
                },
                "list_versions" => {
                    let repo_name = arguments.get("repo")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| jsonrpc_core::Error::invalid_params("repo required"))?;
                    
                    let repo_manager = repo_manager.lock().unwrap();
                    match repo_manager.list_versions(repo_name) {
                        Ok(version_info) => {
                            Ok(json!({
                                "content": [{
                                    "type": "text",
                                    "text": serde_json::to_string_pretty(&version_info).unwrap()
                                }]
                            }))
                        },
                        Err(e) => {
                            Ok(json!({
                                "content": [{
                                    "type": "text",
                                    "text": format!("Error listing versions: {}", e)
                                }],
                                "isError": true
                            }))
                        }
                    }
                },
                "switch_version" => {
                    let repo_name = arguments.get("repo")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| jsonrpc_core::Error::invalid_params("repo required"))?;
                    let version = arguments.get("version")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| jsonrpc_core::Error::invalid_params("version required"))?;
                    
                    let mut repo_manager = repo_manager.lock().unwrap();
                    match repo_manager.switch_version(repo_name, version) {
                        Ok(path) => {
                            // Re-index after switching version
                            let mut index = index.lock().unwrap();
                            index.documents.clear();
                            let _ = index.index_from_manager(&*repo_manager);
                            
                            Ok(json!({
                                "content": [{
                                    "type": "text",
                                    "text": format!("Switched {} to version {} at {:?}\nRe-indexed {} documents", 
                                        repo_name, version, path, index.documents.len())
                                }]
                            }))
                        },
                        Err(e) => {
                            Ok(json!({
                                "content": [{
                                    "type": "text",
                                    "text": format!("Error switching version: {}", e)
                                }],
                                "isError": true
                            }))
                        }
                    }
                },
                "refresh_repos" => {
                    let mut repo_manager = repo_manager.lock().unwrap();
                    match repo_manager.refresh() {
                        Ok(_) => {
                            Ok(json!({
                                "content": [{
                                    "type": "text",
                                    "text": "Successfully refreshed repositories"
                                }]
                            }))
                        },
                        Err(e) => {
                            Ok(json!({
                                "content": [{
                                    "type": "text",
                                    "text": format!("Error refreshing: {}", e)
                                }],
                                "isError": true
                            }))
                        }
                    }
                },
                "bootstrap_status" => {
                    let tools = vec!["chezmoi", "mise", "fish", "hx", "zellij", "starship", "rg", "eza"];
                    let mut status = HashMap::new();
                    
                    for tool in tools {
                        let exists = Command::new("which")
                            .arg(tool)
                            .output()
                            .map(|o| o.status.success())
                            .unwrap_or(false);
                        status.insert(tool, exists);
                    }
                    
                    Ok(json!({
                        "content": [{
                            "type": "text",
                            "text": serde_json::to_string_pretty(&status).unwrap()
                        }]
                    }))
                },
                _ => Err(jsonrpc_core::Error::method_not_found())
            }
        }
    });
    
    // Run the server
    let server = ServerBuilder::new(io);
    let _server_handle = server.build();
    
    info!("MCP server running on stdio");
    // Keep the server running
    std::thread::park();
    
    Ok(())
}
