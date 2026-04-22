//! GitHub client implementation using github-bot-sdk
//!
//! This crate provides implementations of [`GitOperations`] and [`GitHubOperations`] traits
//! using the github-bot-sdk library for GitHub API interactions.

use async_trait::async_trait;
use github_bot_sdk::{
    auth::{
        cache::InMemoryTokenCache, tokens::GitHubAppAuth, AuthenticationProvider, InstallationId,
    },
    client::{
        ClientConfig, CreatePullRequestRequest, CreateReleaseRequest, GitHubClient as SdkClient,
        InstallationClient, UpdatePullRequestRequest, UpdateReleaseRequest,
    },
    error::ApiError,
};
use release_regent_core::{
    traits::{
        git_operations::{
            GetCommitsOptions, GitCommit, GitOperations, GitRepository, GitTag, GitTagType,
            GitUser as GitOpsUser, ListTagsOptions, TagSortOrder,
        },
        github_operations::{
            CollaboratorPermission, CreatePullRequestParams, CreateReleaseParams, GitHubOperations,
            GitUser as GitHubUser, Label, PullRequest, PullRequestBranch, Release, Repository, Tag,
            UpdateReleaseParams,
        },
    },
    CoreError, CoreResult,
};
use std::time::Duration as StdDuration;
use tracing::{debug, info, instrument, warn};

pub mod errors;
pub use errors::{Error, GitHubResult};

/// Maximum number of retry attempts for transient GitHub API failures.
///
/// Configured per `docs/specs/design/error-handling.md`: base delay 100 ms,
/// max delay 30 s, ±25 % jitter, **5 max attempts**.
pub(crate) const MAX_RETRIES: u32 = 5;

/// Retry delay (seconds) after a GitHub secondary (abuse-detection) rate limit.
///
/// GitHub recommends waiting at least 60 s before retrying after a secondary
/// rate limit response.
const SECONDARY_RATE_LIMIT_RETRY_SECS: u64 = 60;

pub mod auth;
pub use auth::{AuthConfig, EnvSecretProvider};

// Re-export SDK types for convenience
pub use github_bot_sdk::auth::{GitHubAppId, InstallationId as SdkInstallationId, PrivateKey};

/// GitHub client that implements Release Regent's trait interfaces using github-bot-sdk
#[derive(Clone)]
pub struct GitHubClient {
    sdk_client: SdkClient,
    installation_id: InstallationId,
}

impl GitHubClient {
    /// Create a new GitHub client with authentication provider.
    ///
    /// `installation_id` is the GitHub App installation identifier extracted
    /// from the incoming webhook payload (`installation.id`).  Pass `0` when
    /// constructing the client at startup before any webhook is received; call
    /// [`scoped_to`](GitHubClient::scoped_to) to obtain a per-request client
    /// with the correct installation ID before making any API calls.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::GitHub`] if the underlying SDK client cannot be built.
    #[allow(clippy::result_large_err)]
    pub fn new(
        auth_provider: impl AuthenticationProvider + 'static,
        installation_id: u64,
    ) -> CoreResult<Self> {
        let config = ClientConfig::default()
            .with_user_agent("release-regent/0.1.0")
            .with_timeout(StdDuration::from_secs(30))
            .with_max_retries(MAX_RETRIES);

        let sdk_client = SdkClient::builder(auth_provider)
            .config(config)
            .build()
            .map_err(|e| CoreError::GitHub {
                source: Box::new(e),
                context: None,
            })?;

        Ok(Self {
            sdk_client,
            installation_id: InstallationId::new(installation_id),
        })
    }

    /// Create a new GitHub client directly from [`AuthConfig`].
    ///
    /// The client is constructed without a bound installation ID (ID `0`).
    /// Call [`scoped_to`](GitHubClient::scoped_to) with the installation ID
    /// from each incoming webhook before making any API calls.
    ///
    /// This convenience constructor wires together all required SDK components:
    /// - [`auth::EnvSecretProvider`] for secret retrieval
    /// - [`auth::DefaultJwtSigner`] for RS256 JWT signing
    /// - [`auth::DefaultGitHubApiClient`] for installation token exchange
    /// - An in-memory token cache
    ///
    /// # Errors
    ///
    /// Returns an error if the private key in `auth_config` is malformed or
    /// if the underlying SDK client cannot be initialised.
    #[allow(clippy::result_large_err)]
    pub fn from_config(auth_config: AuthConfig) -> CoreResult<Self> {
        let secret_provider =
            auth::EnvSecretProvider::new(auth_config).map_err(|e| CoreError::GitHub {
                source: Box::new(e),
                context: None,
            })?;

        let jwt_signer = auth::DefaultJwtSigner::new();
        let api_client = auth::DefaultGitHubApiClient::new();
        let token_cache = InMemoryTokenCache::default();
        let auth_config_sdk = github_bot_sdk::auth::tokens::AuthConfig::default();

        let auth_provider = GitHubAppAuth::new(
            secret_provider,
            jwt_signer,
            api_client,
            token_cache,
            auth_config_sdk,
        );

        // Installation ID 0 is a placeholder; the real ID is supplied per-request
        // via `scoped_to()` after extracting it from the webhook payload.
        Self::new(auth_provider, 0)
    }

    /// Get the SDK client for direct access if needed
    #[must_use]
    pub fn sdk_client(&self) -> &SdkClient {
        &self.sdk_client
    }

    /// Create a new GitHub client pointing at a custom API base URL.
    ///
    /// For use in tests only — points the SDK client at a `wiremock::MockServer`
    /// and disables retries so tests fail fast.
    #[cfg(test)]
    pub(crate) fn new_for_testing(
        auth_provider: impl AuthenticationProvider + 'static,
        installation_id: u64,
        api_base_url: &str,
    ) -> CoreResult<Self> {
        let config = ClientConfig::default()
            .with_github_api_url(api_base_url)
            .with_max_retries(0);

        let sdk_client = SdkClient::builder(auth_provider)
            .config(config)
            .build()
            .map_err(|e| CoreError::GitHub {
                source: Box::new(e),
                context: None,
            })?;

        Ok(Self {
            sdk_client,
            installation_id: InstallationId::new(installation_id),
        })
    }

    /// Get an installation client for API operations
    async fn installation(&self) -> CoreResult<InstallationClient> {
        self.sdk_client
            .installation_by_id(self.installation_id)
            .await
            .map_err(|e| CoreError::GitHub {
                source: Box::new(e),
                context: None,
            })
    }
}

#[async_trait]
impl GitOperations for GitHubClient {
    #[instrument(skip(self))]
    async fn get_commits_between(
        &self,
        owner: &str,
        repo: &str,
        base: &str,
        head: &str,
        _options: GetCommitsOptions,
    ) -> CoreResult<Vec<GitCommit>> {
        info!(owner, repo, base, head, "Getting commits between");

        let installation = self.installation().await?;
        let comparison = installation
            .compare_commits(owner, repo, base, head)
            .await
            .map_err(map_sdk_error)?;

        Ok(comparison
            .commits
            .into_iter()
            .map(convert_sdk_commit_to_git_commit)
            .collect())
    }

    #[instrument(skip(self))]
    async fn get_commit(&self, owner: &str, repo: &str, commit_sha: &str) -> CoreResult<GitCommit> {
        info!(owner, repo, commit_sha, "Getting commit");

        let installation = self.installation().await?;
        let sdk_commit = installation
            .get_commit(owner, repo, commit_sha)
            .await
            .map_err(map_sdk_error)?;

        Ok(convert_sdk_commit_to_git_commit(sdk_commit))
    }

    #[instrument(skip(self))]
    async fn list_tags(
        &self,
        owner: &str,
        repo: &str,
        options: ListTagsOptions,
    ) -> CoreResult<Vec<GitTag>> {
        info!(owner, repo, "Listing tags");

        let installation = self.installation().await?;
        let sdk_tags = installation
            .list_tags(owner, repo)
            .await
            .map_err(map_sdk_error)?;

        let mut tags: Vec<GitTag> = sdk_tags
            .into_iter()
            .map(convert_sdk_tag_to_git_tag)
            .collect();

        // Apply sorting if specified
        apply_tag_sorting(&mut tags, options.sort);

        // Apply pagination
        if let Some(offset) = options.offset {
            tags = tags.into_iter().skip(offset).collect();
        }
        if let Some(limit) = options.limit {
            tags.truncate(limit);
        }

        Ok(tags)
    }

    #[instrument(skip(self))]
    async fn get_tag(&self, owner: &str, repo: &str, tag_name: &str) -> CoreResult<GitTag> {
        info!(owner, repo, tag_name, "Getting tag");

        // SDK doesn't have get_tag, so we list all tags and find the one we need
        let installation = self.installation().await?;
        let sdk_tags = installation
            .list_tags(owner, repo)
            .await
            .map_err(map_sdk_error)?;

        sdk_tags
            .into_iter()
            .find(|t| t.name == tag_name)
            .map(convert_sdk_tag_to_git_tag)
            .ok_or_else(|| CoreError::not_found(format!("tag '{tag_name}' not found")))
    }

    #[instrument(skip(self))]
    async fn tag_exists(&self, owner: &str, repo: &str, tag_name: &str) -> CoreResult<bool> {
        debug!(owner, repo, tag_name, "Checking if tag exists");

        match self.get_tag(owner, repo, tag_name).await {
            Ok(_) => Ok(true),
            Err(CoreError::NotFound { .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }

    #[instrument(skip(self))]
    async fn get_head_commit(
        &self,
        owner: &str,
        repo: &str,
        branch: Option<&str>,
    ) -> CoreResult<GitCommit> {
        info!(owner, repo, branch = ?branch, "Getting HEAD commit");

        let installation = self.installation().await?;

        // Get repository to find default branch if not specified
        let branch_name = if let Some(b) = branch {
            b.to_string()
        } else {
            let repo_info = installation
                .get_repository(owner, repo)
                .await
                .map_err(map_sdk_error)?;
            repo_info.default_branch
        };

        // Get the branch to find its commit SHA
        let branch_info = installation
            .get_branch(owner, repo, &branch_name)
            .await
            .map_err(map_sdk_error)?;

        let sdk_commit = installation
            .get_commit(owner, repo, &branch_info.commit.sha)
            .await
            .map_err(map_sdk_error)?;

        Ok(convert_sdk_commit_to_git_commit(sdk_commit))
    }

    #[instrument(skip(self))]
    async fn get_repository_info(&self, owner: &str, repo: &str) -> CoreResult<GitRepository> {
        info!(owner, repo, "Getting repository info");

        let installation = self.installation().await?;
        let sdk_repo = installation
            .get_repository(owner, repo)
            .await
            .map_err(map_sdk_error)?;

        Ok(GitRepository {
            name: sdk_repo.name.clone(),
            owner: owner.to_string(),
            full_name: sdk_repo.full_name.clone(),
            default_branch: sdk_repo.default_branch.clone(),
            clone_url: sdk_repo.clone_url.clone(),
            ssh_url: sdk_repo.ssh_url.clone(),
            private: sdk_repo.private,
            description: sdk_repo.description.clone(),
        })
    }
}

#[async_trait]
impl GitHubOperations for GitHubClient {
    #[instrument(skip(self, params))]
    async fn create_pull_request(
        &self,
        owner: &str,
        repo: &str,
        params: CreatePullRequestParams,
    ) -> CoreResult<PullRequest> {
        info!(owner, repo, title = %params.title, "Creating pull request");

        let installation = self.installation().await?;

        let request = CreatePullRequestRequest {
            title: params.title,
            head: params.head,
            base: params.base,
            body: params.body,
            draft: Some(params.draft),
            maintainer_can_modify: Some(params.maintainer_can_modify),
            milestone: None,
        };

        let sdk_pr = installation
            .create_pull_request(owner, repo, request)
            .await
            .map_err(map_sdk_error)?;

        convert_sdk_pr_to_release_regent_pr(sdk_pr)
    }

    #[instrument(skip(self, params))]
    async fn create_release(
        &self,
        owner: &str,
        repo: &str,
        params: CreateReleaseParams,
    ) -> CoreResult<Release> {
        info!(owner, repo, tag_name = %params.tag_name, "Creating release");

        let installation = self.installation().await?;

        let request = CreateReleaseRequest {
            tag_name: params.tag_name,
            target_commitish: params.target_commitish,
            name: params.name,
            body: params.body,
            draft: Some(params.draft),
            prerelease: Some(params.prerelease),
            generate_release_notes: Some(params.generate_release_notes),
        };

        let sdk_release = installation
            .create_release(owner, repo, request)
            .await
            .map_err(map_sdk_error)?;

        Ok(convert_sdk_release_to_release_regent_release(sdk_release))
    }

    #[instrument(skip(self))]
    async fn create_tag(
        &self,
        owner: &str,
        repo: &str,
        tag_name: &str,
        commit_sha: &str,
        message: Option<String>,
        tagger: Option<GitHubUser>,
    ) -> CoreResult<Tag> {
        info!(owner, repo, tag_name, commit_sha, "Creating tag");

        let installation = self.installation().await?;

        // Use the SDK's create_tag method
        let _sdk_tag = installation
            .create_tag(owner, repo, tag_name, commit_sha)
            .await
            .map_err(map_sdk_error)?;

        Ok(Tag {
            name: tag_name.to_string(),
            commit_sha: commit_sha.to_string(),
            message,
            tagger,
            created_at: None,
        })
    }

    #[instrument(skip(self))]
    async fn get_latest_release(&self, owner: &str, repo: &str) -> CoreResult<Option<Release>> {
        info!(owner, repo, "Getting latest release");

        let installation = self.installation().await?;

        match installation.get_latest_release(owner, repo).await {
            Ok(sdk_release) => Ok(Some(convert_sdk_release_to_release_regent_release(
                sdk_release,
            ))),
            Err(e) if is_not_found_error(&e) => Ok(None),
            Err(e) => Err(map_sdk_error(e)),
        }
    }

    #[instrument(skip(self))]
    async fn get_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> CoreResult<PullRequest> {
        info!(owner, repo, pr_number, "Getting pull request");

        let installation = self.installation().await?;
        let sdk_pr = installation
            .get_pull_request(owner, repo, pr_number)
            .await
            .map_err(map_sdk_error)?;

        convert_sdk_pr_to_release_regent_pr(sdk_pr)
    }

    #[instrument(skip(self))]
    async fn get_release_by_tag(&self, owner: &str, repo: &str, tag: &str) -> CoreResult<Release> {
        info!(owner, repo, tag, "Getting release by tag");

        let installation = self.installation().await?;
        let sdk_release = installation
            .get_release_by_tag(owner, repo, tag)
            .await
            .map_err(map_sdk_error)?;

        Ok(convert_sdk_release_to_release_regent_release(sdk_release))
    }

    #[instrument(skip(self))]
    async fn list_releases(
        &self,
        owner: &str,
        repo: &str,
        _per_page: Option<u8>,
        _page: Option<u32>,
    ) -> CoreResult<Vec<Release>> {
        info!(owner, repo, "Listing releases");

        let installation = self.installation().await?;
        let sdk_releases = installation
            .list_releases(owner, repo)
            .await
            .map_err(map_sdk_error)?;

        Ok(sdk_releases
            .into_iter()
            .map(convert_sdk_release_to_release_regent_release)
            .collect())
    }

    #[instrument(skip(self))]
    async fn update_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        title: Option<String>,
        body: Option<String>,
        state: Option<String>,
    ) -> CoreResult<PullRequest> {
        info!(owner, repo, pr_number, "Updating pull request");

        let installation = self.installation().await?;

        let request = UpdatePullRequestRequest {
            title,
            body,
            state,
            base: None,
            milestone: None,
        };

        let sdk_pr = installation
            .update_pull_request(owner, repo, pr_number, request)
            .await
            .map_err(map_sdk_error)?;

        convert_sdk_pr_to_release_regent_pr(sdk_pr)
    }

    /// Note: `per_page` and `page` parameters are ignored — this implementation
    /// always fetches all pages and returns a complete result set.
    #[instrument(skip(self))]
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
        info!(
            owner,
            repo,
            state = state.unwrap_or("open"),
            "Listing pull requests"
        );

        let state_str = state.unwrap_or("open");
        let installation = self.installation().await?;
        let mut all_prs: Vec<PullRequest> = Vec::new();
        let mut page: Option<u32> = None;

        loop {
            let response = installation
                .list_pull_requests(owner, repo, Some(state_str), page)
                .await
                .map_err(map_sdk_error)?;

            let has_next = response.has_next();
            let next_page_num = response.next_page_number();

            for sdk_pr in response.items {
                // Client-side head-branch prefix filter.
                if let Some(prefix) = head {
                    if !sdk_pr.head.branch_ref.starts_with(prefix) {
                        continue;
                    }
                }
                // Client-side base-branch exact-match filter.
                if let Some(base_branch) = base {
                    if sdk_pr.base.branch_ref != base_branch {
                        continue;
                    }
                }
                all_prs.push(convert_sdk_pr_to_release_regent_pr(sdk_pr)?);
            }

            match (has_next, next_page_num) {
                (true, Some(next)) => page = Some(next),
                _ => break,
            }
        }

        debug!(
            owner,
            repo,
            state = state_str,
            count = all_prs.len(),
            "list_pull_requests complete"
        );
        Ok(all_prs)
    }

    #[instrument(skip(self))]
    async fn search_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        query: &str,
    ) -> CoreResult<Vec<PullRequest>> {
        info!(owner, repo, query, "Searching pull requests");

        // Determine desired state from the query.  Default to "open" if
        // not specified, which matches the most common usage pattern.
        let state = if query.contains("is:closed") {
            "closed"
        } else if query.contains("is:all") {
            "all"
        } else {
            "open"
        };

        // Extract the head-branch prefix filter, e.g. `head:release/v*` → `"release/v"`.
        // The trailing `*` is a glob wildcard; we strip it and use starts_with matching.
        let head_prefix: Option<&str> = query
            .split_whitespace()
            .find_map(|token| token.strip_prefix("head:").map(|p| p.trim_end_matches('*')));

        let installation = self.installation().await?;
        let mut all_prs: Vec<PullRequest> = Vec::new();
        let mut page: Option<u32> = None;

        loop {
            let response = installation
                .list_pull_requests(owner, repo, Some(state), page)
                .await
                .map_err(map_sdk_error)?;

            let has_next = response.has_next();
            let next_page_num = response.next_page_number();

            for sdk_pr in response.items {
                // Filter by head branch prefix when specified.
                if let Some(prefix) = head_prefix {
                    if !sdk_pr.head.branch_ref.starts_with(prefix) {
                        continue;
                    }
                }
                all_prs.push(convert_sdk_pr_to_release_regent_pr(sdk_pr)?);
            }

            match (has_next, next_page_num) {
                (true, Some(next)) => page = Some(next),
                _ => break,
            }
        }

        debug!(
            owner,
            repo,
            query,
            count = all_prs.len(),
            "search_pull_requests complete"
        );
        Ok(all_prs)
    }

    #[instrument(skip(self, params))]
    async fn update_release(
        &self,
        owner: &str,
        repo: &str,
        release_id: u64,
        params: UpdateReleaseParams,
    ) -> CoreResult<Release> {
        info!(owner, repo, release_id, "Updating release");

        let installation = self.installation().await?;

        let request = UpdateReleaseRequest {
            tag_name: None,
            target_commitish: None,
            name: params.name,
            body: params.body,
            draft: params.draft,
            prerelease: params.prerelease,
        };

        let sdk_release = installation
            .update_release(owner, repo, release_id, request)
            .await
            .map_err(map_sdk_error)?;

        Ok(convert_sdk_release_to_release_regent_release(sdk_release))
    }

    #[instrument(skip(self))]
    async fn create_branch(
        &self,
        owner: &str,
        repo: &str,
        branch_name: &str,
        sha: &str,
    ) -> CoreResult<()> {
        info!(owner, repo, branch_name, sha, "Creating branch");

        let installation = self.installation().await?;

        installation
            .create_branch(owner, repo, branch_name, sha)
            .await
            .map_err(|e| {
                // HTTP 422 from GitHub means "Reference already exists"
                if let github_bot_sdk::error::ApiError::HttpError { status: 422, .. } = &e {
                    CoreError::conflict(format!("branch '{branch_name}' already exists"))
                } else {
                    map_sdk_error(e)
                }
            })?;

        Ok(())
    }

    #[instrument(skip(self))]
    async fn delete_branch(&self, owner: &str, repo: &str, branch_name: &str) -> CoreResult<()> {
        info!(owner, repo, branch_name, "Deleting branch");

        let installation = self.installation().await?;

        installation
            .delete_git_ref(owner, repo, &format!("heads/{branch_name}"))
            .await
            .map_err(map_sdk_error)?;

        Ok(())
    }

    #[instrument(skip(self, body))]
    async fn create_issue_comment(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        body: &str,
    ) -> CoreResult<()> {
        info!(owner, repo, issue_number, "Creating issue comment");

        let installation = self.installation().await?;
        let request = github_bot_sdk::client::CreateCommentRequest {
            body: body.to_string(),
        };

        installation
            .create_issue_comment(owner, repo, issue_number, request)
            .await
            .map_err(map_sdk_error)?;

        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_collaborator_permission(
        &self,
        owner: &str,
        repo: &str,
        username: &str,
    ) -> CoreResult<CollaboratorPermission> {
        use release_regent_core::traits::github_operations::CollaboratorPermission;

        info!(owner, repo, username, "Checking collaborator permission");

        let installation = self.installation().await?;
        let path = format!("/repos/{owner}/{repo}/collaborators/{username}/permission");
        let response = installation.get(&path).await.map_err(map_sdk_error)?;
        let body: serde_json::Value = response.json().await.map_err(CoreError::github)?;

        let permission_str = body
            .get("permission")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("none");

        Ok(match permission_str {
            "admin" => CollaboratorPermission::Admin,
            "maintain" => CollaboratorPermission::Maintain,
            "write" => CollaboratorPermission::Write,
            "triage" => CollaboratorPermission::Triage,
            "read" => CollaboratorPermission::Read,
            "none" => CollaboratorPermission::None,
            other => {
                warn!(
                    owner,
                    repo,
                    username,
                    permission = other,
                    "Unrecognised GitHub collaborator permission string — \
                     treating as None; if this is a new GitHub permission level \
                     the CollaboratorPermission enum must be updated"
                );
                CollaboratorPermission::None
            }
        })
    }

    #[instrument(skip(self, labels))]
    async fn add_labels(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        labels: &[&str],
    ) -> CoreResult<()> {
        info!(owner, repo, issue_number, "Adding labels");

        let installation = self.installation().await?;
        let path = format!("/repos/{owner}/{repo}/issues/{issue_number}/labels");
        let body = serde_json::json!({ "labels": labels });
        installation
            .post(&path, &body)
            .await
            .map_err(map_sdk_error)?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn remove_label(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        label_name: &str,
    ) -> CoreResult<()> {
        info!(
            owner,
            repo,
            issue_number,
            label = label_name,
            "Removing label"
        );

        let installation = self.installation().await?;
        // GitHub returns 404 when the label is not on the issue; treat that as Ok.
        // ASSUMPTION: label names in this codebase match [a-zA-Z0-9:_.-]+ (e.g.
        // "rr:override-minor").  Only colons are percent-encoded here; spaces and
        // other characters are intentionally not handled because Release Regent's
        // own labels never contain them.
        let encoded = label_name.replace(':', "%3A");
        let path = format!("/repos/{owner}/{repo}/issues/{issue_number}/labels/{encoded}");
        match installation.delete(&path).await {
            Ok(_) | Err(github_bot_sdk::error::ApiError::HttpError { status: 404, .. }) => Ok(()),
            Err(e) => Err(map_sdk_error(e)),
        }
    }

    #[instrument(skip(self))]
    async fn list_pr_labels(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
    ) -> CoreResult<Vec<Label>> {
        info!(owner, repo, issue_number, "Listing PR labels");

        let installation = self.installation().await?;
        let path = format!("/repos/{owner}/{repo}/issues/{issue_number}/labels");
        let response = installation.get(&path).await.map_err(map_sdk_error)?;
        let raw: Vec<serde_json::Value> = response.json().await.map_err(CoreError::github)?;

        let labels = raw
            .into_iter()
            .map(|v| Label {
                id: v["id"].as_u64().unwrap_or(0),
                name: v["name"].as_str().unwrap_or("").to_string(),
                color: v["color"].as_str().unwrap_or("").to_string(),
                description: v["description"].as_str().map(str::to_string),
            })
            .collect();

        Ok(labels)
    }

    fn scoped_to(&self, installation_id: u64) -> Self {
        Self {
            sdk_client: self.sdk_client.clone(),
            installation_id: InstallationId::new(installation_id),
        }
    }
}

// ============================================================================
// Type conversion utilities
// ============================================================================

// Note: Placeholder for when SDK has commit support
// Currently SDK's Tag type only has { sha, url } not full commit details
#[allow(dead_code)]
fn convert_sdk_commit_to_git_commit(commit: github_bot_sdk::client::FullCommit) -> GitCommit {
    let message = commit.commit.message.clone();
    let subject = message.lines().next().unwrap_or("").to_string();
    // Body is everything after the subject line and optional blank line
    let body_start: String = message.lines().skip(1).collect::<Vec<&str>>().join("\n");
    let body = {
        let trimmed = body_start.trim_start_matches('\n');
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    };

    GitCommit {
        sha: commit.sha,
        author: GitOpsUser {
            name: commit.commit.author.name,
            email: commit.commit.author.email,
            username: commit.author.map(|u| u.login),
        },
        committer: GitOpsUser {
            name: commit.commit.committer.name,
            email: commit.commit.committer.email,
            username: commit.committer.map(|u| u.login),
        },
        author_date: commit.commit.author.date,
        commit_date: commit.commit.committer.date,
        message,
        subject,
        body,
        parents: commit.parents.into_iter().map(|p| p.sha).collect(),
        files: vec![], // FullCommit doesn't include file-level diff; use compare_commits if needed
    }
}

fn convert_sdk_tag_to_git_tag(tag: github_bot_sdk::client::Tag) -> GitTag {
    GitTag {
        name: tag.name,
        target_sha: tag.commit.sha,
        tag_type: GitTagType::Lightweight, // SDK doesn't distinguish
        message: None,
        tagger: None,
        created_at: None,
    }
}

fn convert_sdk_release_to_release_regent_release(
    release: github_bot_sdk::client::Release,
) -> Release {
    Release {
        id: release.id,
        tag_name: release.tag_name,
        target_commitish: release.target_commitish,
        name: release.name,
        body: release.body,
        draft: release.draft,
        prerelease: release.prerelease,
        created_at: release.created_at,
        published_at: release.published_at,
        author: GitHubUser {
            name: release.author.login.clone(),
            email: format!("{}@users.noreply.github.com", release.author.login),
            login: Some(release.author.login.clone()),
        },
    }
}

#[allow(clippy::unnecessary_wraps)]
#[allow(clippy::result_large_err)]
fn convert_sdk_pr_to_release_regent_pr(
    pr: github_bot_sdk::client::PullRequest,
) -> CoreResult<PullRequest> {
    // Extract owner from full_name since PullRequestRepo doesn't have owner field
    let head_owner = pr
        .head
        .repo
        .full_name
        .split('/')
        .next()
        .unwrap_or("unknown")
        .to_string();
    let base_owner = pr
        .base
        .repo
        .full_name
        .split('/')
        .next()
        .unwrap_or("unknown")
        .to_string();

    Ok(PullRequest {
        number: pr.number,
        title: pr.title,
        body: pr.body,
        state: pr.state,
        draft: pr.draft,
        created_at: pr.created_at,
        updated_at: pr.updated_at,
        merged_at: pr.merged_at,
        user: GitHubUser {
            name: pr.user.login.clone(),
            email: format!("{}@users.noreply.github.com", pr.user.login),
            login: Some(pr.user.login.clone()),
        },
        head: PullRequestBranch {
            ref_name: pr.head.branch_ref.clone(),
            sha: pr.head.sha.clone(),
            repo: Repository {
                id: pr.head.repo.id,
                name: pr.head.repo.name.clone(),
                full_name: pr.head.repo.full_name.clone(),
                owner: head_owner,
                description: None,
                private: false,
                default_branch: String::new(),
                clone_url: String::new(),
                ssh_url: String::new(),
                homepage: None,
            },
        },
        base: PullRequestBranch {
            ref_name: pr.base.branch_ref.clone(),
            sha: pr.base.sha.clone(),
            repo: Repository {
                id: pr.base.repo.id,
                name: pr.base.repo.name.clone(),
                full_name: pr.base.repo.full_name.clone(),
                owner: base_owner,
                description: None,
                private: false,
                default_branch: String::new(),
                clone_url: String::new(),
                ssh_url: String::new(),
                homepage: None,
            },
        },
    })
}

#[allow(clippy::needless_pass_by_value)]
fn apply_tag_sorting(tags: &mut [GitTag], sort: TagSortOrder) {
    match sort {
        TagSortOrder::NameAsc => tags.sort_by(|a, b| a.name.cmp(&b.name)),
        TagSortOrder::NameDesc => tags.sort_by(|a, b| b.name.cmp(&a.name)),
        TagSortOrder::CreationDateAsc => tags.sort_by(|a, b| a.created_at.cmp(&b.created_at)),
        TagSortOrder::CreationDateDesc => tags.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
        TagSortOrder::SemanticVersionAsc | TagSortOrder::SemanticVersionDesc => {
            // Semantic version sorting would require parsing
            // For now, fall back to name sorting
            tags.sort_by(|a, b| a.name.cmp(&b.name));
            if matches!(sort, TagSortOrder::SemanticVersionDesc) {
                tags.reverse();
            }
        }
    }
}

/// Map an SDK `ApiError` to the most semantically accurate `CoreError` variant.
///
/// Correct mapping ensures that `CoreError::is_retryable()` returns `true` for
/// transient server faults (5xx, rate limits, timeouts, network) and `false` for
/// permanent client errors (4xx auth/validation failures).
#[allow(clippy::match_same_arms)] // explicit arms are intentional for clarity
fn map_sdk_error(error: ApiError) -> CoreError {
    match error {
        // ── Permanent: resource not found ───────────────────────────────────
        ApiError::NotFound => CoreError::not_found("GitHub resource not found"),

        // ── Permanent: authentication / authorisation failures ───────────────
        ApiError::AuthenticationFailed => {
            CoreError::authentication("GitHub API authentication failed (401)")
        }
        ApiError::AuthorizationFailed => {
            CoreError::authentication("GitHub API authorisation failed (403)")
        }

        // ── Transient: rate limiting ─────────────────────────────────────────
        ApiError::RateLimitExceeded { reset_at } => {
            // Compute seconds until the rate limit resets; floor at 1 s.
            let retry_after = {
                let secs = (reset_at - chrono::Utc::now()).num_seconds();
                u64::try_from(secs).unwrap_or(1).max(1)
            };
            CoreError::rate_limit_with_retry("GitHub primary rate limit exceeded", retry_after)
        }
        ApiError::SecondaryRateLimit => CoreError::rate_limit_with_retry(
            "GitHub secondary rate limit (abuse detection) exceeded",
            SECONDARY_RATE_LIMIT_RETRY_SECS,
        ),

        // ── Transient: request timed out ─────────────────────────────────────
        ApiError::Timeout => CoreError::timeout("GitHub API request", 30_000),

        // ── Mixed: HTTP status-code based classification ─────────────────────
        ApiError::HttpError { status, message } => match status {
            // Rate-limit via 429 (when the SDK returns HttpError instead of RateLimitExceeded)
            429 => CoreError::rate_limit_with_retry(format!("GitHub rate limit: {message}"), 60),
            // Auth failures
            401 => CoreError::authentication(format!("GitHub 401: {message}")),
            403 => CoreError::authentication(format!("GitHub 403: {message}")),
            // Not found
            404 => CoreError::not_found(format!("GitHub 404: {message}")),
            // Server errors → transient/retryable
            s if s >= 500 => CoreError::network(format!("GitHub server error {s}: {message}")),
            // Everything else (4xx validation errors) → permanent
            _ => CoreError::GitHub {
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("GitHub HTTP {status}: {message}"),
                )),
                context: None,
            },
        },

        // ── Transient: network / transport failures ──────────────────────────
        ApiError::HttpClientError(e) => {
            CoreError::network(format!("GitHub HTTP client error: {e}"))
        }

        // ── Permanent: client mistakes / config errors ───────────────────────
        ApiError::InvalidRequest { message } => CoreError::GitHub {
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("GitHub invalid request: {message}"),
            )),
            context: None,
        },
        ApiError::Configuration { message } => {
            CoreError::config(format!("GitHub client configuration error: {message}"))
        }
        ApiError::TokenGenerationFailed { message } => {
            CoreError::authentication(format!("GitHub token generation failed: {message}"))
        }
        ApiError::TokenExchangeFailed { message } => {
            CoreError::authentication(format!("GitHub token exchange failed: {message}"))
        }

        // ── Permanent: JSON parsing error ────────────────────────────────────
        ApiError::JsonError(e) => CoreError::github(e),
    }
}

fn is_not_found_error(error: &ApiError) -> bool {
    matches!(
        error,
        ApiError::NotFound | ApiError::HttpError { status: 404, .. }
    )
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;

#[cfg(test)]
#[path = "models_tests.rs"]
mod models_tests;

#[cfg(test)]
#[path = "release_tests.rs"]
mod release_tests;

#[cfg(test)]
#[path = "pr_management_tests.rs"]
mod pr_management_tests;
