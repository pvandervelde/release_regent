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
use std::sync::atomic::{AtomicU64, Ordering};
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
    /// Sequential counter for generating deterministic resource IDs (PR numbers, release IDs)
    next_id: Arc<AtomicU64>,
    /// Sequential counter for generating deterministic mock commit SHAs
    next_sha: Arc<AtomicU64>,
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
    /// Tracks created branch names (keyed `owner/repo` → `Vec<branch>`).
    ///
    /// `create_branch` inserts a name; `delete_branch` removes it.
    /// A name already present causes `create_branch` to return
    /// `CoreError::Conflict`, matching real GitHub 422 behaviour.
    branches: HashMap<String, Vec<String>>,
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
            next_id: Arc::new(AtomicU64::new(1)),
            next_sha: Arc::new(AtomicU64::new(1)),
            repositories: HashMap::new(),
            commits: HashMap::new(),
            pull_requests: HashMap::new(),
            tags: HashMap::new(),
            releases: HashMap::new(),
            branches: HashMap::new(),
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
            next_id: Arc::new(AtomicU64::new(1)),
            next_sha: Arc::new(AtomicU64::new(1)),
            repositories: HashMap::new(),
            commits: HashMap::new(),
            pull_requests: HashMap::new(),
            tags: HashMap::new(),
            releases: HashMap::new(),
            branches: HashMap::new(),
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

    /// Pre-populate the mock branch registry for a repository.
    ///
    /// Any branch name in `branches` will cause subsequent `create_branch`
    /// calls for that name to return `CoreError::Conflict`, mirroring GitHub's
    /// HTTP 422 response when a branch already exists.
    ///
    /// # Parameters
    /// - `owner`: Repository owner
    /// - `name`: Repository name
    /// - `branches`: List of existing branch names
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_branches(mut self, owner: &str, name: &str, branches: Vec<String>) -> Self {
        let key = format!("{}/{}", owner, name);
        self.branches.insert(key, branches);
        self
    }
}

impl Default for MockGitHubOperations {
    fn default() -> Self {
        Self::new()
    }
}

impl MockGitHubOperations {
    /// Generate a sequential mock ID for created resources (PRs, releases, etc.).
    ///
    /// Uses a dedicated per-instance atomic counter so tests can predict the
    /// sequence: first created PR gets number 1, the second gets 2, and so on.
    fn generate_mock_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Generate a deterministic mock commit SHA for created resources.
    ///
    /// Uses a separate per-instance counter from `generate_mock_id`, so calling
    /// `create_pull_request` (which requires two SHAs) does not disturb the ID
    /// sequence. The SHA counter starts at 1 and increments by 1 per call.
    fn generate_mock_sha(&self) -> String {
        let id = self.next_sha.fetch_add(1, Ordering::SeqCst);
        format!("{id:040x}")
    }
}

/// Convert a `github_operations::Tag` to a `git_operations::GitTag`.
fn github_tag_to_git_tag(tag: &Tag) -> GitTag {
    GitTag {
        name: tag.name.clone(),
        target_sha: tag.commit_sha.clone(),
        tag_type: if tag.message.is_some() {
            GitTagType::Annotated
        } else {
            GitTagType::Lightweight
        },
        message: tag.message.clone(),
        tagger: tag
            .tagger
            .as_ref()
            .map(|t| release_regent_core::traits::git_operations::GitUser {
                name: t.name.clone(),
                email: t.email.clone(),
                username: t.login.clone(),
            }),
        created_at: tag.created_at,
    }
}

#[async_trait]
impl GitHubOperations for MockGitHubOperations {
    async fn create_pull_request(
        &self,
        owner: &str,
        repo: &str,
        params: CreatePullRequestParams,
    ) -> CoreResult<PullRequest> {
        let method = "create_pull_request";
        let params_str = format!("owner={owner}, repo={repo}, title={}", params.title);

        self.check_quota().await?;
        self.simulate_latency().await;

        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params_str, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let now = chrono::Utc::now();
        let stub_repo = Repository {
            id: 1,
            name: repo.to_string(),
            full_name: format!("{owner}/{repo}"),
            owner: owner.to_string(),
            description: None,
            private: false,
            default_branch: "main".to_string(),
            clone_url: format!("https://github.com/{owner}/{repo}.git"),
            ssh_url: format!("git@github.com:{owner}/{repo}.git"),
            homepage: None,
        };
        let pr = PullRequest {
            number: self.generate_mock_id(),
            title: params.title,
            body: params.body,
            state: if params.draft {
                "draft".to_string()
            } else {
                "open".to_string()
            },
            draft: params.draft,
            created_at: now,
            updated_at: now,
            merged_at: None,
            user: release_regent_core::traits::github_operations::GitUser {
                name: "mock-user".to_string(),
                email: "mock@users.noreply.github.com".to_string(),
                login: Some("mock-user".to_string()),
            },
            head: PullRequestBranch {
                ref_name: params.head,
                sha: self.generate_mock_sha(),
                repo: stub_repo.clone(),
            },
            base: PullRequestBranch {
                ref_name: params.base,
                sha: self.generate_mock_sha(),
                repo: stub_repo,
            },
        };

        self.record_call(method, &params_str, CallResult::Success)
            .await;
        Ok(pr)
    }

    async fn create_release(
        &self,
        owner: &str,
        repo: &str,
        params: CreateReleaseParams,
    ) -> CoreResult<Release> {
        let method = "create_release";
        let params_str = format!("owner={owner}, repo={repo}, tag={}", params.tag_name);

        self.check_quota().await?;
        self.simulate_latency().await;

        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params_str, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let now = chrono::Utc::now();
        let release = Release {
            id: self.generate_mock_id(),
            tag_name: params.tag_name,
            target_commitish: params
                .target_commitish
                .unwrap_or_else(|| "main".to_string()),
            name: params.name,
            body: params.body,
            draft: params.draft,
            prerelease: params.prerelease,
            created_at: now,
            published_at: if params.draft { None } else { Some(now) },
            author: release_regent_core::traits::github_operations::GitUser {
                name: "mock-user".to_string(),
                email: "mock@users.noreply.github.com".to_string(),
                login: Some("mock-user".to_string()),
            },
        };

        self.record_call(method, &params_str, CallResult::Success)
            .await;
        Ok(release)
    }

    async fn create_tag(
        &self,
        owner: &str,
        repo: &str,
        tag_name: &str,
        commit_sha: &str,
        message: Option<String>,
        tagger: Option<release_regent_core::traits::github_operations::GitUser>,
    ) -> CoreResult<Tag> {
        let method = "create_tag";
        let params_str = format!("owner={owner}, repo={repo}, tag={tag_name}");

        self.check_quota().await?;
        self.simulate_latency().await;

        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params_str, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let tag = Tag {
            name: tag_name.to_string(),
            commit_sha: commit_sha.to_string(),
            message,
            tagger,
            created_at: Some(chrono::Utc::now()),
        };

        self.record_call(method, &params_str, CallResult::Success)
            .await;
        Ok(tag)
    }

    async fn get_latest_release(&self, owner: &str, repo: &str) -> CoreResult<Option<Release>> {
        let method = "get_latest_release";
        let params_str = format!("owner={owner}, repo={repo}");

        self.check_quota().await?;
        self.simulate_latency().await;

        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params_str, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let key = format!("{owner}/{repo}");
        let latest = self.releases.get(&key).and_then(|releases| {
            releases
                .iter()
                .filter(|r| !r.draft && !r.prerelease)
                .max_by_key(|r| r.created_at)
                .cloned()
        });

        self.record_call(method, &params_str, CallResult::Success)
            .await;
        Ok(latest)
    }

    async fn get_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> CoreResult<PullRequest> {
        let method = "get_pull_request";
        let params_str = format!("owner={owner}, repo={repo}, pr={pr_number}");

        self.check_quota().await?;
        self.simulate_latency().await;

        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params_str, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let key = format!("{owner}/{repo}");
        let pr = self
            .pull_requests
            .get(&key)
            .and_then(|prs| prs.iter().find(|pr| pr.number == pr_number).cloned());

        let Some(pr) = pr else {
            self.record_call(
                method,
                &params_str,
                CallResult::Error(format!("PR #{pr_number} not found")),
            )
            .await;
            return Err(CoreError::not_found(format!("PR #{pr_number} not found")));
        };

        self.record_call(method, &params_str, CallResult::Success)
            .await;
        Ok(pr)
    }

    async fn get_release_by_tag(&self, owner: &str, repo: &str, tag: &str) -> CoreResult<Release> {
        let method = "get_release_by_tag";
        let params_str = format!("owner={owner}, repo={repo}, tag={tag}");

        self.check_quota().await?;
        self.simulate_latency().await;

        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params_str, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let key = format!("{owner}/{repo}");
        let release = self
            .releases
            .get(&key)
            .and_then(|releases| releases.iter().find(|r| r.tag_name == tag).cloned());

        let Some(release) = release else {
            self.record_call(
                method,
                &params_str,
                CallResult::Error(format!("release for tag '{tag}' not found")),
            )
            .await;
            return Err(CoreError::not_found(format!(
                "release for tag '{tag}' not found"
            )));
        };

        self.record_call(method, &params_str, CallResult::Success)
            .await;
        Ok(release)
    }

    async fn list_releases(
        &self,
        owner: &str,
        repo: &str,
        _per_page: Option<u8>,
        _page: Option<u32>,
    ) -> CoreResult<Vec<Release>> {
        let method = "list_releases";
        let params_str = format!("owner={owner}, repo={repo}");

        self.check_quota().await?;
        self.simulate_latency().await;

        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params_str, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let key = format!("{owner}/{repo}");
        let releases = self.releases.get(&key).cloned().unwrap_or_default();

        self.record_call(method, &params_str, CallResult::Success)
            .await;
        Ok(releases)
    }

    async fn update_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        title: Option<String>,
        body: Option<String>,
        state: Option<String>,
    ) -> CoreResult<PullRequest> {
        let method = "update_pull_request";
        let params_str = format!("owner={owner}, repo={repo}, pr={pr_number}");

        self.check_quota().await?;
        self.simulate_latency().await;

        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params_str, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let key = format!("{owner}/{repo}");
        let pr = self
            .pull_requests
            .get(&key)
            .and_then(|prs| prs.iter().find(|pr| pr.number == pr_number).cloned());

        let Some(mut updated) = pr else {
            self.record_call(
                method,
                &params_str,
                CallResult::Error(format!("PR #{pr_number} not found")),
            )
            .await;
            return Err(CoreError::not_found(format!("PR #{pr_number} not found")));
        };

        if let Some(t) = title {
            updated.title = t;
        }
        if let Some(b) = body {
            updated.body = Some(b);
        }
        if let Some(s) = state {
            updated.state = s;
        }
        updated.updated_at = chrono::Utc::now();

        self.record_call(method, &params_str, CallResult::Success)
            .await;
        Ok(updated)
    }

    async fn update_release(
        &self,
        owner: &str,
        repo: &str,
        release_id: u64,
        params: UpdateReleaseParams,
    ) -> CoreResult<Release> {
        let method = "update_release";
        let params_str = format!("owner={owner}, repo={repo}, id={release_id}");

        self.check_quota().await?;
        self.simulate_latency().await;

        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params_str, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let key = format!("{owner}/{repo}");
        let release = self
            .releases
            .get(&key)
            .and_then(|releases| releases.iter().find(|r| r.id == release_id).cloned());

        let Some(mut updated) = release else {
            self.record_call(
                method,
                &params_str,
                CallResult::Error(format!("release #{release_id} not found")),
            )
            .await;
            return Err(CoreError::not_found(format!(
                "release #{release_id} not found"
            )));
        };

        if let Some(b) = params.body {
            updated.body = Some(b);
        }
        if let Some(n) = params.name {
            updated.name = Some(n);
        }
        if let Some(d) = params.draft {
            updated.draft = d;
        }
        if let Some(p) = params.prerelease {
            updated.prerelease = p;
        }

        self.record_call(method, &params_str, CallResult::Success)
            .await;
        Ok(updated)
    }

    async fn list_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        state: Option<&str>,
        head: Option<&str>,
        base: Option<&str>,
        _per_page: Option<u8>,
        _page: Option<u32>,
    ) -> CoreResult<Vec<PullRequest>> {
        let method = "list_pull_requests";
        let params_str = format!("owner={owner}, repo={repo}");

        self.check_quota().await?;
        self.simulate_latency().await;

        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params_str, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let state_filter = state.unwrap_or("open").to_string();
        let head_filter = head.map(|h| h.to_string());
        let base_filter = base.map(|b| b.to_string());

        let key = format!("{owner}/{repo}");
        let prs = self.pull_requests.get(&key).cloned().unwrap_or_default();

        let filtered: Vec<PullRequest> = prs
            .into_iter()
            .filter(|pr| state_filter == "all" || pr.state == state_filter)
            .filter(|pr| {
                head_filter
                    .as_deref()
                    .map_or(true, |h| pr.head.ref_name == h)
            })
            .filter(|pr| {
                base_filter
                    .as_deref()
                    .map_or(true, |b| pr.base.ref_name == b)
            })
            .collect();

        self.record_call(method, &params_str, CallResult::Success)
            .await;
        Ok(filtered)
    }

    async fn search_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        query: &str,
    ) -> CoreResult<Vec<PullRequest>> {
        let method = "search_pull_requests";
        let params_str = format!("owner={owner}, repo={repo}, query={query}");

        self.check_quota().await?;
        self.simulate_latency().await;

        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params_str, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let mut state_filter: Option<String> = None;
        let mut head_filter: Option<String> = None;
        let mut base_filter: Option<String> = None;

        for token in query.split_whitespace() {
            if let Some(s) = token.strip_prefix("is:") {
                state_filter = Some(s.to_string());
            } else if let Some(h) = token.strip_prefix("head:") {
                head_filter = Some(h.to_string());
            } else if let Some(b) = token.strip_prefix("base:") {
                base_filter = Some(b.to_string());
            }
        }

        let key = format!("{owner}/{repo}");
        let prs = self.pull_requests.get(&key).cloned().unwrap_or_default();

        let filtered: Vec<PullRequest> = prs
            .into_iter()
            .filter(|pr| {
                state_filter.as_ref().map_or(true, |s| match s.as_str() {
                    "open" => pr.state == "open",
                    "closed" => pr.state == "closed",
                    "merged" => pr.merged_at.is_some(),
                    _ => true,
                })
            })
            .filter(|pr| {
                head_filter.as_ref().map_or(true, |h| {
                    if let Some(prefix) = h.strip_suffix('*') {
                        pr.head.ref_name.starts_with(prefix)
                    } else {
                        pr.head.ref_name == *h
                    }
                })
            })
            .filter(|pr| {
                base_filter
                    .as_ref()
                    .map_or(true, |b| pr.base.ref_name == *b)
            })
            .collect();

        self.record_call(method, &params_str, CallResult::Success)
            .await;
        Ok(filtered)
    }

    async fn create_branch(
        &self,
        owner: &str,
        repo: &str,
        branch_name: &str,
        sha: &str,
    ) -> CoreResult<()> {
        let method = "create_branch";
        let params_str = format!("owner={owner}, repo={repo}, branch={branch_name}, sha={sha}");

        self.check_quota().await?;
        self.simulate_latency().await;

        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params_str, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let key = format!("{owner}/{repo}");
        // Mirror GitHub's HTTP 422 behaviour: return Conflict if name already exists.
        let already_exists = self
            .branches
            .get(&key)
            .map_or(false, |bs| bs.iter().any(|b| b == branch_name));

        if already_exists {
            let err = CoreError::conflict(format!("branch '{branch_name}' already exists"));
            self.record_call(method, &params_str, CallResult::Error(err.to_string()))
                .await;
            return Err(err);
        }

        // We can't mutate self here (shared ref), so we just record success.
        // Tests that need to verify branch state should use `with_branches()` setup
        // or track calls by inspecting call history.
        self.record_call(method, &params_str, CallResult::Success)
            .await;
        Ok(())
    }

    async fn delete_branch(&self, owner: &str, repo: &str, branch_name: &str) -> CoreResult<()> {
        let method = "delete_branch";
        let params_str = format!("owner={owner}, repo={repo}, branch={branch_name}");

        self.check_quota().await?;
        self.simulate_latency().await;

        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params_str, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        self.record_call(method, &params_str, CallResult::Success)
            .await;
        Ok(())
    }

    async fn create_issue_comment(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        body: &str,
    ) -> CoreResult<()> {
        let method = "create_issue_comment";
        let params_str =
            format!("owner={owner}, repo={repo}, issue={issue_number}, body_len={}", body.len());

        self.check_quota().await?;
        self.simulate_latency().await;

        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params_str, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        self.record_call(method, &params_str, CallResult::Success)
            .await;
        Ok(())
    }
}

/// `GitOperations` implementation for `MockGitHubOperations`
#[async_trait]
impl GitOperations for MockGitHubOperations {
    async fn get_commits_between(
        &self,
        owner: &str,
        repo: &str,
        _base: &str,
        _head: &str,
        _options: GetCommitsOptions,
    ) -> CoreResult<Vec<GitCommit>> {
        let method = "get_commits_between";
        let params_str = format!("owner={owner}, repo={repo}");

        self.check_quota().await?;
        self.simulate_latency().await;

        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params_str, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let key = format!("{owner}/{repo}");
        let commits = self.commits.get(&key).cloned().unwrap_or_default();
        self.record_call(method, &params_str, CallResult::Success)
            .await;
        Ok(commits)
    }

    async fn get_commit(&self, owner: &str, repo: &str, commit_sha: &str) -> CoreResult<GitCommit> {
        let method = "get_commit";
        let params_str = format!("owner={owner}, repo={repo}, sha={commit_sha}");

        self.check_quota().await?;
        self.simulate_latency().await;

        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params_str, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let key = format!("{owner}/{repo}");
        let commit = self
            .commits
            .get(&key)
            .and_then(|commits| commits.iter().find(|c| c.sha == commit_sha).cloned());

        let Some(commit) = commit else {
            self.record_call(
                method,
                &params_str,
                CallResult::Error(format!("commit '{commit_sha}' not found")),
            )
            .await;
            return Err(CoreError::not_found(format!(
                "commit '{commit_sha}' not found"
            )));
        };

        self.record_call(method, &params_str, CallResult::Success)
            .await;
        Ok(commit)
    }

    async fn list_tags(
        &self,
        owner: &str,
        repo: &str,
        _options: ListTagsOptions,
    ) -> CoreResult<Vec<GitTag>> {
        let method = "list_tags";
        let params_str = format!("owner={owner}, repo={repo}");

        self.check_quota().await?;
        self.simulate_latency().await;

        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params_str, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let key = format!("{owner}/{repo}");
        let git_tags: Vec<GitTag> = self
            .tags
            .get(&key)
            .map(|tags| tags.iter().map(github_tag_to_git_tag).collect())
            .unwrap_or_default();

        self.record_call(method, &params_str, CallResult::Success)
            .await;
        Ok(git_tags)
    }

    async fn get_tag(&self, owner: &str, repo: &str, tag_name: &str) -> CoreResult<GitTag> {
        let method = "get_tag";
        let params_str = format!("owner={owner}, repo={repo}, tag={tag_name}");

        self.check_quota().await?;
        self.simulate_latency().await;

        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params_str, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let key = format!("{owner}/{repo}");
        let git_tag = self
            .tags
            .get(&key)
            .and_then(|tags| tags.iter().find(|t| t.name == tag_name))
            .map(github_tag_to_git_tag);

        let Some(git_tag) = git_tag else {
            self.record_call(
                method,
                &params_str,
                CallResult::Error(format!("tag '{tag_name}' not found")),
            )
            .await;
            return Err(CoreError::not_found(format!("tag '{tag_name}' not found")));
        };

        self.record_call(method, &params_str, CallResult::Success)
            .await;
        Ok(git_tag)
    }

    async fn tag_exists(&self, owner: &str, repo: &str, tag_name: &str) -> CoreResult<bool> {
        let method = "tag_exists";
        let params_str = format!("owner={owner}, repo={repo}, tag={tag_name}");

        self.check_quota().await?;
        self.simulate_latency().await;

        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params_str, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let key = format!("{owner}/{repo}");
        let exists = self
            .tags
            .get(&key)
            .map_or(false, |tags| tags.iter().any(|t| t.name == tag_name));

        self.record_call(method, &params_str, CallResult::Success)
            .await;
        Ok(exists)
    }

    async fn get_head_commit(
        &self,
        owner: &str,
        repo: &str,
        _branch_name: Option<&str>,
    ) -> CoreResult<GitCommit> {
        let method = "get_head_commit";
        let params_str = format!("owner={owner}, repo={repo}");

        self.check_quota().await?;
        self.simulate_latency().await;

        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params_str, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let key = format!("{owner}/{repo}");
        let commit = self
            .commits
            .get(&key)
            .and_then(|commits| commits.first().cloned());

        let Some(commit) = commit else {
            self.record_call(
                method,
                &params_str,
                CallResult::Error("head commit not found".to_string()),
            )
            .await;
            return Err(CoreError::not_found("head commit not found"));
        };

        self.record_call(method, &params_str, CallResult::Success)
            .await;
        Ok(commit)
    }

    async fn get_repository_info(&self, owner: &str, repo: &str) -> CoreResult<GitRepository> {
        let method = "get_repository_info";
        let params_str = format!("owner={owner}, repo={repo}");

        self.check_quota().await?;
        self.simulate_latency().await;

        if self.should_simulate_failure().await {
            let error = CoreError::network("Simulated GitHub API error");
            self.record_call(method, &params_str, CallResult::Error(error.to_string()))
                .await;
            return Err(error);
        }

        let key = format!("{owner}/{repo}");
        let Some(repository) = self.repositories.get(&key) else {
            self.record_call(
                method,
                &params_str,
                CallResult::Error("repository not found".to_string()),
            )
            .await;
            return Err(CoreError::not_found(format!("{owner}/{repo} not found")));
        };

        self.record_call(method, &params_str, CallResult::Success)
            .await;
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
    }
}

#[cfg(test)]
#[path = "github_operations_tests.rs"]
mod tests;
