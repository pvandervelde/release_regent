//! GitCommit builder for creating test GitCommit data

use crate::builders::{helpers::*, TestDataBuilder};
use chrono::{DateTime, Utc};
use release_regent_core::traits::git_operations::*;

/// Builder for creating test GitCommit data
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
    /// Create a new GitCommit builder with defaults
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

    /// Set GitCommit message
    pub fn with_message(mut self, message: &str) -> Self {
        self.message = message.to_string();
        self
    }

    /// Set conventional GitCommit message
    pub fn with_conventional(mut self, commit_type: &str, description: &str) -> Self {
        self.message = format!("{}: {}", commit_type, description);
        self
    }

    /// Set conventional GitCommit message (convenience method for single string)
    pub fn with_conventional_message(mut self, message: &str) -> Self {
        self.message = message.to_string();
        self
    }

    /// Set GitCommit author
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

impl TestDataBuilder<GitCommit> for CommitBuilder {
    fn build(self) -> GitCommit {
        let message = self.message;
        let subject = GitCommit::extract_subject(&message);
        let body = GitCommit::extract_body(&message);

        GitCommit {
            sha: self.sha,
            author: GitUser {
                email: self.author_email.clone(),
                username: Some(
                    self.author_name
                        .split('@')
                        .next()
                        .unwrap_or("user")
                        .to_string(),
                ),
                name: self.author_name.clone(),
            },
            committer: GitUser {
                email: self.committer_email.clone(),
                username: Some(
                    self.committer_name
                        .split('@')
                        .next()
                        .unwrap_or("user")
                        .to_string(),
                ),
                name: self.committer_name.clone(),
            },
            author_date: self.author_date,
            commit_date: self.committer_date,
            message: message.clone(),
            subject,
            body,
            parents: self.parents.into_iter().map(|sha| sha).collect(),
            files: Vec::new(), // Empty files list for testing
        }
    }

    fn reset(self) -> Self {
        Self::new()
    }
}
