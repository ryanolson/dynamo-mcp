use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub published_at: String,
    pub prerelease: bool,
    pub draft: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubRepo {
    pub default_branch: String,
}

pub struct GitHubClient {
    client: reqwest::blocking::Client,
    owner: String,
    repo: String,
}

impl GitHubClient {
    pub fn new(owner: impl Into<String>, repo: impl Into<String>) -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
            owner: owner.into(),
            repo: repo.into(),
        }
    }

    /// Get the latest release tag from GitHub
    pub fn get_latest_release(&self) -> Result<Option<String>> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            self.owner, self.repo
        );

        let response = self.client
            .get(&url)
            .header("User-Agent", "dynamo-mcp")
            .send();

        match response {
            Ok(resp) if resp.status().is_success() => {
                let release: GitHubRelease = resp.json()?;
                info!("Found latest release: {}", release.tag_name);
                Ok(Some(release.tag_name))
            }
            Ok(resp) if resp.status() == 404 => {
                info!("No releases found for {}/{}", self.owner, self.repo);
                Ok(None)
            }
            Ok(resp) => {
                warn!("GitHub API error: {}", resp.status());
                Ok(None)
            }
            Err(e) => {
                warn!("Failed to fetch release: {}", e);
                Ok(None)
            }
        }
    }

    /// Get all releases from GitHub
    pub fn list_releases(&self) -> Result<Vec<String>> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases",
            self.owner, self.repo
        );

        let response = self.client
            .get(&url)
            .header("User-Agent", "dynamo-mcp")
            .send();

        match response {
            Ok(resp) if resp.status().is_success() => {
                let releases: Vec<GitHubRelease> = resp.json()?;
                Ok(releases.into_iter()
                    .filter(|r| !r.draft)
                    .map(|r| r.tag_name)
                    .collect())
            }
            _ => Ok(Vec::new())
        }
    }

    /// Get the default branch name
    pub fn get_default_branch(&self) -> Result<String> {
        let url = format!(
            "https://api.github.com/repos/{}/{}",
            self.owner, self.repo
        );

        let response = self.client
            .get(&url)
            .header("User-Agent", "dynamo-mcp")
            .send();

        match response {
            Ok(resp) if resp.status().is_success() => {
                let repo: GitHubRepo = resp.json()?;
                Ok(repo.default_branch)
            }
            _ => {
                // Fallback to main
                Ok("main".to_string())
            }
        }
    }

    /// Get the clone URL for the repository
    pub fn clone_url(&self) -> String {
        format!("https://github.com/{}/{}.git", self.owner, self.repo)
    }
}