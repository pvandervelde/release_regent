//! Tag builder for creating test GitHub tag data

use crate::builders::{helpers::generate_commit_sha, TestDataBuilder};
use chrono::{DateTime, Utc};
use release_regent_core::traits::github_operations::{GitUser, Tag};

/// Builder for creating test GitHub [`Tag`] instances.
///
/// Produces [`Tag`] values with sensible defaults for unit and integration tests.
/// All fields can be overridden with the fluent builder methods.
///
/// # Example
///
/// ```rust
/// use release_regent_testing::builders::{TagBuilder, TestDataBuilder};
///
/// let tag = TagBuilder::new()
///     .with_name("v2.1.0")
///     .with_commit_sha("abc123def456")
///     .annotated()
///     .build();
///
/// assert_eq!(tag.name, "v2.1.0");
/// assert!(tag.message.is_some());
/// ```
#[derive(Debug, Clone)]
pub struct TagBuilder {
    name: String,
    commit_sha: String,
    message: Option<String>,
    tagger: Option<GitUser>,
    created_at: Option<DateTime<Utc>>,
}

impl TagBuilder {
    /// Create a new tag builder with sensible defaults.
    #[must_use] 
    pub fn new() -> Self {
        Self {
            name: "v1.0.0".to_string(),
            commit_sha: generate_commit_sha(),
            message: None,
            tagger: None,
            created_at: None,
        }
    }

    /// Set the tag name.
    #[must_use] 
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    /// Set the target commit SHA.
    #[must_use] 
    pub fn with_commit_sha(mut self, sha: &str) -> Self {
        self.commit_sha = sha.to_string();
        self
    }

    /// Set a tag message, making this an annotated tag.
    #[must_use] 
    pub fn with_message(mut self, message: &str) -> Self {
        self.message = Some(message.to_string());
        self
    }

    /// Set the tagger information.
    #[must_use] 
    pub fn with_tagger(mut self, tagger: GitUser) -> Self {
        self.tagger = Some(tagger);
        self
    }

    /// Set the tag creation timestamp.
    #[must_use] 
    pub fn with_created_at(mut self, created_at: DateTime<Utc>) -> Self {
        self.created_at = Some(created_at);
        self
    }

    /// Make this an annotated tag using a default release message.
    #[must_use] 
    pub fn annotated(mut self) -> Self {
        self.message = Some(format!("Release {}", self.name));
        self
    }
}

impl Default for TagBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TestDataBuilder<Tag> for TagBuilder {
    fn build(self) -> Tag {
        Tag {
            name: self.name,
            commit_sha: self.commit_sha,
            message: self.message,
            tagger: self.tagger,
            created_at: self.created_at,
        }
    }

    fn reset(self) -> Self {
        Self::new()
    }
}
