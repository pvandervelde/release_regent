//! Release builder for creating test GitHub release data

use crate::builders::{helpers::*, TestDataBuilder};
use chrono::{DateTime, Utc};
use release_regent_core::{
    traits::github_operations::{GitUser, Release},
    versioning::SemanticVersion,
};

/// Builder for creating test GitHub release data
#[derive(Debug, Clone)]
pub struct ReleaseBuilder {
    id: u64,
    tag_name: String,
    target_commitish: String,
    name: Option<String>,
    body: Option<String>,
    draft: bool,
    prerelease: bool,
    created_at: DateTime<Utc>,
    published_at: Option<DateTime<Utc>>,
    author: GitUser,
}

impl ReleaseBuilder {
    /// Create a new release builder with defaults
    pub fn new() -> Self {
        let version = SemanticVersion {
            major: 1,
            minor: 0,
            patch: 0,
            prerelease: None,
            build: None,
        };

        let now = Utc::now();

        Self {
            id: generate_id(),
            tag_name: format!("v{}", version),
            target_commitish: "main".to_string(),
            name: Some(format!("Release {}", version)),
            body: Some(generate_release_notes()),
            draft: false,
            prerelease: false,
            created_at: now,
            published_at: Some(now),
            author: GitUser {
                login: Some(generate_github_login()),
                name: generate_full_name(),
                email: generate_email(),
            },
        }
    }

    /// Set release tag name
    pub fn with_tag_name(mut self, tag_name: &str) -> Self {
        self.tag_name = tag_name.to_string();
        self
    }

    /// Set target commit-ish (branch/tag/commit)
    pub fn with_target_commitish(mut self, target_commitish: &str) -> Self {
        self.target_commitish = target_commitish.to_string();
        self
    }

    /// Set release name
    pub fn with_name<S: Into<String>>(mut self, name: Option<S>) -> Self {
        self.name = name.map(|s| s.into());
        self
    }

    /// Set release body/description
    pub fn with_body<S: Into<String>>(mut self, body: Option<S>) -> Self {
        self.body = body.map(|s| s.into());
        self
    }

    /// Set as draft release
    pub fn as_draft(mut self) -> Self {
        self.draft = true;
        self.published_at = None;
        self
    }

    /// Set as prerelease
    pub fn as_prerelease(mut self) -> Self {
        self.prerelease = true;
        self
    }

    /// Set release author
    pub fn with_author(mut self, author: GitUser) -> Self {
        self.author = author;
        self
    }

    /// Set created timestamp
    pub fn created_at(mut self, timestamp: DateTime<Utc>) -> Self {
        self.created_at = timestamp;
        self
    }

    /// Set published timestamp
    pub fn published_at(mut self, timestamp: Option<DateTime<Utc>>) -> Self {
        self.published_at = timestamp;
        self
    }

    /// Create from semantic version
    pub fn from_version(version: SemanticVersion) -> Self {
        Self::new()
            .with_tag_name(&format!("v{}", version))
            .with_name(Some(format!("Release {}", version)))
    }

    /// Create major release
    pub fn major_release() -> Self {
        let version = SemanticVersion {
            major: 2,
            minor: 0,
            patch: 0,
            prerelease: None,
            build: None,
        };
        Self::from_version(version)
    }

    /// Create minor release
    pub fn minor_release() -> Self {
        let version = SemanticVersion {
            major: 1,
            minor: 1,
            patch: 0,
            prerelease: None,
            build: None,
        };
        Self::from_version(version)
    }

    /// Create patch release
    pub fn patch_release() -> Self {
        let version = SemanticVersion {
            major: 1,
            minor: 0,
            patch: 1,
            prerelease: None,
            build: None,
        };
        Self::from_version(version)
    }

    /// Create beta release
    pub fn beta_release() -> Self {
        let version = SemanticVersion {
            major: 1,
            minor: 0,
            patch: 0,
            prerelease: Some("beta.1".to_string()),
            build: None,
        };
        Self::from_version(version).as_prerelease()
    }
}

impl TestDataBuilder<Release> for ReleaseBuilder {
    fn build(self) -> Release {
        Release {
            id: self.id,
            tag_name: self.tag_name,
            target_commitish: self.target_commitish,
            name: self.name,
            body: self.body,
            draft: self.draft,
            prerelease: self.prerelease,
            created_at: self.created_at,
            published_at: self.published_at,
            author: self.author,
        }
    }

    fn reset(self) -> Self {
        Self::new()
    }
}

impl Default for ReleaseBuilder {
    fn default() -> Self {
        Self::new()
    }
}
