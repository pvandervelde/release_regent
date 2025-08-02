//! Repository builder for creating test repository data

use crate::builders::{helpers::*, TestDataBuilder};
use chrono::Utc;
use release_regent_core::traits::github_operations::*;

/// Builder for creating test repository data
#[derive(Debug, Clone)]
pub struct RepositoryBuilder {
    name: String,
    owner_login: String,
    description: Option<String>,
    private: bool,
    default_branch: String,
    language: Option<String>,
}

impl RepositoryBuilder {
    /// Create a new repository builder with defaults
    pub fn new() -> Self {
        Self {
            name: generate_repo_name(),
            owner_login: generate_github_login(),
            description: Some("Test repository".to_string()),
            private: false,
            default_branch: "main".to_string(),
            language: Some("Rust".to_string()),
        }
    }

    /// Set repository name
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    /// Set repository owner
    pub fn with_owner(mut self, owner: &str) -> Self {
        self.owner_login = owner.to_string();
        self
    }

    /// Set default branch
    pub fn with_default_branch(mut self, branch: &str) -> Self {
        self.default_branch = branch.to_string();
        self
    }
}

impl Default for RepositoryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TestDataBuilder<Repository> for RepositoryBuilder {
    fn build(self) -> Repository {
        let id = generate_id();
        let _timestamp = Utc::now();

        Repository {
            id,
            name: self.name.clone(),
            full_name: format!("{}/{}", self.owner_login, self.name),
            private: self.private,
            owner: self.owner_login.clone(),
            description: self.description,
            ssh_url: format!("git@github.com:{}/{}.git", self.owner_login, self.name),
            clone_url: format!("https://github.com/{}/{}.git", self.owner_login, self.name),
            homepage: None,
            default_branch: self.default_branch,
        }
    }

    fn reset(self) -> Self {
        Self::new()
    }
}
