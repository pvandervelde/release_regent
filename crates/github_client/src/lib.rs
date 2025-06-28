//! GitHub API client for Release Regent
//!
//! This crate provides a high-level GitHub API client specifically designed for Release Regent's
//! needs, including PR management, release creation, and authentication handling.

use octocrab::Octocrab;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info, warn};

pub mod errors;
pub mod pr_management;
pub mod release;

pub use errors::{GitHubError, GitHubResult};

/// Configuration for GitHub client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubConfig {
    /// GitHub App ID
    pub app_id: u64,
    /// GitHub App private key (PEM format)
    pub private_key: String,
    /// GitHub installation ID
    pub installation_id: u64,
    /// GitHub API base URL (for GitHub Enterprise)
    pub base_url: Option<String>,
}

/// GitHub client for Release Regent operations
#[derive(Debug, Clone)]
pub struct GitHubClient {
    octocrab: Arc<Octocrab>,
    config: GitHubConfig,
}

impl GitHubClient {
    /// Create a new GitHub client with the provided configuration
    ///
    /// # Arguments
    /// * `config` - GitHub configuration including app credentials
    ///
    /// # Returns
    /// * `GitHubResult<Self>` - The configured client or an error
    ///
    /// # Examples
    /// ```no_run
    /// use release_regent_github_client::{GitHubClient, GitHubConfig};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = GitHubConfig {
    ///     app_id: 123456,
    ///     private_key: "-----BEGIN RSA PRIVATE KEY-----\n...".to_string(),
    ///     installation_id: 789012,
    ///     base_url: None,
    /// };
    ///
    /// let client = GitHubClient::new(config).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(config: GitHubConfig) -> GitHubResult<Self> {
        debug!("Creating GitHub client for app_id: {}", config.app_id);

        // Create Octocrab instance
        let octocrab = match &config.base_url {
            Some(url) => {
                info!("Using custom GitHub base URL: {}", url);
                Octocrab::builder().base_uri(url)?.build()?
            }
            None => {
                debug!("Using default GitHub.com API");
                Octocrab::builder().build()?
            }
        };

        Ok(Self {
            octocrab: Arc::new(octocrab),
            config,
        })
    }

    /// Authenticate with GitHub using the app credentials
    ///
    /// This method generates a JWT token and exchanges it for an installation token.
    pub async fn authenticate(&self) -> GitHubResult<()> {
        info!(
            "Authenticating with GitHub for installation: {}",
            self.config.installation_id
        );

        // TODO: Implement JWT generation and token exchange
        // This will be implemented in subsequent issues
        warn!("Authentication not yet implemented - placeholder");

        Ok(())
    }

    /// Get the repository information
    ///
    /// # Arguments
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    pub async fn get_repository(&self, owner: &str, repo: &str) -> GitHubResult<Repository> {
        debug!("Getting repository information for {}/{}", owner, repo);

        // TODO: Implement repository fetching
        // This will be implemented in subsequent issues
        warn!("Repository fetching not yet implemented - placeholder");

        Ok(Repository {
            owner: owner.to_string(),
            name: repo.to_string(),
            default_branch: "main".to_string(),
        })
    }

    /// Get the underlying Octocrab client for advanced operations
    pub fn octocrab(&self) -> &Octocrab {
        &self.octocrab
    }

    /// Get the client configuration
    pub fn config(&self) -> &GitHubConfig {
        &self.config
    }
}

/// Repository information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    /// Repository owner
    pub owner: String,
    /// Repository name
    pub name: String,
    /// Default branch name
    pub default_branch: String,
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
