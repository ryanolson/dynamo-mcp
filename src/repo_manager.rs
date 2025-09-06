use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{info, warn};

use crate::github::GitHubClient;

#[derive(Debug, Clone, serde::Serialize)]
pub struct VersionInfo {
    pub repo: String,
    pub current_version: String,
    pub current_branch: Option<String>,
    pub current_commit: Option<String>,
    pub branches: Vec<String>,
    pub tags: Vec<String>,
    pub releases: Vec<String>,
}

pub struct RepoManager {
    cache_base: PathBuf,
    bare_repos: PathBuf,
    worktrees: PathBuf,
    repos: HashMap<String, RepoInfo>,
}

#[derive(Debug, Clone)]
struct RepoInfo {
    owner: String,
    name: String,
    current_version: String,
    worktree_path: PathBuf,
}

impl RepoManager {
    pub fn new() -> Result<Self> {
        let cache_base = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("No home directory"))?
            .join(".cache")
            .join("dynamo-mcp");
        
        let bare_repos = cache_base.join("bare");
        let worktrees = cache_base.join("worktrees");
        
        // Create directories if they don't exist
        std::fs::create_dir_all(&bare_repos)?;
        std::fs::create_dir_all(&worktrees)?;
        
        Ok(Self {
            cache_base,
            bare_repos,
            worktrees,
            repos: HashMap::new(),
        })
    }
    
    /// Setup a repository with optional version override
    pub fn setup_repo(
        &mut self,
        name: &str,
        owner: &str,
        repo: &str,
        version: Option<&str>,
        use_local: bool,
    ) -> Result<PathBuf> {
        // Check for local override
        if use_local {
            let local_path = dirs::home_dir()
                .ok_or_else(|| anyhow::anyhow!("No home directory"))?
                .join("repo")
                .join(repo);
            
            if local_path.exists() {
                info!("Using local repository at {:?}", local_path);
                self.repos.insert(name.to_string(), RepoInfo {
                    owner: owner.to_string(),
                    name: repo.to_string(),
                    current_version: "local".to_string(),
                    worktree_path: local_path.clone(),
                });
                return Ok(local_path);
            }
        }
        
        // Setup bare repository if needed
        let bare_path = self.bare_repos.join(format!("{}.git", repo));
        if !bare_path.exists() {
            self.clone_bare_repo(owner, repo, &bare_path)?;
        }
        
        // Fetch latest changes
        self.fetch_updates(&bare_path)?;
        
        // Determine version to use
        let version = if let Some(v) = version {
            v.to_string()
        } else {
            // Default to latest release, fallback to main
            let github = GitHubClient::new(owner, repo);
            github.get_latest_release()?
                .unwrap_or_else(|| {
                    github.get_default_branch()
                        .unwrap_or_else(|_| "main".to_string())
                })
        };
        
        // Create or reuse worktree
        let worktree_path = self.create_worktree(repo, &bare_path, &version)?;
        
        // Store repo info
        self.repos.insert(name.to_string(), RepoInfo {
            owner: owner.to_string(),
            name: repo.to_string(),
            current_version: version.clone(),
            worktree_path: worktree_path.clone(),
        });
        
        info!("Setup {} at version {} in {:?}", name, version, worktree_path);
        Ok(worktree_path)
    }
    
    /// Switch a repository to a different version
    pub fn switch_version(&mut self, name: &str, version: &str) -> Result<PathBuf> {
        let repo_info = self.repos.get(name)
            .ok_or_else(|| anyhow::anyhow!("Repository {} not setup", name))?
            .clone();
        
        let bare_path = self.bare_repos.join(format!("{}.git", repo_info.name));
        
        // Create new worktree for this version
        let worktree_path = self.create_worktree(&repo_info.name, &bare_path, version)?;
        
        // Update repo info
        self.repos.insert(name.to_string(), RepoInfo {
            owner: repo_info.owner,
            name: repo_info.name,
            current_version: version.to_string(),
            worktree_path: worktree_path.clone(),
        });
        
        info!("Switched {} to version {}", name, version);
        Ok(worktree_path)
    }
    
    /// Refresh repositories by fetching latest changes
    pub fn refresh(&mut self) -> Result<()> {
        for repo_info in self.repos.values() {
            let bare_path = self.bare_repos.join(format!("{}.git", repo_info.name));
            if bare_path.exists() {
                self.fetch_updates(&bare_path)?;
                info!("Refreshed {}", repo_info.name);
            }
        }
        Ok(())
    }
    
    /// List available versions for a repository
    pub fn list_versions(&self, name: &str) -> Result<VersionInfo> {
        let repo_info = self.repos.get(name)
            .ok_or_else(|| anyhow::anyhow!("Repository {} not setup", name))?;
        
        let bare_path = self.bare_repos.join(format!("{}.git", repo_info.name));
        
        // Get branches
        let branches = self.get_branches(&bare_path)?;
        
        // Get tags
        let tags = self.get_tags(&bare_path)?;
        
        // Get GitHub releases
        let github = GitHubClient::new(&repo_info.owner, &repo_info.name);
        let releases = github.list_releases().unwrap_or_default();
        
        // Get current commit
        let current_commit = self.get_current_commit(&repo_info.worktree_path)?;
        let current_branch = self.get_current_branch(&repo_info.worktree_path)?;
        
        Ok(VersionInfo {
            repo: name.to_string(),
            current_version: repo_info.current_version.clone(),
            current_branch,
            current_commit,
            branches,
            tags,
            releases,
        })
    }
    
    /// Get the current worktree path for a repository
    pub fn get_path(&self, name: &str) -> Option<PathBuf> {
        self.repos.get(name).map(|info| info.worktree_path.clone())
    }
    
    // Private helper methods
    
    fn clone_bare_repo(&self, owner: &str, repo: &str, bare_path: &Path) -> Result<()> {
        let url = format!("https://github.com/{}/{}.git", owner, repo);
        
        info!("Cloning bare repository from {}", url);
        
        let output = Command::new("git")
            .args(&["clone", "--bare", &url, bare_path.to_str().unwrap()])
            .output()
            .context("Failed to execute git clone")?;
        
        if !output.status.success() {
            anyhow::bail!("Failed to clone repository: {}", 
                String::from_utf8_lossy(&output.stderr));
        }
        
        Ok(())
    }
    
    fn fetch_updates(&self, bare_path: &Path) -> Result<()> {
        let output = Command::new("git")
            .args(&["fetch", "origin", "--tags"])
            .current_dir(bare_path)
            .output()
            .context("Failed to execute git fetch")?;
        
        if !output.status.success() {
            warn!("Failed to fetch updates: {}", 
                String::from_utf8_lossy(&output.stderr));
        }
        
        Ok(())
    }
    
    fn create_worktree(&self, repo: &str, bare_path: &Path, version: &str) -> Result<PathBuf> {
        // Sanitize version name for filesystem
        let safe_version = version.replace('/', "_").replace('\\', "_");
        let worktree_path = self.worktrees.join(format!("{}_{}", repo, safe_version));
        
        // Check if worktree already exists
        if worktree_path.exists() {
            // Checkout the correct version in existing worktree
            let output = Command::new("git")
                .args(&["checkout", version])
                .current_dir(&worktree_path)
                .output()?;
            
            if output.status.success() {
                return Ok(worktree_path);
            }
            
            // If checkout failed, remove and recreate
            warn!("Failed to checkout in existing worktree, recreating");
            std::fs::remove_dir_all(&worktree_path)?;
        }
        
        // Create new worktree
        info!("Creating worktree for {} at {}", repo, version);
        
        let output = Command::new("git")
            .args(&["worktree", "add", worktree_path.to_str().unwrap(), version])
            .current_dir(bare_path)
            .output()
            .context("Failed to execute git worktree add")?;
        
        if !output.status.success() {
            anyhow::bail!("Failed to create worktree: {}", 
                String::from_utf8_lossy(&output.stderr));
        }
        
        Ok(worktree_path)
    }
    
    fn get_branches(&self, bare_path: &Path) -> Result<Vec<String>> {
        let output = Command::new("git")
            .args(&["branch", "-r"])
            .current_dir(bare_path)
            .output()?;
        
        if output.status.success() {
            let branches = String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter_map(|line| {
                    let branch = line.trim();
                    if branch.contains("HEAD") {
                        None
                    } else {
                        branch.strip_prefix("origin/")
                            .map(|b| b.to_string())
                    }
                })
                .collect();
            Ok(branches)
        } else {
            Ok(Vec::new())
        }
    }
    
    fn get_tags(&self, bare_path: &Path) -> Result<Vec<String>> {
        let output = Command::new("git")
            .args(&["tag"])
            .current_dir(bare_path)
            .output()?;
        
        if output.status.success() {
            let tags = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|line| line.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect();
            Ok(tags)
        } else {
            Ok(Vec::new())
        }
    }
    
    fn get_current_commit(&self, worktree_path: &Path) -> Result<Option<String>> {
        let output = Command::new("git")
            .args(&["rev-parse", "HEAD"])
            .current_dir(worktree_path)
            .output()?;
        
        if output.status.success() {
            Ok(Some(String::from_utf8_lossy(&output.stdout).trim().to_string()))
        } else {
            Ok(None)
        }
    }
    
    fn get_current_branch(&self, worktree_path: &Path) -> Result<Option<String>> {
        let output = Command::new("git")
            .args(&["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(worktree_path)
            .output()?;
        
        if output.status.success() {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if branch == "HEAD" {
                Ok(None) // Detached HEAD state
            } else {
                Ok(Some(branch))
            }
        } else {
            Ok(None)
        }
    }
    
    /// Clean up old worktrees, keeping the most recent N
    pub fn cleanup_old_worktrees(&mut self, keep_recent: usize) -> Result<()> {
        // TODO: Implement cleanup logic
        // List worktrees, sort by access time, remove old ones
        Ok(())
    }
}