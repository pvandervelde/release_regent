//! Git operations trait
//!
//! This trait defines the contract for core Git operations that are independent
//! of the specific Git hosting platform (GitHub, GitLab, Bitbucket, etc.).
//! It focuses on pure Git operations like commits, tags, and references.

use crate::CoreResult;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Core Git operations contract
///
/// This trait abstracts fundamental Git operations that are needed for version
/// calculation and release management, independent of the hosting platform.
///
/// The GitHubOperations trait composes this trait to provide platform-specific
/// functionality on top of core Git operations.
///
/// # Design Principles
///
/// - **Platform Independence**: Operations should work with any Git repository
/// - **Version Calculation Focus**: Optimized for commit history analysis
/// - **Clear Semantics**: Each operation has well-defined behavior and error cases
/// - **Async Support**: All operations are async-compatible for performance
///
/// # Error Handling
///
/// All methods return `CoreResult<T>` and must properly handle common Git scenarios:
/// - Repository not found or inaccessible
/// - Invalid references (branches, tags, commits)
/// - Network connectivity issues
/// - Authentication failures
/// - Concurrent access conflicts
///
/// # Authentication
///
/// Authentication is implementation-specific and should be handled by the
/// implementing type's constructor or configuration.
#[async_trait]
pub trait GitOperations: Send + Sync {
    /// Get commits between two references
    ///
    /// Returns commits in the range from `base` (exclusive) to `head` (inclusive),
    /// following Git's revision range semantics (`base..head`).
    ///
    /// # Parameters
    /// - `owner`: Repository owner (user or organization)
    /// - `repo`: Repository name
    /// - `base`: Base reference (commit SHA, branch, or tag) - excluded from results
    /// - `head`: Head reference (commit SHA, branch, or tag) - included in results
    /// - `options`: Additional options for commit retrieval
    ///
    /// # Returns
    /// List of commits in chronological order (oldest first), or empty if no commits
    /// exist in the range.
    ///
    /// # Errors
    /// - `CoreError::Git` - Git operation failed
    /// - `CoreError::InvalidInput` - Invalid references or parameters
    /// - `CoreError::NotFound` - Repository or references not found
    ///
    /// # Examples
    /// ```ignore
    /// // Get commits since last release
    /// let commits = git.get_commits_between(
    ///     "myorg", "myrepo", "v1.0.0", "main",
    ///     GetCommitsOptions::default()
    /// ).await?;
    /// ```
    async fn get_commits_between(
        &self,
        owner: &str,
        repo: &str,
        base: &str,
        head: &str,
        options: GetCommitsOptions,
    ) -> CoreResult<Vec<GitCommit>>;

    /// Get specific commit information
    ///
    /// Retrieves detailed information about a single commit, including
    /// message, author, timestamp, and parent relationships.
    ///
    /// # Parameters
    /// - `owner`: Repository owner
    /// - `repo`: Repository name
    /// - `commit_sha`: Full or abbreviated commit SHA
    ///
    /// # Returns
    /// Detailed commit information
    ///
    /// # Errors
    /// - `CoreError::Git` - Git operation failed
    /// - `CoreError::InvalidInput` - Invalid commit SHA format
    /// - `CoreError::NotFound` - Commit not found
    async fn get_commit(&self, owner: &str, repo: &str, commit_sha: &str) -> CoreResult<GitCommit>;

    /// List all tags in the repository
    ///
    /// Returns all Git tags, optionally filtered and paginated.
    /// Tags are returned in creation order (newest first by default).
    ///
    /// # Parameters
    /// - `owner`: Repository owner
    /// - `repo`: Repository name
    /// - `options`: Options for tag listing and filtering
    ///
    /// # Returns
    /// List of tags matching the criteria
    ///
    /// # Errors
    /// - `CoreError::Git` - Git operation failed
    /// - `CoreError::NotFound` - Repository not found
    async fn list_tags(
        &self,
        owner: &str,
        repo: &str,
        options: ListTagsOptions,
    ) -> CoreResult<Vec<GitTag>>;

    /// Get specific tag information
    ///
    /// Retrieves detailed information about a single tag, including
    /// target commit, message (for annotated tags), and tagger information.
    ///
    /// # Parameters
    /// - `owner`: Repository owner
    /// - `repo`: Repository name
    /// - `tag_name`: Tag name (without refs/tags/ prefix)
    ///
    /// # Returns
    /// Detailed tag information
    ///
    /// # Errors
    /// - `CoreError::Git` - Git operation failed
    /// - `CoreError::InvalidInput` - Invalid tag name
    /// - `CoreError::NotFound` - Tag not found
    async fn get_tag(&self, owner: &str, repo: &str, tag_name: &str) -> CoreResult<GitTag>;

    /// Check if a tag exists
    ///
    /// Efficiently checks whether a tag exists without retrieving full tag information.
    ///
    /// # Parameters
    /// - `owner`: Repository owner
    /// - `repo`: Repository name
    /// - `tag_name`: Tag name to check
    ///
    /// # Returns
    /// True if tag exists, false otherwise
    ///
    /// # Errors
    /// - `CoreError::Git` - Git operation failed
    /// - `CoreError::NotFound` - Repository not found
    async fn tag_exists(&self, owner: &str, repo: &str, tag_name: &str) -> CoreResult<bool>;

    /// Get the current HEAD commit
    ///
    /// Returns the commit that the default branch (or specified branch) currently points to.
    ///
    /// # Parameters
    /// - `owner`: Repository owner
    /// - `repo`: Repository name
    /// - `branch`: Branch name (optional, defaults to repository's default branch)
    ///
    /// # Returns
    /// HEAD commit information
    ///
    /// # Errors
    /// - `CoreError::Git` - Git operation failed
    /// - `CoreError::NotFound` - Repository or branch not found
    async fn get_head_commit(
        &self,
        owner: &str,
        repo: &str,
        branch: Option<&str>,
    ) -> CoreResult<GitCommit>;

    /// Get repository information
    ///
    /// Returns basic repository metadata needed for Git operations.
    ///
    /// # Parameters
    /// - `owner`: Repository owner
    /// - `repo`: Repository name
    ///
    /// # Returns
    /// Repository information including default branch and clone URLs
    ///
    /// # Errors
    /// - `CoreError::Git` - Git operation failed
    /// - `CoreError::NotFound` - Repository not found or inaccessible
    async fn get_repository_info(&self, owner: &str, repo: &str) -> CoreResult<GitRepository>;
}

/// Options for retrieving commits between references
#[derive(Debug, Clone, Default)]
pub struct GetCommitsOptions {
    /// Maximum number of commits to return (default: no limit)
    pub limit: Option<usize>,
    /// Skip this many commits from the beginning (default: 0)
    pub offset: Option<usize>,
    /// Include merge commits (default: true)
    pub include_merges: bool,
    /// Only include commits that modify these paths (default: all paths)
    pub paths: Option<Vec<String>>,
    /// Author email filter (default: all authors)
    pub author: Option<String>,
    /// Since this timestamp (default: no limit)
    pub since: Option<DateTime<Utc>>,
    /// Until this timestamp (default: no limit)
    pub until: Option<DateTime<Utc>>,
}

/// Options for listing tags
#[derive(Debug, Clone, Default)]
pub struct ListTagsOptions {
    /// Maximum number of tags to return (default: 100)
    pub limit: Option<usize>,
    /// Skip this many tags from the beginning (default: 0)
    pub offset: Option<usize>,
    /// Tag name pattern filter (shell glob pattern, default: all tags)
    pub pattern: Option<String>,
    /// Sort order (default: CreationDate descending)
    pub sort: TagSortOrder,
}

/// Sort order for tag listing
#[derive(Debug, Clone, Default)]
pub enum TagSortOrder {
    /// Sort by creation date, newest first
    #[default]
    CreationDateDesc,
    /// Sort by creation date, oldest first
    CreationDateAsc,
    /// Sort by tag name alphabetically
    NameAsc,
    /// Sort by tag name reverse alphabetically
    NameDesc,
    /// Sort by semantic version (if tags follow semver)
    SemanticVersionDesc,
    /// Sort by semantic version ascending
    SemanticVersionAsc,
}

/// Git commit information
///
/// Represents a single Git commit with all metadata needed for version calculation
/// and changelog generation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitCommit {
    /// Commit SHA (full 40-character hex string)
    pub sha: String,
    /// Commit author information
    pub author: GitUser,
    /// Commit committer information (may differ from author)
    pub committer: GitUser,
    /// Author timestamp (when changes were made)
    pub author_date: DateTime<Utc>,
    /// Commit timestamp (when commit was created)
    pub commit_date: DateTime<Utc>,
    /// Full commit message including subject and body
    pub message: String,
    /// Commit message subject line (first line)
    pub subject: String,
    /// Commit message body (everything after first line, if present)
    pub body: Option<String>,
    /// Parent commit SHAs (empty for root commit, multiple for merge commits)
    pub parents: Vec<String>,
    /// Files modified in this commit (optional, may be empty for performance)
    pub files: Vec<String>,
}

/// Git tag information
///
/// Represents a Git tag, which can be either lightweight or annotated.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitTag {
    /// Tag name (without refs/tags/ prefix)
    pub name: String,
    /// Commit SHA that this tag points to
    pub target_sha: String,
    /// Tag type (lightweight or annotated)
    pub tag_type: GitTagType,
    /// Tag message (only for annotated tags)
    pub message: Option<String>,
    /// Tagger information (only for annotated tags)
    pub tagger: Option<GitUser>,
    /// Tag creation date (only for annotated tags)
    pub created_at: Option<DateTime<Utc>>,
}

/// Git tag type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GitTagType {
    /// Lightweight tag (just a reference to a commit)
    Lightweight,
    /// Annotated tag (has message, tagger, and date)
    Annotated,
}

/// Git user information (author/committer/tagger)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitUser {
    /// User name
    pub name: String,
    /// User email address
    pub email: String,
    /// Platform-specific username (e.g., GitHub login)
    pub username: Option<String>,
}

/// Git repository information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitRepository {
    /// Repository name
    pub name: String,
    /// Repository owner/organization
    pub owner: String,
    /// Full repository name (owner/name)
    pub full_name: String,
    /// Default branch name (usually main or master)
    pub default_branch: String,
    /// Repository clone URL (HTTPS)
    pub clone_url: String,
    /// Repository SSH URL
    pub ssh_url: String,
    /// Whether the repository is private
    pub private: bool,
    /// Repository description
    pub description: Option<String>,
}

impl GitCommit {
    /// Extract the subject line from a commit message
    ///
    /// Returns the first line of the commit message, which is conventionally
    /// used as the subject or summary.
    pub fn extract_subject(message: &str) -> String {
        message.lines().next().unwrap_or("").to_string()
    }

    /// Extract the body from a commit message
    ///
    /// Returns everything after the first line and any following blank lines.
    /// Returns None if there's no body content.
    pub fn extract_body(message: &str) -> Option<String> {
        let mut lines = message.lines();
        lines.next(); // Skip subject line

        // Skip blank lines after subject
        let remaining: Vec<&str> = lines.skip_while(|line| line.trim().is_empty()).collect();

        if remaining.is_empty() {
            None
        } else {
            Some(remaining.join("\n"))
        }
    }

    /// Check if this is a merge commit
    ///
    /// A merge commit has multiple parents (more than one).
    pub fn is_merge_commit(&self) -> bool {
        self.parents.len() > 1
    }
}

impl GitTag {
    /// Check if this tag follows semantic versioning
    ///
    /// Returns true if the tag name matches semantic versioning pattern,
    /// optionally with a 'v' prefix.
    pub fn is_semver(&self) -> bool {
        let name = self.name.strip_prefix('v').unwrap_or(&self.name);
        // Basic semver pattern check
        let parts: Vec<&str> = name.split('.').collect();
        if parts.len() < 3 {
            return false;
        }

        parts
            .iter()
            .take(3)
            .all(|part| part.chars().all(|c| c.is_ascii_digit()) && !part.is_empty())
    }

    /// Parse the semantic version from this tag
    ///
    /// Attempts to parse the tag name as a semantic version.
    /// Returns None if the tag doesn't follow semantic versioning.
    pub fn parse_semver(&self) -> Option<crate::versioning::SemanticVersion> {
        if !self.is_semver() {
            return None;
        }

        crate::versioning::VersionCalculator::parse_version(&self.name).ok()
    }
}
