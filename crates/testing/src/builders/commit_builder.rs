//! Commit builder for creating test commit data

use crate::builders::{helpers::*, TestDataBuilder};
use chrono::{DateTime, Utc};
use release_regent_core::traits::github_operations::*;

/// Builder for creating test commit data
#[derive(Debug, Clone)]
pub struct CommitBuilder {
    sha: String,
    message: String,
    author_name: String,
    author_email: String,
    committer_name: String,
    committer_email: String,
    author_date: DateTime<Utc>,
    committer_date: DateTime<Utc>,
    tree_sha: String,
    parents: Vec<String>,
}

impl CommitBuilder {
    /// Create a new commit builder with defaults
    pub fn new() -> Self {
        let timestamp = generate_recent_timestamp();
        let author_email = generate_email();

        Self {
            sha: generate_git_sha(),
            message: "feat: add new feature".to_string(),
            author_name: generate_github_login(),
            author_email: author_email.clone(),
            committer_name: generate_github_login(),
            committer_email: author_email,
            author_date: timestamp,
            committer_date: timestamp,
            tree_sha: generate_git_sha(),
            parents: vec![],
        }
    }

    /// Set commit message
    pub fn with_message(mut self, message: &str) -> Self {
        self.message = message.to_string();
        self
    }

    /// Set conventional commit message
    pub fn with_conventional_message(mut self, commit_type: &str, description: &str) -> Self {
        self.message = format!("{}: {}", commit_type, description);
        self
    }

    /// Set commit author
    pub fn with_author(mut self, name: &str, email: &str) -> Self {
        self.author_name = name.to_string();
        self.author_email = email.to_string();
        self
    }
}

impl Default for CommitBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TestDataBuilder<Commit> for CommitBuilder {
    fn build(self) -> Commit {
        // TODO: implement - placeholder for compilation
        // This should create a proper Commit struct
        Commit {
            sha: self.sha,
            author: GitUser {
                email: "user@example.com".to_string(),
                login: Some("user".to_string()),
                name: "user".to_string(),
            },
            committer: GitUser {
                email: "user@example.com".to_string(),
                login: Some("user".to_string()),
                name: "user".to_string(),
            },
            parents: self.parents.into_iter().map(|sha| sha).collect(),
            date: chrono::DateTime::parse_from_rfc3339("2025-07-26T17:54:00+12:00")
                .unwrap()
                .to_utc(),
            message: "This is not a good commit message".to_string(),
        }
    }

    fn reset(self) -> Self {
        Self::new()
    }
}
