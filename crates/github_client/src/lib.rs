//! Crate for interacting with the GitHub REST API.
//!
//! This crate provides a client for making authenticated requests to GitHub,
//! authenticating as a GitHub App using its ID and private key.

use async_trait::async_trait;
use octocrab::{Octocrab, Result as OctocrabResult};
use release_regent_core::{traits::github_operations::*, CoreError, CoreResult, GitHubOperations};
use tracing::{debug, error, info, instrument};

pub mod errors;
pub use errors::{Error, GitHubResult};
// For backward compatibility
pub use errors::Error as GitHubError;

pub mod auth;
pub use auth::{
    authenticate_with_access_token, create_app_client, create_token_client, AuthConfig,
    GitHubAuthManager,
};

pub mod models;

pub mod pr_management;
pub mod release;

/// A client for interacting with the GitHub API, authenticated as a GitHub App.
///
/// This struct provides a high-level interface for GitHub API operations using
/// GitHub App authentication. It wraps an Octocrab client and provides methods
/// for repository management, installation token retrieval, and organization queries.
///
/// # Examples
///
/// ```rust,no_run
/// use release_regent_github_client::{GitHubClient, create_app_client};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let app_id = 123456;
///     let private_key = "-----BEGIN RSA PRIVATE KEY-----\n...\n-----END RSA PRIVATE KEY-----";
///
///     let octocrab_client = create_app_client(app_id, private_key).await?;
///     let github_client = GitHubClient::new(octocrab_client);
///
///     let installations = github_client.list_installations().await?;
///     println!("Found {} installations", installations.len());
///
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct GitHubClient {
    /// The underlying Octocrab client used for API requests
    client: Octocrab,
    /// Optional authentication manager for advanced token management
    auth_manager: Option<GitHubAuthManager>,
}

impl GitHubClient {
    /// Creates an installation client for the specified installation ID.
    ///
    /// This method creates a new GitHubClient instance that is authenticated with
    /// an installation token for the specified installation ID. If an authentication
    /// manager is available, it will use token caching for better performance.
    ///
    /// # Arguments
    ///
    /// * `installation_id` - The GitHub App installation ID
    ///
    /// # Returns
    ///
    /// Returns a new `GitHubClient` instance authenticated for the installation.
    ///
    /// # Errors
    ///
    /// Returns an error if no authentication manager is available or if the
    /// installation token cannot be acquired.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use release_regent_github_client::{GitHubClient, AuthConfig, GitHubAuthManager};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = AuthConfig::new(123456, "private_key", None)?;
    ///     let auth_manager = GitHubAuthManager::new(config)?;
    ///     let github_client = GitHubClient::with_auth_manager(auth_manager).await?;
    ///     let installation_client = github_client.create_installation_client(987654).await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn create_installation_client(&self, installation_id: u64) -> GitHubResult<Self> {
        let auth_manager = self.auth_manager.as_ref().ok_or_else(|| {
            Error::configuration("auth_manager", "Authentication manager not available")
        })?;

        let client = auth_manager
            .create_installation_client(installation_id)
            .await?;
        Ok(Self {
            client,
            auth_manager: Some(auth_manager.clone()),
        })
    }

    /// Fetches details for a specific repository.
    ///
    /// # Arguments
    ///
    /// * `owner` - The owner of the repository (user or organization name).
    /// * `repo` - The name of the repository.
    ///
    /// # Errors
    /// Returns an `Error::Octocrab` if the API call fails.
    #[instrument(skip(self), fields(owner = %owner, repo = %repo))]
    pub async fn get_repository(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<models::Repository, Error> {
        let result = self.client.repos(owner, repo).get().await;
        match result {
            Ok(r) => Ok(models::Repository::from(r)),
            Err(e) => {
                log_octocrab_error("Failed to get repository", e);
                return Err(Error::InvalidResponse);
            }
        }
    }

    /// Lists all installations for the authenticated GitHub App.
    ///
    /// This method retrieves all installations where the GitHub App is installed,
    /// which can be used to find the installation ID for a specific organization.
    ///
    /// # Returns
    ///
    /// A `Result` containing a vector of installation objects, or an error if the
    /// operation fails.
    ///
    /// # Errors
    ///
    /// Returns an `Error::InvalidResponse` if the API call fails or the response
    /// cannot be parsed.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use release_regent_github_client::{GitHubClient, create_app_client};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// #     let app_id = 123456;
    /// #     let private_key = "-----BEGIN RSA PRIVATE KEY-----\n...\n-----END RSA PRIVATE KEY-----";
    /// #     let client_octocrab = create_app_client(app_id, private_key).await?;
    /// #     let client = GitHubClient::new(client_octocrab);
    ///
    ///     let installations = client.list_installations().await?;
    ///     for installation in installations {
    ///         println!("Installation ID: {}, Account: {}", installation.id, installation.account.login);
    ///     }
    ///
    /// #     Ok(())
    /// # }
    /// ```
    #[instrument(skip(self))]
    pub async fn list_installations(&self) -> Result<Vec<models::Installation>, Error> {
        info!("Listing installations for GitHub App using JWT authentication");

        // Use direct REST API call instead of octocrab's high-level method
        let result: OctocrabResult<Vec<octocrab::models::Installation>> =
            self.client.get("/app/installations", None::<&()>).await;

        match result {
            Ok(installations) => {
                let converted_installations: Vec<models::Installation> = installations
                    .into_iter()
                    .map(models::Installation::from)
                    .collect();

                info!(
                    count = converted_installations.len(),
                    "Successfully retrieved installations for GitHub App"
                );

                Ok(converted_installations)
            }
            Err(e) => {
                error!(
                    "Failed to list installations - this likely means JWT authentication failed"
                );
                log_octocrab_error("Failed to list installations", e);
                Err(Error::InvalidResponse)
            }
        }
    }

    /// Creates a new `GitHubClient` instance with the provided Octocrab client.
    ///
    /// This constructor wraps an existing Octocrab client that should already be
    /// configured with appropriate authentication (typically GitHub App JWT).
    ///
    /// # Arguments
    ///
    /// * `client` - An authenticated Octocrab client instance
    ///
    /// # Returns
    ///
    /// Returns a new `GitHubClient` instance ready for API operations.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use release_regent_github_client::{GitHubClient, create_app_client};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let app_id = 123456;
    ///     let private_key = "-----BEGIN RSA PRIVATE KEY-----\n...\n-----END RSA PRIVATE KEY-----";
    ///
    ///     let octocrab_client = create_app_client(app_id, private_key).await?;
    ///     let github_client = GitHubClient::new(octocrab_client);
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn new(client: Octocrab) -> Self {
        Self {
            client,
            auth_manager: None,
        }
    }

    /// Creates a new `GitHubClient` instance with the provided authentication manager.
    ///
    /// This constructor creates a GitHubClient with an integrated authentication manager
    /// that provides advanced token management features including caching, rate limiting,
    /// and automatic token refresh.
    ///
    /// # Arguments
    ///
    /// * `auth_manager` - A GitHubAuthManager instance configured with GitHub App credentials
    ///
    /// # Returns
    ///
    /// Returns a new `GitHubClient` instance with integrated authentication management.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use release_regent_github_client::{GitHubClient, AuthConfig, GitHubAuthManager};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = AuthConfig::new(123456, "private_key", None)?;
    ///     let auth_manager = GitHubAuthManager::new(config)?;
    ///     let github_client = GitHubClient::with_auth_manager(auth_manager).await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn with_auth_manager(auth_manager: GitHubAuthManager) -> GitHubResult<Self> {
        let client = auth_manager.create_app_client().await?;
        Ok(Self {
            client,
            auth_manager: Some(auth_manager),
        })
    }
}

#[async_trait]
impl GitHubOperations for GitHubClient {
    /// Get commits between two references
    ///
    /// # Parameters
    /// - `owner`: Repository owner name
    /// - `repo`: Repository name
    /// - `base`: Base reference (commit SHA, branch, or tag)
    /// - `head`: Head reference (commit SHA, branch, or tag)
    /// - `per_page`: Number of commits per page (max 250)
    /// - `page`: Page number to retrieve (1-based)
    ///
    /// # Returns
    /// List of commits between base and head, ordered chronologically
    ///
    /// # Errors
    /// - `CoreError::GitHub` - API communication failed
    /// - `CoreError::InvalidInput` - Invalid references or pagination
    /// - `CoreError::NotSupported` - References not found
    async fn compare_commits(
        &self,
        owner: &str,
        repo: &str,
        base: &str,
        head: &str,
        per_page: Option<u8>,
        page: Option<u32>,
    ) -> CoreResult<Vec<Commit>> {
        debug!(
            "Comparing commits from {} to {} for {}/{}",
            base, head, owner, repo
        );

        // For now, implement a simple version that gets commits from head
        // In a full implementation, this would use the GitHub compare API
        let repos = self.client.repos(owner, repo);
        let commits = repos.list_commits();
        let result = commits.sha(head).send().await;
        match result {
            Ok(commits_page) => {
                let commits: Vec<Commit> = commits_page
                    .items
                    .into_iter()
                    .take(10) // Limit to avoid too many results
                    .map(|commit| {
                        let author = commit
                            .commit
                            .author
                            .as_ref()
                            .map(|a| GitUser {
                                name: a.user.name.clone(),
                                email: a.user.email.clone(),
                                login: commit.author.as_ref().map(|u| u.login.clone()),
                            })
                            .unwrap_or_else(|| GitUser {
                                name: "Unknown".to_string(),
                                email: "unknown@example.com".to_string(),
                                login: None,
                            });

                        let committer = commit
                            .commit
                            .committer
                            .as_ref()
                            .map(|c| GitUser {
                                name: c.user.name.clone(),
                                email: c.user.email.clone(),
                                login: commit.committer.as_ref().map(|u| u.login.clone()),
                            })
                            .unwrap_or_else(|| GitUser {
                                name: "Unknown".to_string(),
                                email: "unknown@example.com".to_string(),
                                login: None,
                            });

                        let parents: Vec<String> =
                            commit.parents.into_iter().filter_map(|p| p.sha).collect();

                        Commit {
                            sha: commit.sha,
                            message: commit.commit.message,
                            author,
                            committer,
                            date: commit
                                .commit
                                .committer
                                .and_then(|c| c.date)
                                .unwrap_or_else(chrono::Utc::now),
                            parents,
                        }
                    })
                    .collect();

                debug!(
                    "Found {} commits for comparison between {} and {} for {}/{}",
                    commits.len(),
                    base,
                    head,
                    owner,
                    repo
                );
                Ok(commits)
            }
            Err(e) => {
                error!(
                    "Failed to compare commits from {} to {} for {}/{}: {}",
                    base, head, owner, repo, e
                );
                Err(CoreError::github(crate::Error::from(e)))
            }
        }
    }

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
        // TODO: Implement using octocrab pulls API - requires careful field access pattern handling
        Err(CoreError::not_supported(
            "create_pull_request",
            "Complex octocrab field access patterns - will be implemented in Phase 3",
        ))
    }

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
    ) -> CoreResult<Release> {
        debug!(
            "Creating release '{}' for tag {} in {}/{}",
            params.name.as_ref().unwrap_or(&params.tag_name),
            params.tag_name,
            owner,
            repo
        );

        let release_create_params = serde_json::json!({
            "tag_name": params.tag_name,
            "target_commitish": params.target_commitish,
            "name": params.name,
            "body": params.body,
            "draft": params.draft,
            "prerelease": params.prerelease,
            "generate_release_notes": params.generate_release_notes
        });

        let result: Result<octocrab::models::repos::Release, _> = self
            .client
            .post(
                &format!("/repos/{}/{}/releases", owner, repo),
                Some(&release_create_params),
            )
            .await;

        match result {
            Ok(release) => {
                let release = Release {
                    id: release.id.0,
                    tag_name: release.tag_name,
                    name: release.name,
                    body: release.body,
                    draft: release.draft,
                    prerelease: release.prerelease,
                    created_at: release.created_at.unwrap_or_else(chrono::Utc::now),
                    published_at: release.published_at,
                    target_commitish: release.target_commitish,
                    author: release
                        .author
                        .map(|a| GitUser {
                            name: a.login.clone(),
                            email: a.email.unwrap_or_default(),
                            login: Some(a.login),
                        })
                        .unwrap_or_else(|| GitUser {
                            name: "Unknown".to_string(),
                            email: "unknown@example.com".to_string(),
                            login: None,
                        }),
                };

                debug!(
                    "Created release '{}' (id: {}) for tag {} in {}/{}",
                    release.name.as_ref().unwrap_or(&release.tag_name),
                    release.id,
                    params.tag_name,
                    owner,
                    repo
                );
                Ok(release)
            }
            Err(e) => {
                error!(
                    "Failed to create release '{}' for tag {} in {}/{}: {}",
                    params.name.as_ref().unwrap_or(&params.tag_name),
                    params.tag_name,
                    owner,
                    repo,
                    e
                );
                Err(CoreError::github(crate::Error::from(e)))
            }
        }
    }

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
    ) -> CoreResult<Tag> {
        debug!(
            "Creating tag {} for commit {} in {}/{}",
            tag_name, commit_sha, owner, repo
        );

        // For lightweight tags, we create a reference directly
        let tag_ref = format!("refs/tags/{}", tag_name);

        let tag_create_params = serde_json::json!({
            "ref": tag_ref,
            "sha": commit_sha
        });

        let result: Result<serde_json::Value, _> = self
            .client
            .post(
                &format!("/repos/{}/{}/git/refs", owner, repo),
                Some(&tag_create_params),
            )
            .await;

        match result {
            Ok(_) => {
                let tag = Tag {
                    name: tag_name.to_string(),
                    commit_sha: commit_sha.to_string(),
                    created_at: Some(chrono::Utc::now()),
                    message,
                    tagger,
                };

                debug!(
                    "Created tag {} for commit {} in {}/{}",
                    tag_name, commit_sha, owner, repo
                );
                Ok(tag)
            }
            Err(e) => {
                error!(
                    "Failed to create tag {} for commit {} in {}/{}: {}",
                    tag_name, commit_sha, owner, repo, e
                );
                Err(CoreError::github(crate::Error::from(e)))
            }
        }
    }

    /// Get specific commit information
    ///
    /// # Parameters
    /// - `owner`: Repository owner name
    /// - `repo`: Repository name
    /// - `commit_sha`: Commit SHA to retrieve
    ///
    /// # Returns
    /// Detailed commit information including author, message, and metadata
    ///
    /// # Errors
    /// - `CoreError::GitHub` - API communication failed
    /// - `CoreError::InvalidInput` - Invalid commit SHA format
    /// - `CoreError::NotSupported` - Commit not found
    async fn get_commit(&self, owner: &str, repo: &str, commit_sha: &str) -> CoreResult<Commit> {
        debug!("Getting commit {} for {}/{}", commit_sha, owner, repo);

        let result = self.client.commits(owner, repo).get(commit_sha).await;
        match result {
            Ok(commit) => {
                let author = commit
                    .commit
                    .author
                    .as_ref()
                    .map(|a| GitUser {
                        name: a.user.name.clone(),
                        email: a.user.email.clone(),
                        login: commit.author.as_ref().map(|u| u.login.clone()),
                    })
                    .unwrap_or_else(|| GitUser {
                        name: "Unknown".to_string(),
                        email: "unknown@example.com".to_string(),
                        login: None,
                    });

                let committer = commit
                    .commit
                    .committer
                    .as_ref()
                    .map(|c| GitUser {
                        name: c.user.name.clone(),
                        email: c.user.email.clone(),
                        login: commit.committer.as_ref().map(|u| u.login.clone()),
                    })
                    .unwrap_or_else(|| GitUser {
                        name: "Unknown".to_string(),
                        email: "unknown@example.com".to_string(),
                        login: None,
                    });

                let parents: Vec<String> =
                    commit.parents.into_iter().filter_map(|p| p.sha).collect();

                Ok(Commit {
                    sha: commit.sha,
                    message: commit.commit.message,
                    author,
                    committer,
                    date: commit
                        .commit
                        .committer
                        .and_then(|c| c.date)
                        .unwrap_or_else(chrono::Utc::now),
                    parents,
                })
            }
            Err(e) => {
                error!("Failed to get commit {}: {}", commit_sha, e);
                Err(CoreError::github(crate::Error::from(e)))
            }
        }
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
    async fn get_latest_release(&self, owner: &str, repo: &str) -> CoreResult<Option<Release>> {
        debug!("Getting latest release for {}/{}", owner, repo);

        let result = self.client.repos(owner, repo).releases().get_latest().await;
        match result {
            Ok(release) => {
                let release = Release {
                    id: release.id.0,
                    tag_name: release.tag_name,
                    name: release.name,
                    body: release.body,
                    draft: release.draft,
                    prerelease: release.prerelease,
                    created_at: release.created_at.unwrap_or_else(chrono::Utc::now),
                    published_at: release.published_at,
                    target_commitish: release.target_commitish,
                    author: release
                        .author
                        .map(|a| GitUser {
                            name: a.login.clone(),
                            email: a.email.unwrap_or_default(),
                            login: Some(a.login),
                        })
                        .unwrap_or_else(|| GitUser {
                            name: "Unknown".to_string(),
                            email: "unknown@example.com".to_string(),
                            login: None,
                        }),
                };

                debug!(
                    "Found latest release {} for {}/{}",
                    release.tag_name, owner, repo
                );
                Ok(Some(release))
            }
            Err(octocrab::Error::GitHub { source, .. }) if source.status_code == 404 => {
                debug!("No releases found for {}/{}", owner, repo);
                Ok(None)
            }
            Err(e) => {
                error!("Failed to get latest release for {}/{}: {}", owner, repo, e);
                Err(CoreError::github(crate::Error::from(e)))
            }
        }
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
        // TODO: Implement using octocrab pulls API - requires careful field access pattern handling
        Err(CoreError::not_supported(
            "get_pull_request",
            "Complex octocrab field access patterns - will be implemented in Phase 3",
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
    async fn get_release_by_tag(&self, owner: &str, repo: &str, tag: &str) -> CoreResult<Release> {
        debug!("Getting release by tag {} for {}/{}", tag, owner, repo);

        let result = self
            .client
            .repos(owner, repo)
            .releases()
            .get_by_tag(tag)
            .await;
        match result {
            Ok(release) => {
                let release = Release {
                    id: release.id.0,
                    tag_name: release.tag_name,
                    name: release.name,
                    body: release.body,
                    draft: release.draft,
                    prerelease: release.prerelease,
                    created_at: release.created_at.unwrap_or_else(chrono::Utc::now),
                    published_at: release.published_at,
                    target_commitish: release.target_commitish,
                    author: release
                        .author
                        .map(|a| GitUser {
                            name: a.login.clone(),
                            email: a.email.unwrap_or_default(),
                            login: Some(a.login),
                        })
                        .unwrap_or_else(|| GitUser {
                            name: "Unknown".to_string(),
                            email: "unknown@example.com".to_string(),
                            login: None,
                        }),
                };

                debug!(
                    "Found release {} for tag {} in {}/{}",
                    release.name.as_ref().unwrap_or(&release.tag_name),
                    tag,
                    owner,
                    repo
                );
                Ok(release)
            }
            Err(e) => {
                error!(
                    "Failed to get release by tag {} for {}/{}: {}",
                    tag, owner, repo, e
                );
                Err(CoreError::github(crate::Error::from(e)))
            }
        }
    }

    /// Retrieve repository information
    ///
    /// # Parameters
    /// - `owner`: Repository owner (user or organization name)
    /// - `repo`: Repository name
    ///
    /// # Returns
    /// Repository information including metadata and configuration
    ///
    /// # Errors
    /// - `CoreError::GitHub` - API communication failed
    /// - `CoreError::InvalidInput` - Invalid owner or repo name
    /// - `CoreError::NotSupported` - Repository not accessible
    async fn get_repository(&self, owner: &str, repo: &str) -> CoreResult<Repository> {
        debug!("Getting repository {}/{}", owner, repo);

        let result = self.client.repos(owner, repo).get().await;
        match result {
            Ok(r) => Ok(Repository {
                id: r.id.0,
                name: r.name,
                full_name: r.full_name.unwrap_or_else(|| format!("{}/{}", owner, repo)),
                owner: r.owner.unwrap().login,
                description: r.description,
                homepage: r.homepage,
                private: r.private.unwrap_or(false),
                default_branch: r.default_branch.unwrap_or_else(|| "main".to_string()),
                clone_url: r.clone_url.map(|u| u.to_string()).unwrap_or_default(),
                ssh_url: r.ssh_url.unwrap_or_default(),
            }),
            Err(e) => {
                error!("Failed to get repository: {}", e);
                Err(CoreError::github(crate::Error::from(e)))
            }
        }
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
        owner: &str,
        repo: &str,
        per_page: Option<u8>,
        page: Option<u32>,
    ) -> CoreResult<Vec<Release>> {
        debug!("Listing releases for {}/{}", owner, repo);

        let repos = self.client.repos(owner, repo);
        let releases = repos.releases();
        let mut releases_handler = releases.list();

        if let Some(per_page) = per_page {
            releases_handler = releases_handler.per_page(per_page);
        }

        if let Some(page) = page {
            releases_handler = releases_handler.page(page);
        }

        let result = releases_handler.send().await;
        match result {
            Ok(releases_page) => {
                let releases: Vec<Release> = releases_page
                    .items
                    .into_iter()
                    .map(|release| Release {
                        id: release.id.0,
                        tag_name: release.tag_name,
                        name: release.name,
                        body: release.body,
                        draft: release.draft,
                        prerelease: release.prerelease,
                        created_at: release.created_at.unwrap_or_else(chrono::Utc::now),
                        published_at: release.published_at,
                        target_commitish: release.target_commitish,
                        author: release
                            .author
                            .map(|a| GitUser {
                                name: a.login.clone(),
                                email: a.email.unwrap_or_default(),
                                login: Some(a.login),
                            })
                            .unwrap_or_else(|| GitUser {
                                name: "Unknown".to_string(),
                                email: "unknown@example.com".to_string(),
                                login: None,
                            }),
                    })
                    .collect();

                debug!("Found {} releases for {}/{}", releases.len(), owner, repo);
                Ok(releases)
            }
            Err(e) => {
                error!("Failed to list releases for {}/{}: {}", owner, repo, e);
                Err(CoreError::github(crate::Error::from(e)))
            }
        }
    }

    /// List all tags in a repository
    ///
    /// # Parameters
    /// - `owner`: Repository owner name
    /// - `repo`: Repository name
    /// - `per_page`: Number of tags per page (max 100)
    /// - `page`: Page number to retrieve (1-based)
    ///
    /// # Returns
    /// List of tags ordered by creation date (newest first)
    ///
    /// # Errors
    /// - `CoreError::GitHub` - API communication failed
    /// - `CoreError::InvalidInput` - Invalid pagination parameters
    async fn list_tags(
        &self,
        owner: &str,
        repo: &str,
        per_page: Option<u8>,
        page: Option<u32>,
    ) -> CoreResult<Vec<Tag>> {
        debug!("Listing tags for {}/{}", owner, repo);

        let repos = self.client.repos(owner, repo);
        let mut tags_handler = repos.list_tags();

        if let Some(per_page) = per_page {
            tags_handler = tags_handler.per_page(per_page);
        }

        if let Some(page) = page {
            tags_handler = tags_handler.page(page);
        }

        let result = tags_handler.send().await;
        match result {
            Ok(tags_page) => {
                let tags: Vec<Tag> = tags_page
                    .items
                    .into_iter()
                    .map(|tag| Tag {
                        name: tag.name,
                        commit_sha: tag.commit.sha,
                        created_at: None, // GitHub API doesn't provide creation date for lightweight tags
                        message: None,    // Lightweight tags don't have messages
                        tagger: None,     // Lightweight tags don't have tagger info
                    })
                    .collect();

                debug!("Found {} tags for {}/{}", tags.len(), owner, repo);
                Ok(tags)
            }
            Err(e) => {
                error!("Failed to list tags for {}/{}: {}", owner, repo, e);
                Err(CoreError::github(crate::Error::from(e)))
            }
        }
    }

    /// Check if a tag exists
    ///
    /// # Parameters
    /// - `owner`: Repository owner name
    /// - `repo`: Repository name
    /// - `tag_name`: Tag name to check
    ///
    /// # Returns
    /// True if tag exists, false otherwise
    ///
    /// # Errors
    /// - `CoreError::GitHub` - API communication failed
    /// - `CoreError::InvalidInput` - Invalid tag name
    async fn tag_exists(&self, owner: &str, repo: &str, tag_name: &str) -> CoreResult<bool> {
        debug!("Checking if tag {} exists for {}/{}", tag_name, owner, repo);

        // Use list_tags and check if our tag exists in the list
        match self.list_tags(owner, repo, Some(100), None).await {
            Ok(tags) => {
                let exists = tags.iter().any(|tag| tag.name == tag_name);
                debug!("Tag {} exists for {}/{}: {}", tag_name, owner, repo, exists);
                Ok(exists)
            }
            Err(e) => {
                error!(
                    "Failed to check if tag {} exists for {}/{}: {}",
                    tag_name, owner, repo, e
                );
                Err(e)
            }
        }
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
        // TODO: Implement using octocrab pulls API - requires careful field access pattern handling
        Err(CoreError::not_supported(
            "update_pull_request",
            "Complex octocrab field access patterns - will be implemented in Phase 3",
        ))
    }

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
    ) -> CoreResult<Release> {
        debug!("Updating release {} for {}/{}", release_id, owner, repo);

        let mut update_params = serde_json::Map::new();

        if let Some(name) = params.name {
            update_params.insert("name".to_string(), serde_json::Value::String(name));
        }

        if let Some(body) = params.body {
            update_params.insert("body".to_string(), serde_json::Value::String(body));
        }

        if let Some(draft) = params.draft {
            update_params.insert("draft".to_string(), serde_json::Value::Bool(draft));
        }

        if let Some(prerelease) = params.prerelease {
            update_params.insert(
                "prerelease".to_string(),
                serde_json::Value::Bool(prerelease),
            );
        }

        let result: Result<octocrab::models::repos::Release, _> = self
            .client
            .patch(
                &format!("/repos/{}/{}/releases/{}", owner, repo, release_id),
                Some(&serde_json::Value::Object(update_params)),
            )
            .await;

        match result {
            Ok(release) => {
                let release = Release {
                    id: release.id.0,
                    tag_name: release.tag_name,
                    name: release.name,
                    body: release.body,
                    draft: release.draft,
                    prerelease: release.prerelease,
                    created_at: release.created_at.unwrap_or_else(chrono::Utc::now),
                    published_at: release.published_at,
                    target_commitish: release.target_commitish,
                    author: release
                        .author
                        .map(|a| GitUser {
                            name: a.login.clone(),
                            email: a.email.unwrap_or_default(),
                            login: Some(a.login),
                        })
                        .unwrap_or_else(|| GitUser {
                            name: "Unknown".to_string(),
                            email: "unknown@example.com".to_string(),
                            login: None,
                        }),
                };

                debug!(
                    "Updated release {} '{}' for {}/{}",
                    release.id,
                    release.name.as_ref().unwrap_or(&release.tag_name),
                    owner,
                    repo
                );
                Ok(release)
            }
            Err(e) => {
                error!(
                    "Failed to update release {} for {}/{}: {}",
                    release_id, owner, repo, e
                );
                Err(CoreError::github(crate::Error::from(e)))
            }
        }
    }
}

/// Helper function to log Octocrab errors with appropriate detail.
///
/// This function examines the type of Octocrab error and logs relevant
/// information for debugging purposes. It handles different error types
/// with appropriate context and formatting.
fn log_octocrab_error(message: &str, e: octocrab::Error) {
    match e {
        octocrab::Error::GitHub { source, backtrace } => {
            let err = source;
            error!(
                error_message = err.message,
                backtrace = backtrace.to_string(),
                "{}. Received an error from GitHub",
                message
            )
        }
        octocrab::Error::UriParse { source, backtrace } => error!(
            error_message = source.to_string(),
            backtrace = backtrace.to_string(),
            "{}. Failed to parse URI.",
            message
        ),

        octocrab::Error::Uri { source, backtrace } => error!(
            error_message = source.to_string(),
            backtrace = backtrace.to_string(),
            "{}, Failed to parse URI.",
            message
        ),
        octocrab::Error::InvalidHeaderValue { source, backtrace } => error!(
            error_message = source.to_string(),
            backtrace = backtrace.to_string(),
            "{}. One of the header values was invalid.",
            message
        ),
        octocrab::Error::InvalidUtf8 { source, backtrace } => error!(
            error_message = source.to_string(),
            backtrace = backtrace.to_string(),
            "{}. The message wasn't valid UTF-8.",
            message,
        ),
        _ => error!(error_message = e.to_string(), message),
    };
}

// Reference the tests module in the separate file
#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
