//! GitHub API operations trait
//!
//! This trait defines the contract for all GitHub API interactions required
//! by Release Regent. It extends the core GitOperations trait with GitHub-specific
//! functionality like pull requests, releases, and GitHub-specific metadata.

use super::git_operations::GitOperations;
use crate::CoreResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// GitHub API operations contract
///
/// This trait extends GitOperations with GitHub-specific functionality including
/// pull requests, releases, and GitHub's enhanced metadata and collaboration features.
///
/// It composes the core Git operations to provide a complete GitHub API interface
/// while maintaining clear separation between Git operations and platform features.
///
/// # Architecture
///
/// ```text
/// GitHubOperations
///       ↓ extends
/// GitOperations (core Git functionality)
/// ```
///
/// Version calculators should depend on GitOperations for commit access,
/// while release management depends on GitHubOperations for PR and release creation.
///
/// # Error Handling
///
/// All methods return `CoreResult<T>` and must properly map GitHub API errors
/// to `CoreError` variants. Common error scenarios include:
/// - Authentication failures
/// - Rate limiting
/// - Network timeouts
/// - Resource not found
/// - Insufficient permissions
///
/// # Rate Limiting
///
/// Implementations should handle GitHub's rate limiting automatically,
/// including proper backoff strategies and retry logic.
///
/// # Authentication
///
/// This trait assumes proper authentication has been established.
/// The authentication mechanism is implementation-specific.
#[async_trait]
pub trait GitHubOperations: GitOperations + Send + Sync {
    /// Create a new pull request
    ///
    /// # Parameters
    /// - `owner`: Repository owner name
    /// - `repo`: Repository name
    /// - `params`: Pull request creation parameters
    ///
    /// # Returns
    /// Created pull request information
    ///
    /// # Errors
    /// - `CoreError::GitHub` - API communication failed
    /// - `CoreError::InvalidInput` - Invalid branch names or parameters
    /// - `CoreError::NotSupported` - Insufficient permissions
    async fn create_pull_request(
        &self,
        owner: &str,
        repo: &str,
        params: CreatePullRequestParams,
    ) -> CoreResult<PullRequest>;

    /// Create a new release
    ///
    /// # Parameters
    /// - `owner`: Repository owner name
    /// - `repo`: Repository name
    /// - `params`: Release creation parameters
    ///
    /// # Returns
    /// Created release information
    ///
    /// # Errors
    /// - `CoreError::GitHub` - API communication failed
    /// - `CoreError::InvalidInput` - Invalid tag name or parameters
    /// - `CoreError::NotSupported` - Tag already exists or insufficient permissions
    async fn create_release(
        &self,
        owner: &str,
        repo: &str,
        params: CreateReleaseParams,
    ) -> CoreResult<Release>;

    /// Create a new tag
    ///
    /// # Parameters
    /// - `owner`: Repository owner name
    /// - `repo`: Repository name
    /// - `tag_name`: Name of the tag to create
    /// - `commit_sha`: Commit SHA to tag
    /// - `message`: Tag message for annotated tags (optional)
    /// - `tagger`: Tagger information (optional, defaults to authenticated user)
    ///
    /// # Returns
    /// Created tag information
    ///
    /// # Errors
    /// - `CoreError::GitHub` - API communication failed
    /// - `CoreError::InvalidInput` - Invalid tag name or commit SHA
    /// - `CoreError::NotSupported` - Tag already exists or insufficient permissions
    async fn create_tag(
        &self,
        owner: &str,
        repo: &str,
        tag_name: &str,
        commit_sha: &str,
        message: Option<String>,
        tagger: Option<GitUser>,
    ) -> CoreResult<Tag>;

    /// Get the latest release (non-draft, non-prerelease)
    ///
    /// # Parameters
    /// - `owner`: Repository owner name
    /// - `repo`: Repository name
    ///
    /// # Returns
    /// Latest stable release information, or None if no releases exist
    ///
    /// # Errors
    /// - `CoreError::GitHub` - API communication failed
    /// - `CoreError::InvalidInput` - Invalid repository parameters
    async fn get_latest_release(&self, owner: &str, repo: &str) -> CoreResult<Option<Release>>;

    /// Get pull request information
    ///
    /// # Parameters
    /// - `owner`: Repository owner name
    /// - `repo`: Repository name
    /// - `pr_number`: Pull request number
    ///
    /// # Returns
    /// Pull request information including status and metadata
    ///
    /// # Errors
    /// - `CoreError::GitHub` - API communication failed
    /// - `CoreError::InvalidInput` - Invalid PR number
    /// - `CoreError::NotSupported` - PR not found
    async fn get_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> CoreResult<PullRequest>;

    /// Get release information by tag name
    ///
    /// # Parameters
    /// - `owner`: Repository owner name
    /// - `repo`: Repository name
    /// - `tag`: Tag name to find release for
    ///
    /// # Returns
    /// Release information if found
    ///
    /// # Errors
    /// - `CoreError::GitHub` - API communication failed
    /// - `CoreError::InvalidInput` - Invalid tag name
    /// - `CoreError::NotSupported` - Release not found
    async fn get_release_by_tag(&self, owner: &str, repo: &str, tag: &str) -> CoreResult<Release>;

    /// List releases in a repository
    ///
    /// # Parameters
    /// - `owner`: Repository owner name
    /// - `repo`: Repository name
    /// - `per_page`: Number of releases per page (max 100)
    /// - `page`: Page number to retrieve (1-based)
    ///
    /// # Returns
    /// List of releases ordered by creation date (newest first)
    ///
    /// # Errors
    /// - `CoreError::GitHub` - API communication failed
    /// - `CoreError::InvalidInput` - Invalid pagination parameters
    async fn list_releases(
        &self,
        owner: &str,
        repo: &str,
        per_page: Option<u8>,
        page: Option<u32>,
    ) -> CoreResult<Vec<Release>>;

    /// List pull requests in a repository
    ///
    /// Returns pull requests matching the specified filters.
    ///
    /// # Parameters
    /// - `owner`: Repository owner name
    /// - `repo`: Repository name
    /// - `state`: PR state filter: `"open"`, `"closed"`, or `"all"` (default: `"open"`)
    /// - `head`: Filter by head branch name (optional)
    /// - `base`: Filter by base branch name (optional)
    /// - `per_page`: Number of PRs per page, max 100 (optional)
    /// - `page`: Page number, 1-based (optional)
    ///
    /// # Returns
    /// List of pull requests matching the filters
    ///
    /// # Errors
    /// - `CoreError::GitHub` - API communication failed
    /// - `CoreError::InvalidInput` - Invalid parameters
    async fn list_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        state: Option<&str>,
        head: Option<&str>,
        base: Option<&str>,
        per_page: Option<u8>,
        page: Option<u32>,
    ) -> CoreResult<Vec<PullRequest>>;

    /// Search pull requests using GitHub search query syntax
    ///
    /// Supports a subset of GitHub search qualifiers:
    /// - `is:open` / `is:closed` / `is:merged` — filter by state
    /// - `head:BRANCH` or `head:PREFIX*` — filter by head branch (glob prefix with `*`)
    /// - `base:BRANCH` — filter by exact base branch name
    ///
    /// # Parameters
    /// - `owner`: Repository owner name
    /// - `repo`: Repository name
    /// - `query`: Space-separated search qualifiers
    ///
    /// # Returns
    /// List of matching pull requests
    ///
    /// # Errors
    /// - `CoreError::GitHub` - API communication failed
    async fn search_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        query: &str,
    ) -> CoreResult<Vec<PullRequest>>;

    /// Update an existing pull request
    ///
    /// # Parameters
    /// - `owner`: Repository owner name
    /// - `repo`: Repository name
    /// - `pr_number`: Pull request number
    /// - `title`: New PR title (optional)
    /// - `body`: New PR body (optional)
    /// - `state`: New PR state ("open" or "closed") (optional)
    ///
    /// # Returns
    /// Updated pull request information
    ///
    /// # Errors
    /// - `CoreError::GitHub` - API communication failed
    /// - `CoreError::InvalidInput` - Invalid parameters
    /// - `CoreError::NotSupported` - PR not found or insufficient permissions
    async fn update_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        title: Option<String>,
        body: Option<String>,
        state: Option<String>,
    ) -> CoreResult<PullRequest>;

    /// Update an existing release
    ///
    /// # Parameters
    /// - `owner`: Repository owner name
    /// - `repo`: Repository name
    /// - `release_id`: Release ID to update
    /// - `params`: Release update parameters
    ///
    /// # Returns
    /// Updated release information
    ///
    /// # Errors
    /// - `CoreError::GitHub` - API communication failed
    /// - `CoreError::InvalidInput` - Invalid release ID or parameters
    /// - `CoreError::NotSupported` - Release not found or insufficient permissions
    async fn update_release(
        &self,
        owner: &str,
        repo: &str,
        release_id: u64,
        params: UpdateReleaseParams,
    ) -> CoreResult<Release>;
}

// Note: Git commit information is now provided by GitOperations trait
// Use super::git_operations::GitCommit for commit data

/// Parameters for creating a new pull request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePullRequestParams {
    /// Base branch (target)
    pub base: String,
    /// PR body/description
    pub body: Option<String>,
    /// Whether to create as draft
    pub draft: bool,
    /// Head branch (source)
    pub head: String,
    /// Whether maintainers can edit the PR
    pub maintainer_can_modify: bool,
    /// PR title
    pub title: String,
}

/// Parameters for creating a new release
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateReleaseParams {
    /// Release body/notes
    pub body: Option<String>,
    /// Whether this is a draft release
    pub draft: bool,
    /// Whether to generate release notes automatically
    pub generate_release_notes: bool,
    /// Release name/title
    pub name: Option<String>,
    /// Whether this is a pre-release
    pub prerelease: bool,
    /// Tag name for the release
    pub tag_name: String,
    /// Target commit SHA or branch name
    pub target_commitish: Option<String>,
}

/// Git user information (author/committer)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitUser {
    /// User email
    pub email: String,
    /// User login (GitHub username)
    pub login: Option<String>,
    /// User name
    pub name: String,
}

/// Pull request information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    /// Base branch information
    pub base: PullRequestBranch,
    /// PR body/description
    pub body: Option<String>,
    /// PR creation date
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Whether the PR is a draft
    pub draft: bool,
    /// Head branch information
    pub head: PullRequestBranch,
    /// PR merge date (if merged)
    pub merged_at: Option<chrono::DateTime<chrono::Utc>>,
    /// PR number
    pub number: u64,
    /// PR state (open, closed, merged)
    pub state: String,
    /// PR title
    pub title: String,
    /// PR update date
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// PR author
    pub user: GitUser,
}

/// Pull request branch information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestBranch {
    /// Branch name
    pub ref_name: String,
    /// Repository information (may be different for forks)
    pub repo: Repository,
    /// Commit SHA
    pub sha: String,
}

/// GitHub release information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    /// Release author
    pub author: GitUser,
    /// Release body/notes
    pub body: Option<String>,
    /// Release creation date
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Whether this is a draft release
    pub draft: bool,
    /// Release ID
    pub id: u64,
    /// Release name/title
    pub name: Option<String>,
    /// Whether this is a pre-release
    pub prerelease: bool,
    /// Release publication date
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Release tag name
    pub tag_name: String,
    /// Target commit SHA
    pub target_commitish: String,
}

/// Repository information from GitHub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    /// Repository clone URL (HTTPS)
    pub clone_url: String,
    /// Default branch name
    pub default_branch: String,
    /// Repository description
    pub description: Option<String>,
    /// Repository full name (owner/name)
    pub full_name: String,
    /// Repository homepage URL
    pub homepage: Option<String>,
    /// Repository ID
    pub id: u64,
    /// Repository name
    pub name: String,
    /// Repository owner/organization name
    pub owner: String,
    /// Whether the repository is private
    pub private: bool,
    /// Repository SSH URL
    pub ssh_url: String,
}

/// Git tag information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    /// Commit SHA that the tag points to
    pub commit_sha: String,
    /// Tag creation date
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Tag message (for annotated tags)
    pub message: Option<String>,
    /// Tag name
    pub name: String,
    /// Tag author information
    pub tagger: Option<GitUser>,
}

/// Parameters for updating an existing release
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateReleaseParams {
    /// Release body/notes
    pub body: Option<String>,
    /// Whether this is a draft release
    pub draft: Option<bool>,
    /// Release name/title
    pub name: Option<String>,
    /// Whether this is a pre-release
    pub prerelease: Option<bool>,
}
