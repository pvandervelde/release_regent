//! Mock implementation of GitHubOperations trait
//!
//! Provides a comprehensive mock implementation that supports all GitHub API
//! operations required by Release Regent without making actual API calls.

use crate::mocks::{CallResult, MockConfig, MockState, SharedMockState};
use async_trait::async_trait;
use release_regent_core::{
    traits::{git_operations::*, github_operations::*},
    CoreError, CoreResult, GitHubOperations, GitOperations,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Mock implementation of GitHubOperations trait
///
/// This mock supports:
/// - Deterministic responses for reproducible testing
/// - Configurable error simulation
/// - Call tracking and verification
/// - Realistic data generation
/// - Performance testing support
///
/// # Example Usage
///
/// ```rust
/// use release_regent_testing::mocks::github_operations::MockGitHubOperations;
///
/// let mock = MockGitHubOperations::new()
///     .with_repository_exists(true)
///     .with_default_branch("main");
/// ```
#[derive(Debug)]
pub struct MockGitHubOperations {
    /// Shared state for tracking and configuration
    state: SharedMockState,
    /// Pre-configured repository data
    repositories: HashMap<String, Repository>,
    /// Pre-configured GitCommit data
    commits: HashMap<String, Vec<GitCommit>>,
    /// Pre-configured pull request data
    pull_requests: HashMap<String, Vec<PullRequest>>,
    /// Pre-configured tag data
    tags: HashMap<String, Vec<Tag>>,
    /// Pre-configured release data
    releases: HashMap<String, Vec<Release>>,
}

impl MockGitHubOperations {
    /// Get the total number of calls made
    ///
    /// # Returns
    /// Total call count
    pub async fn call_count(&self) -> u64 {
        self.state.read().await.call_count()
    }

    /// Get the call history for verification
    ///
    /// # Returns
    /// Reference to all recorded method calls
    pub async fn call_history(&self) -> Vec<crate::mocks::CallInfo> {
        self.state.read().await.call_history().to_vec()
    }

    /// Check if quota has been exceeded
    async fn check_quota(&self) -> CoreResult<()> {
        if self.state.read().await.is_quota_exceeded() {
            return Err(CoreError::rate_limit("Mock quota exceeded"));
        }
        Ok(())
    }

    /// Get commits between two references
    ///
    /// Returns the configured GitCommit data for the repository.
    ///
    /// # Parameters
    /// - `owner`: Repository owner
    /// - `name`: Repository name
    /// - `base`: Base reference (ignored in mock)
    /// - `head`: Head reference (ignored in mock)
    ///
    /// # Returns
    /// List of commits
    ///
    /// # Errors
    /// - `CoreError::NotFound` - No commits configured for repository
    /// - `CoreError::GitHub` - Simulated GitHub API error
    pub async fn get_commits_between(
        &self,
        owner: &str,
        name: &str,
        base: &str,
        head: &str,
    ) -> CoreResult<Vec<GitCommit>> {
        let method = "get_commits_between";
        let params = format!(
            "owner={}, name={}, base={}, head={}",
            owner, name, base, head
        );

        // Check quota and simulate latency
        self.check_quota().await?;
        self.simulate_latency().await;

        // Simulate failure if configured
        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let key = format!("{}/{}", owner, name);
        let commits = self.commits.get(&key).cloned().unwrap_or_default();

        self.record_call(method, &params, CallResult::Success).await;
        Ok(commits)
    }

    /// Get repository releases
    ///
    /// Returns the configured release data for the repository.
    ///
    /// # Parameters
    /// - `owner`: Repository owner
    /// - `name`: Repository name
    ///
    /// # Returns
    /// List of releases
    ///
    /// # Errors
    /// - `CoreError::GitHub` - Simulated GitHub API error
    pub async fn get_releases(&self, owner: &str, name: &str) -> CoreResult<Vec<Release>> {
        let method = "get_releases";
        let params = format!("owner={}, name={}", owner, name);

        // Check quota and simulate latency
        self.check_quota().await?;
        self.simulate_latency().await;

        // Simulate failure if configured
        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let key = format!("{}/{}", owner, name);
        let releases = self.releases.get(&key).cloned().unwrap_or_default();

        self.record_call(method, &params, CallResult::Success).await;
        Ok(releases)
    }

    /// Get repository tags
    ///
    /// Returns the configured tag data for the repository.
    ///
    /// # Parameters
    /// - `owner`: Repository owner
    /// - `name`: Repository name
    ///
    /// # Returns
    /// List of tags
    ///
    /// # Errors
    /// - `CoreError::GitHub` - Simulated GitHub API error
    pub async fn get_tags(&self, owner: &str, name: &str) -> CoreResult<Vec<Tag>> {
        let method = "get_tags";
        let params = format!("owner={}, name={}", owner, name);

        // Check quota and simulate latency
        self.check_quota().await?;
        self.simulate_latency().await;

        // Simulate failure if configured
        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let key = format!("{}/{}", owner, name);
        let tags = self.tags.get(&key).cloned().unwrap_or_default();

        self.record_call(method, &params, CallResult::Success).await;
        Ok(tags)
    }

    /// Create a new mock with default configuration
    ///
    /// Returns a mock configured for basic testing scenarios with:
    /// - Deterministic behavior enabled
    /// - Call tracking enabled
    /// - No failure simulation
    /// - Zero latency simulation
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(MockState::new())),
            repositories: HashMap::new(),
            commits: HashMap::new(),
            pull_requests: HashMap::new(),
            tags: HashMap::new(),
            releases: HashMap::new(),
        }
    }

    /// Record a method call for tracking
    async fn record_call(&self, method: &str, parameters: &str, result: CallResult) {
        self.state
            .write()
            .await
            .record_call(method, parameters, result);
    }

    /// Check if should simulate failure
    async fn should_simulate_failure(&self) -> bool {
        self.state.read().await.should_simulate_failure()
    }

    /// Simulate latency if configured
    async fn simulate_latency(&self) {
        self.state.read().await.simulate_latency().await;
    }

    /// Configure the mock with GitCommit data for a repository
    ///
    /// # Parameters
    /// - `owner`: Repository owner
    /// - `name`: Repository name
    /// - `commits`: List of commits to return
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_commits(mut self, owner: &str, name: &str, commits: Vec<GitCommit>) -> Self {
        let key = format!("{}/{}", owner, name);
        self.commits.insert(key, commits);
        self
    }

    /// Create a new mock with custom configuration
    ///
    /// # Parameters
    /// - `config`: Mock behavior configuration
    ///
    /// # Returns
    /// Configured mock instance
    pub fn with_config(config: MockConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(MockState::with_config(config))),
            repositories: HashMap::new(),
            commits: HashMap::new(),
            pull_requests: HashMap::new(),
            tags: HashMap::new(),
            releases: HashMap::new(),
        }
    }

    /// Configure the mock with a default branch name
    ///
    /// # Parameters
    /// - `branch`: Default branch name
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_default_branch(mut self, branch: &str) -> Self {
        // Update existing repositories with the new default branch
        for repository in self.repositories.values_mut() {
            repository.default_branch = branch.to_string();
        }
        self
    }

    /// Configure the mock with release data for a repository
    ///
    /// # Parameters
    /// - `owner`: Repository owner
    /// - `name`: Repository name
    /// - `releases`: List of releases to return
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_releases(mut self, owner: &str, name: &str, releases: Vec<Release>) -> Self {
        let key = format!("{}/{}", owner, name);
        self.releases.insert(key, releases);
        self
    }

    /// Configure the mock to return a specific repository
    ///
    /// # Parameters
    /// - `owner`: Repository owner
    /// - `name`: Repository name
    /// - `repository`: Repository data to return
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_repository(mut self, owner: &str, name: &str, repository: Repository) -> Self {
        let key = format!("{}/{}", owner, name);
        self.repositories.insert(key, repository);
        self
    }

    /// Configure the mock to indicate if a repository exists
    ///
    /// # Parameters
    /// - `exists`: Whether the repository should exist
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_repository_exists(self, exists: bool) -> Self {
        if exists {
            self.with_repository(
                "test",
                "repo",
                Repository {
                    id: 12345,
                    name: "repo".to_string(),
                    full_name: "test/repo".to_string(),
                    private: false,
                    owner: "test".to_string(),
                    description: Some("Test repository".to_string()),
                    ssh_url: "git@github.com:test/repo.git".to_string(),
                    clone_url: "https://github.com/test/repo.git".to_string(),
                    homepage: None,
                    default_branch: "main".to_string(),
                },
            )
        } else {
            self
        }
    }

    /// Configure the mock with pull request data for a repository
    ///
    /// # Parameters
    /// - `owner`: Repository owner
    /// - `repo`: Repository name
    /// - `prs`: List of pull requests to return
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_pull_requests(mut self, owner: &str, repo: &str, prs: Vec<PullRequest>) -> Self {
        let key = format!("{}/{}", owner, repo);
        self.pull_requests.insert(key, prs);
        self
    }

    /// Configure the mock with tag data for a repository
    ///
    /// # Parameters
    /// - `owner`: Repository owner
    /// - `name`: Repository name
    /// - `tags`: List of tags to return
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_tags(mut self, owner: &str, name: &str, tags: Vec<Tag>) -> Self {
        let key = format!("{}/{}", owner, name);
        self.tags.insert(key, tags);
        self
    }
}

impl Default for MockGitHubOperations {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GitHubOperations for MockGitHubOperations {
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
        _owner: &str,
        _repo: &str,
        _params: CreatePullRequestParams,
    ) -> CoreResult<PullRequest> {
        // TODO: implement - placeholder for compilation
        Err(CoreError::not_supported(
            "MockGitHubOperations",
            "create_pull_request not yet implemented",
        ))
    }

    /// Create a new release (placeholder implementation)
    ///
    /// # Parameters
    /// - `owner`: Repository owner
    /// - `name`: Repository name
    /// - `params`: Release creation parameters
    ///
    /// # Returns
    /// Created release information
    ///
    /// # Errors
    /// - `CoreError::NotSupported` - Not yet implemented in mock
    async fn create_release(
        &self,
        _owner: &str,
        _name: &str,
        _params: CreateReleaseParams,
    ) -> CoreResult<Release> {
        // TODO: implement - placeholder for compilation
        Err(CoreError::not_supported(
            "MockGitHubOperations",
            "create_release not yet implemented",
        ))
    }

    /// Create a Git tag (placeholder implementation)
    ///
    /// # Parameters
    /// - `owner`: Repository owner
    /// - `name`: Repository name
    /// - `params`: Tag creation parameters
    ///
    /// # Returns
    /// Created tag information
    ///
    /// # Errors
    /// - `CoreError::NotSupported` - Not yet implemented in mock
    async fn create_tag(
        &self,
        _owner: &str,
        _repo: &str,
        _tag_name: &str,
        _commit_sha: &str,
        _message: Option<String>,
        _tagger: Option<release_regent_core::traits::github_operations::GitUser>,
    ) -> CoreResult<Tag> {
        // TODO: implement - placeholder for compilation
        Err(CoreError::not_supported(
            "MockGitHubOperations",
            "create_tag not yet implemented",
        ))
    }

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
    async fn get_latest_release(&self, _owner: &str, _repo: &str) -> CoreResult<Option<Release>> {
        // TODO: implement - placeholder for compilation
        Err(CoreError::not_supported(
            "MockGitHubOperations",
            "get_latest_release not yet implemented",
        ))
    }

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
        _owner: &str,
        _repo: &str,
        _pr_number: u64,
    ) -> CoreResult<PullRequest> {
        // TODO: implement - placeholder for compilation
        Err(CoreError::not_supported(
            "MockGitHubOperations",
            "get_pull_request not yet implemented",
        ))
    }

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
    async fn get_release_by_tag(
        &self,
        _owner: &str,
        _repo: &str,
        _tag: &str,
    ) -> CoreResult<Release> {
        // TODO: implement - placeholder for compilation
        Err(CoreError::not_supported(
            "MockGitHubOperations",
            "get_release_by_tag not yet implemented",
        ))
    }

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
        _owner: &str,
        _repo: &str,
        _per_page: Option<u8>,
        _page: Option<u32>,
    ) -> CoreResult<Vec<Release>> {
        // TODO: implement - placeholder for compilation
        Err(CoreError::not_supported(
            "MockGitHubOperations",
            "list_releases not yet implemented",
        ))
    }

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
        _owner: &str,
        _repo: &str,
        _pr_number: u64,
        _title: Option<String>,
        _body: Option<String>,
        _state: Option<String>,
    ) -> CoreResult<PullRequest> {
        // TODO: implement - placeholder for compilation
        Err(CoreError::not_supported(
            "MockGitHubOperations",
            "update_pull_request not yet implemented",
        ))
    }

    /// Update an existing release (placeholder implementation)
    ///
    /// # Parameters
    /// - `owner`: Repository owner
    /// - `name`: Repository name
    /// - `release_id`: Release ID to update
    /// - `params`: Release update parameters
    ///
    /// # Returns
    /// Updated release information
    ///
    /// # Errors
    /// - `CoreError::NotSupported` - Not yet implemented in mock
    async fn update_release(
        &self,
        _owner: &str,
        _name: &str,
        _release_id: u64,
        _params: UpdateReleaseParams,
    ) -> CoreResult<Release> {
        // TODO: implement - placeholder for compilation
        Err(CoreError::not_supported(
            "MockGitHubOperations",
            "update_release not yet implemented",
        ))
    }

    async fn list_pull_requests(
        &self,
        _owner: &str,
        _repo: &str,
        _state: Option<&str>,
        _head: Option<&str>,
        _base: Option<&str>,
        _per_page: Option<u8>,
        _page: Option<u32>,
    ) -> CoreResult<Vec<PullRequest>> {
        // TODO: implement - placeholder for compilation
        Err(CoreError::not_supported(
            "MockGitHubOperations",
            "list_pull_requests not yet implemented",
        ))
    }

    async fn search_pull_requests(
        &self,
        _owner: &str,
        _repo: &str,
        _query: &str,
    ) -> CoreResult<Vec<PullRequest>> {
        // TODO: implement - placeholder for compilation
        Err(CoreError::not_supported(
            "MockGitHubOperations",
            "search_pull_requests not yet implemented",
        ))
    }
}

/// `GitOperations` implementation for `MockGitHubOperations`
#[async_trait]
impl GitOperations for MockGitHubOperations {
    async fn get_commits_between(
        &self,
        owner: &str,
        repo: &str,
        base: &str,
        head: &str,
        _options: GetCommitsOptions,
    ) -> CoreResult<Vec<GitCommit>> {
        let key = format!("{}/{}", owner, repo);
        if let Some(commits) = self.commits.get(&key) {
            // For simplicity, return all commits - in a real implementation
            // this would filter between base and head
            Ok(commits.clone())
        } else {
            Ok(Vec::new())
        }
    }

    async fn get_commit(&self, owner: &str, repo: &str, commit_sha: &str) -> CoreResult<GitCommit> {
        let key = format!("{}/{}", owner, repo);
        if let Some(commits) = self.commits.get(&key) {
            if let Some(commit) = commits.iter().find(|c| c.sha == commit_sha) {
                Ok(commit.clone())
            } else {
                Err(CoreError::network("Commit not found"))
            }
        } else {
            Err(CoreError::network("Repository not found"))
        }
    }

    async fn list_tags(
        &self,
        _owner: &str,
        _repo: &str,
        _options: ListTagsOptions,
    ) -> CoreResult<Vec<GitTag>> {
        // TODO: implement proper tag storage and retrieval
        Ok(Vec::new())
    }

    async fn get_tag(&self, _owner: &str, _repo: &str, _tag_name: &str) -> CoreResult<GitTag> {
        // TODO: implement proper tag storage and retrieval
        Err(CoreError::network("Tag not found"))
    }

    async fn tag_exists(&self, _owner: &str, _repo: &str, _tag_name: &str) -> CoreResult<bool> {
        // TODO: implement proper tag storage and retrieval
        Ok(false)
    }

    async fn get_head_commit(
        &self,
        owner: &str,
        repo: &str,
        _branch_name: Option<&str>,
    ) -> CoreResult<GitCommit> {
        // For simplicity, return the first commit in the list
        let key = format!("{}/{}", owner, repo);
        if let Some(commits) = self.commits.get(&key) {
            if let Some(commit) = commits.first() {
                Ok(commit.clone())
            } else {
                Err(CoreError::network("No commits found"))
            }
        } else {
            Err(CoreError::network("Repository not found"))
        }
    }

    async fn get_repository_info(&self, owner: &str, repo: &str) -> CoreResult<GitRepository> {
        let method = "get_repository";
        let params = format!("owner: {}, repo: {}", owner, repo);

        // Check quota and simulate latency
        self.check_quota().await?;
        self.simulate_latency().await;

        // Simulate failure if configured
        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        // Record successful call
        self.record_call(method, &params, CallResult::Success).await;

        // Convert Repository to GitRepository
        let key = format!("{}/{}", owner, repo);
        if let Some(repository) = self.repositories.get(&key) {
            Ok(GitRepository {
                name: repository.name.clone(),
                owner: repository.owner.clone(),
                full_name: repository.full_name.clone(),
                default_branch: repository.default_branch.clone(),
                clone_url: repository.clone_url.clone(),
                ssh_url: repository.ssh_url.clone(),
                private: repository.private,
                description: repository.description.clone(),
            })
        } else {
            Err(CoreError::network("Repository not found"))
        }
    }
}

#[cfg(test)]
#[path = "github_operations_tests.rs"]
mod tests;
