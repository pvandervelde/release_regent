//! Pull request management for Release Regent
//!
//! This module handles creating, updating, and managing release pull requests.

use crate::{GitHubClient, GitHubResult};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Options for creating a pull request
#[derive(Debug, Clone)]
pub struct CreatePullRequestOptions {
    /// PR title
    pub title: String,
    /// PR body
    pub body: String,
    /// Base branch to merge into
    pub base: String,
    /// Head branch containing changes
    pub head: String,
    /// Whether to create as draft
    pub draft: bool,
}

/// Pull request information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    /// PR number
    pub number: u64,
    /// PR title
    pub title: String,
    /// PR body
    pub body: String,
    /// Base branch (target)
    pub base: String,
    /// Head branch (source)
    pub head: String,
    /// Whether the PR is in draft state
    pub draft: bool,
}

#[cfg(test)]
#[path = "pr_management_tests.rs"]
mod tests;
