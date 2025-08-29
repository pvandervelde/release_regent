//! GitHub release and tag management for Release Regent
//!
//! This module handles creating Git tags and GitHub releases.

use crate::{GitHubClient, GitHubResult};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Options for creating a release
#[derive(Debug, Clone)]
pub struct CreateReleaseOptions {
    /// Tag name for the release
    pub tag_name: String,
    /// Release name/title
    pub name: String,
    /// Release body/description
    pub body: String,
    /// Git commitish (SHA, branch, or tag) to create the tag from
    pub target_commitish: Option<String>,
    /// Whether to create as draft
    pub draft: bool,
    /// Whether to mark as prerelease
    pub prerelease: bool,
}

/// GitHub release information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    /// Release ID
    pub id: u64,
    /// Release tag name
    pub tag_name: String,
    /// Release name/title
    pub name: String,
    /// Release body/description
    pub body: String,
    /// Whether this is a draft release
    pub draft: bool,
    /// Whether this is a prerelease
    pub prerelease: bool,
}

#[cfg(test)]
#[path = "release_tests.rs"]
mod tests;
