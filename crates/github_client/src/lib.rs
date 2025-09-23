//! Crate for interacting with the GitHub REST API.
//!
//! This crate provides a client for making authenticated requests to GitHub,
//! authenticating as a GitHub App using its ID and private key.

use async_trait::async_trait;
use octocrab::{Octocrab, Result as OctocrabResult};
use release_regent_core::{traits::github_operations::*, CoreError, CoreResult, GitHubOperations};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, error, info, instrument, warn};

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

/// Configuration for retry logic
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Base delay between retries (will be exponentially increased)
    pub base_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Factor by which delay is multiplied after each retry
    pub backoff_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_factor: 2.0,
        }
    }
}

/// Rate limiting state tracker
#[derive(Debug)]
struct RateLimitState {
    /// Remaining requests in current window
    remaining: Option<u32>,
    /// Time when rate limit window resets
    reset_time: Option<Instant>,
    /// Whether we're currently rate limited
    is_limited: bool,
    /// Time when secondary rate limit expires (if any)
    secondary_reset_time: Option<Instant>,
}

impl Default for RateLimitState {
    fn default() -> Self {
        Self {
            remaining: None,
            reset_time: None,
            is_limited: false,
            secondary_reset_time: None,
        }
    }
}

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
    /// Retry configuration for handling transient failures
    retry_config: RetryConfig,
    /// Rate limiting state to track API limits
    rate_limit_state: Arc<Mutex<RateLimitState>>,
    /// Correlation ID for request tracking
    correlation_id: String,
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
            retry_config: RetryConfig::default(),
            rate_limit_state: Arc::new(Mutex::new(RateLimitState::default())),
            correlation_id: uuid::Uuid::new_v4().to_string(),
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
            retry_config: RetryConfig::default(),
            rate_limit_state: Arc::new(Mutex::new(RateLimitState::default())),
            correlation_id: uuid::Uuid::new_v4().to_string(),
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
            retry_config: RetryConfig::default(),
            rate_limit_state: Arc::new(Mutex::new(RateLimitState::default())),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        })
    }

    /// Configure retry behavior for API requests
    ///
    /// # Arguments
    /// * `retry_config` - Configuration for retry logic including max attempts and backoff settings
    ///
    /// # Examples
    /// ```rust,no_run
    /// use release_regent_github_client::{GitHubClient, RetryConfig};
    /// use std::time::Duration;
    /// use octocrab::Octocrab;
    ///
    /// let octocrab_client = Octocrab::builder().build().unwrap();
    /// let mut client = GitHubClient::new(octocrab_client);
    /// let retry_config = RetryConfig {
    ///     max_attempts: 5,
    ///     base_delay: Duration::from_millis(200),
    ///     max_delay: Duration::from_secs(60),
    ///     backoff_factor: 2.5,
    /// };
    /// client.with_retry_config(retry_config);
    /// ```
    pub fn with_retry_config(mut self, retry_config: RetryConfig) -> Self {
        self.retry_config = retry_config;
        self
    }

    // === FACTORY METHODS FOR ENHANCED DEVELOPER EXPERIENCE ===

    /// Creates a new GitHubClient from GitHub App credentials with enhanced configuration.
    ///
    /// This is a high-level factory method that creates a fully configured GitHub client
    /// using GitHub App authentication. It automatically sets up authentication management,
    /// retry logic, and rate limiting for production use.
    ///
    /// # Arguments
    ///
    /// * `app_id` - The GitHub App ID
    /// * `private_key` - The private key for JWT signing
    /// * `github_base_url` - Optional GitHub Enterprise Server base URL
    ///
    /// # Returns
    ///
    /// Returns a fully configured `GitHubClient` instance ready for GitHub App operations.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid, the private key cannot be parsed,
    /// or the client cannot be created.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use release_regent_github_client::GitHubClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = GitHubClient::from_github_app(
    ///         123456,
    ///         "-----BEGIN RSA PRIVATE KEY-----\n...\n-----END RSA PRIVATE KEY-----",
    ///         None, // Use GitHub.com
    ///     ).await?;
    ///
    ///     let installations = client.list_installations().await?;
    ///     println!("Found {} installations", installations.len());
    ///     Ok(())
    /// }
    /// ```
    pub async fn from_github_app(
        app_id: u64,
        private_key: impl Into<String>,
        github_base_url: Option<String>,
    ) -> GitHubResult<Self> {
        let config = AuthConfig::new(app_id, private_key, github_base_url)?;
        let auth_manager = GitHubAuthManager::new(config)?;
        Self::with_auth_manager(auth_manager).await
    }

    /// Creates a new GitHubClient from environment variables.
    ///
    /// This factory method reads GitHub App credentials from environment variables
    /// and creates a fully configured client. This is ideal for containerized environments
    /// and CI/CD pipelines where secrets are managed through environment variables.
    ///
    /// Expected environment variables:
    /// - `GITHUB_APP_ID`: GitHub App ID
    /// - `GITHUB_PRIVATE_KEY`: Private key content
    /// - `GITHUB_BASE_URL`: Optional GitHub Enterprise Server URL
    ///
    /// # Returns
    ///
    /// Returns a fully configured `GitHubClient` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if required environment variables are missing or invalid.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use release_regent_github_client::GitHubClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // Assumes GITHUB_APP_ID and GITHUB_PRIVATE_KEY are set
    ///     let client = GitHubClient::from_env().await?;
    ///
    ///     let installations = client.list_installations().await?;
    ///     println!("Found {} installations", installations.len());
    ///     Ok(())
    /// }
    /// ```
    pub async fn from_env() -> GitHubResult<Self> {
        let config = AuthConfig::from_env()?;
        let auth_manager = GitHubAuthManager::new(config)?;
        Self::with_auth_manager(auth_manager).await
    }

    /// Creates a new GitHubClient using a personal access token.
    ///
    /// This factory method creates a client authenticated with a personal access token
    /// rather than GitHub App authentication. This is useful for user-specific operations
    /// or when GitHub App setup is not available.
    ///
    /// # Arguments
    ///
    /// * `token` - The personal access token
    /// * `github_base_url` - Optional GitHub Enterprise Server base URL
    ///
    /// # Returns
    ///
    /// Returns a fully configured `GitHubClient` instance authenticated with the token.
    ///
    /// # Errors
    ///
    /// Returns an error if the token is invalid or the client cannot be created.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use release_regent_github_client::GitHubClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = GitHubClient::from_token(
    ///         "ghp_your_personal_access_token",
    ///         None, // Use GitHub.com
    ///     ).await?;
    ///
    ///     let repo = client.get_repository("owner", "repo").await?;
    ///     println!("Repository: {}", repo.name());
    ///     Ok(())
    /// }
    /// ```
    pub async fn from_token(
        token: impl Into<String>,
        github_base_url: Option<String>,
    ) -> GitHubResult<Self> {
        let token = token.into();
        if token.is_empty() {
            return Err(Error::invalid_input("token", "Token cannot be empty"));
        }

        let base_url = github_base_url
            .map(|url| format!("{}/api/v3", url))
            .unwrap_or_else(|| "https://api.github.com".to_string());

        let client = octocrab::Octocrab::builder()
            .base_uri(&base_url)
            .map_err(|e| {
                Error::configuration(
                    "base_uri",
                    &format!("Failed to configure GitHub client: {}", e),
                )
            })?
            .personal_token(token)
            .build()
            .map_err(|e| {
                Error::configuration(
                    "client_build",
                    &format!("Failed to build GitHub client: {}", e),
                )
            })?;

        Ok(Self::new(client))
    }

    /// Creates a GitHub client for a specific installation with enhanced configuration.
    ///
    /// This factory method creates a client authenticated for a specific GitHub App installation.
    /// The client is automatically configured with installation tokens, retry logic, and
    /// rate limiting for production use.
    ///
    /// # Arguments
    ///
    /// * `app_id` - The GitHub App ID
    /// * `private_key` - The private key for JWT signing
    /// * `installation_id` - The specific installation ID to authenticate for
    /// * `github_base_url` - Optional GitHub Enterprise Server base URL
    ///
    /// # Returns
    ///
    /// Returns a `GitHubClient` instance authenticated for the specified installation.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid, the installation token cannot
    /// be acquired, or the client cannot be created.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use release_regent_github_client::GitHubClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = GitHubClient::for_installation(
    ///         123456,
    ///         "-----BEGIN RSA PRIVATE KEY-----\n...\n-----END RSA PRIVATE KEY-----",
    ///         987654, // Installation ID
    ///         None,   // Use GitHub.com
    ///     ).await?;
    ///
    ///     let repo = client.get_repository("owner", "repo").await?;
    ///     println!("Repository: {}", repo.name());
    ///     Ok(())
    /// }
    /// ```
    pub async fn for_installation(
        app_id: u64,
        private_key: impl Into<String>,
        installation_id: u64,
        github_base_url: Option<String>,
    ) -> GitHubResult<Self> {
        let config = AuthConfig::new(app_id, private_key, github_base_url)?;
        let auth_manager = GitHubAuthManager::new(config)?;
        let client = auth_manager
            .create_installation_client(installation_id)
            .await?;

        Ok(Self {
            client,
            auth_manager: Some(auth_manager),
            retry_config: RetryConfig::default(),
            rate_limit_state: Arc::new(Mutex::new(RateLimitState::default())),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        })
    }

    /// Get the current correlation ID for request tracking
    pub fn correlation_id(&self) -> &str {
        &self.correlation_id
    }

    /// Factory method for GitHub App authentication.
    ///
    /// Creates a GitHubClient using GitHub App credentials for JWT-based authentication.
    /// This is suitable for GitHub App installations and accessing app-level operations.
    ///
    /// # Arguments
    /// * `app_id` - The GitHub App ID
    /// * `private_key` - The GitHub App private key in PEM format
    ///
    /// # Returns
    /// Returns a configured `GitHubClient` instance for GitHub App authentication.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use release_regent_github_client::GitHubClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = GitHubClient::from_app(123456, "-----BEGIN RSA PRIVATE KEY-----...").await?;
    ///     let installations = client.list_installations().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn from_app(app_id: u64, private_key: impl Into<String>) -> GitHubResult<Self> {
        let config = AuthConfig::new(app_id, private_key, None)?;
        let auth_manager = GitHubAuthManager::new(config)?;
        Self::with_auth_manager(auth_manager).await
    }

    /// Factory method for installation-specific authentication.
    ///
    /// Creates a GitHubClient authenticated for a specific GitHub App installation.
    /// This is suitable for operations on repositories where the app is installed.
    ///
    /// # Arguments
    /// * `app_id` - The GitHub App ID
    /// * `private_key` - The GitHub App private key in PEM format
    /// * `installation_id` - The installation ID for the target organization/user
    ///
    /// # Returns
    /// Returns a configured `GitHubClient` instance for installation authentication.
    ///
    /// # Examples
    /// ```rust,no_run
    /// use release_regent_github_client::GitHubClient;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = GitHubClient::from_installation(123456, "-----BEGIN RSA PRIVATE KEY-----...", 987654).await?;
    ///     let repo = client.get_repository("org", "repo").await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn from_installation(
        app_id: u64,
        private_key: impl Into<String>,
        installation_id: u64,
    ) -> GitHubResult<Self> {
        let config = AuthConfig::new(app_id, private_key, None)?;
        let auth_manager = GitHubAuthManager::new(config)?;
        let app_client = Self::with_auth_manager(auth_manager).await?;
        app_client.create_installation_client(installation_id).await
    }

    // === CONFIGURATION BUILDER METHODS ===

    /// Creates a builder for configuring GitHub App authentication.
    ///
    /// This method returns a `GitHubAppBuilder` that provides a fluent API
    /// for configuring GitHub App authentication with custom settings.
    ///
    /// # Arguments
    ///
    /// * `app_id` - The GitHub App ID
    /// * `private_key` - The private key for JWT signing
    ///
    /// # Returns
    ///
    /// Returns a `GitHubAppBuilder` for configuring the client.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use release_regent_github_client::GitHubClient;
    /// use std::time::Duration;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = GitHubClient::app_builder(
    ///         123456,
    ///         "-----BEGIN RSA PRIVATE KEY-----\n...\n-----END RSA PRIVATE KEY-----"
    ///     )
    ///     .github_enterprise("https://github.enterprise.com")
    ///     .jwt_expiration(Duration::from_secs(300))
    ///     .retry_config(|config| config.max_attempts(5).base_delay(Duration::from_millis(200)))
    ///     .build().await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn app_builder(app_id: u64, private_key: impl Into<String>) -> GitHubAppBuilder {
        GitHubAppBuilder::new(app_id, private_key.into())
    }

    /// Creates a builder for configuring personal access token authentication.
    ///
    /// This method returns a `TokenBuilder` that provides a fluent API
    /// for configuring token-based authentication with custom settings.
    ///
    /// # Arguments
    ///
    /// * `token` - The personal access token
    ///
    /// # Returns
    ///
    /// Returns a `TokenBuilder` for configuring the client.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use release_regent_github_client::GitHubClient;
    /// use std::time::Duration;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = GitHubClient::token_builder("ghp_your_token")
    ///         .github_enterprise("https://github.enterprise.com")
    ///         .retry_config(|config| config.max_attempts(3))
    ///         .build().await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn token_builder(token: impl Into<String>) -> TokenBuilder {
        TokenBuilder::new(token.into())
    }

    /// Creates a builder for configuring installation-specific authentication.
    ///
    /// This method returns an `InstallationBuilder` that provides a fluent API
    /// for configuring installation-specific authentication with custom settings.
    ///
    /// # Arguments
    ///
    /// * `app_id` - The GitHub App ID
    /// * `private_key` - The private key for JWT signing
    /// * `installation_id` - The installation ID to authenticate for
    ///
    /// # Returns
    ///
    /// Returns an `InstallationBuilder` for configuring the client.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use release_regent_github_client::GitHubClient;
    /// use std::time::Duration;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = GitHubClient::installation_builder(
    ///         123456,
    ///         "-----BEGIN RSA PRIVATE KEY-----\n...\n-----END RSA PRIVATE KEY-----",
    ///         987654
    ///     )
    ///     .github_enterprise("https://github.enterprise.com")
    ///     .retry_config(|config| config.max_attempts(5))
    ///     .build().await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn installation_builder(
        app_id: u64,
        private_key: impl Into<String>,
        installation_id: u64,
    ) -> InstallationBuilder {
        InstallationBuilder::new(app_id, private_key.into(), installation_id)
    }

    /// Execute a GitHub API operation with retry logic and rate limiting
    async fn execute_with_retry<F, T, Fut>(
        &self,
        operation_name: &str,
        operation: F,
    ) -> CoreResult<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, octocrab::Error>>,
    {
        let mut attempt = 0;
        let mut delay = self.retry_config.base_delay;

        loop {
            attempt += 1;

            // Check and wait for rate limits before making the request
            self.check_rate_limits().await?;

            let start_time = Instant::now();

            info!(
                operation = operation_name,
                attempt = attempt,
                correlation_id = self.correlation_id,
                "Executing GitHub API operation"
            );

            match operation().await {
                Ok(result) => {
                    let duration = start_time.elapsed();
                    info!(
                        operation = operation_name,
                        attempt = attempt,
                        duration_ms = duration.as_millis(),
                        correlation_id = self.correlation_id,
                        "GitHub API operation completed successfully"
                    );
                    return Ok(result);
                }
                Err(error) => {
                    let duration = start_time.elapsed();

                    // Update rate limit state from error headers if available
                    self.update_rate_limit_from_error(&error).await;

                    // Check if this is a retryable error
                    if !self.is_retryable_error(&error) || attempt >= self.retry_config.max_attempts
                    {
                        error!(
                            operation = operation_name,
                            attempt = attempt,
                            duration_ms = duration.as_millis(),
                            error = %error,
                            correlation_id = self.correlation_id,
                            "GitHub API operation failed permanently"
                        );
                        return Err(CoreError::github(crate::Error::from(error)));
                    }

                    warn!(
                        operation = operation_name,
                        attempt = attempt,
                        duration_ms = duration.as_millis(),
                        error = %error,
                        next_delay_ms = delay.as_millis(),
                        correlation_id = self.correlation_id,
                        "GitHub API operation failed, retrying"
                    );

                    // Wait before retrying
                    tokio::time::sleep(delay).await;

                    // Calculate next delay with exponential backoff
                    delay = Duration::from_millis(std::cmp::min(
                        (delay.as_millis() as f64 * self.retry_config.backoff_factor) as u64,
                        self.retry_config.max_delay.as_millis() as u64,
                    ));
                }
            }
        }
    }

    /// Check if we need to wait for rate limits
    async fn check_rate_limits(&self) -> CoreResult<()> {
        let mut state = self.rate_limit_state.lock().await;
        let now = Instant::now();

        // Check secondary rate limit first (abuse detection)
        if let Some(secondary_reset) = state.secondary_reset_time {
            if now < secondary_reset {
                let wait_time = secondary_reset.duration_since(now);
                warn!(
                    wait_time_ms = wait_time.as_millis(),
                    correlation_id = self.correlation_id,
                    "Waiting for secondary rate limit to reset"
                );
                drop(state); // Release lock before sleeping
                tokio::time::sleep(wait_time).await;
                return Ok(());
            } else {
                // Secondary rate limit has expired
                let mut state = self.rate_limit_state.lock().await;
                state.secondary_reset_time = None;
            }
        }

        // Check primary rate limit
        if state.is_limited {
            if let Some(reset_time) = state.reset_time {
                if now < reset_time {
                    let wait_time = reset_time.duration_since(now);
                    warn!(
                        wait_time_ms = wait_time.as_millis(),
                        remaining = state.remaining,
                        correlation_id = self.correlation_id,
                        "Waiting for primary rate limit to reset"
                    );
                    drop(state); // Release lock before sleeping
                    tokio::time::sleep(wait_time).await;
                    return Ok(());
                } else {
                    // Rate limit has reset
                    state.is_limited = false;
                    state.reset_time = None;
                    state.remaining = None;
                }
            }
        }

        Ok(())
    }

    /// Update rate limit state from GitHub API error
    async fn update_rate_limit_from_error(&self, error: &octocrab::Error) {
        if let octocrab::Error::GitHub { source, .. } = error {
            let mut state = self.rate_limit_state.lock().await;

            // Check for rate limit exceeded (status 403)
            if source.status_code == 403 {
                // Primary rate limit exceeded
                if source.message.contains("rate limit exceeded") {
                    state.is_limited = true;
                    // GitHub typically resets every hour, but we'll be conservative
                    state.reset_time = Some(Instant::now() + Duration::from_secs(3600));
                    warn!(
                        error_message = source.message,
                        correlation_id = self.correlation_id,
                        "Primary rate limit exceeded"
                    );
                }
                // Secondary rate limit (abuse detection)
                else if source.message.contains("abuse") || source.status_code == 403 {
                    state.secondary_reset_time = Some(Instant::now() + Duration::from_secs(60));
                    warn!(
                        error_message = source.message,
                        correlation_id = self.correlation_id,
                        "Secondary rate limit (abuse detection) triggered"
                    );
                }
            }
        }
    }

    /// Check if an error is retryable
    fn is_retryable_error(&self, error: &octocrab::Error) -> bool {
        match error {
            octocrab::Error::GitHub { source, .. } => {
                match source.status_code.as_u16() {
                    // Rate limiting - retryable after waiting
                    403 if source.message.contains("rate limit") => true,
                    // Secondary rate limiting - retryable after waiting
                    403 if source.message.contains("abuse") => true,
                    // Server errors - retryable
                    500..=599 => true,
                    // Request timeout - retryable
                    408 => true,
                    // Too many requests - retryable
                    429 => true,
                    // Client errors are generally not retryable
                    400..=499 => false,
                    // Other status codes - not retryable
                    _ => false,
                }
            }
            // Network/connection errors - retryable
            octocrab::Error::InvalidHeaderValue { .. } => false,
            octocrab::Error::Uri { .. } => false,
            octocrab::Error::UriParse { .. } => false,
            octocrab::Error::InvalidUtf8 { .. } => false,
            // Default to retryable for unknown error types
            _ => true,
        }
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
            operation = "compare_commits",
            owner = owner,
            repo = repo,
            base = base,
            head = head,
            per_page = per_page.unwrap_or(30),
            page = page.unwrap_or(1),
            correlation_id = self.correlation_id,
            "Comparing commits between base and head"
        );

        // For now, implement a simple version that gets commits from head
        // In a full implementation, this would use the GitHub compare API
        let commits_page = self
            .execute_with_retry("compare_commits", || async {
                let repos = self.client.repos(owner, repo);
                let commits = repos.list_commits();
                commits.sha(head).send().await
            })
            .await?;

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
            operation = "compare_commits",
            owner = owner,
            repo = repo,
            base = base,
            head = head,
            commit_count = commits.len(),
            correlation_id = self.correlation_id,
            "Found commits for comparison"
        );
        Ok(commits)
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
        owner: &str,
        repo: &str,
        params: CreatePullRequestParams,
    ) -> CoreResult<PullRequest> {
        debug!(
            operation = "create_pull_request",
            owner = owner,
            repo = repo,
            head = params.head,
            base = params.base,
            title = params.title,
            correlation_id = self.correlation_id,
            "Creating pull request (not yet implemented)"
        );

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
            operation = "create_release",
            owner = owner,
            repo = repo,
            tag_name = params.tag_name,
            name = params.name.as_ref().unwrap_or(&params.tag_name),
            draft = params.draft,
            prerelease = params.prerelease,
            correlation_id = self.correlation_id,
            "Creating new release"
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

        let release: octocrab::models::repos::Release = self
            .execute_with_retry("create_release", || async {
                self.client
                    .post(
                        &format!("/repos/{}/{}/releases", owner, repo),
                        Some(&release_create_params),
                    )
                    .await
            })
            .await?;

        let result = Release {
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
            release_id = result.id,
            correlation_id = self.correlation_id,
            "Successfully created release"
        );
        Ok(result)
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
            operation = "create_tag",
            owner = owner,
            repo = repo,
            tag_name = tag_name,
            commit_sha = commit_sha,
            correlation_id = self.correlation_id,
            "Creating tag for commit"
        );

        // For lightweight tags, we create a reference directly
        let tag_ref = format!("refs/tags/{}", tag_name);

        let tag_create_params = serde_json::json!({
            "ref": tag_ref,
            "sha": commit_sha
        });

        let _result: serde_json::Value = self
            .execute_with_retry("create_tag", || async {
                self.client
                    .post(
                        &format!("/repos/{}/{}/git/refs", owner, repo),
                        Some(&tag_create_params),
                    )
                    .await
            })
            .await?;

        let tag = Tag {
            name: tag_name.to_string(),
            commit_sha: commit_sha.to_string(),
            created_at: Some(chrono::Utc::now()),
            message,
            tagger,
        };

        debug!(
            operation = "create_tag",
            owner = owner,
            repo = repo,
            tag_name = tag_name,
            commit_sha = commit_sha,
            correlation_id = self.correlation_id,
            "Successfully created tag"
        );
        Ok(tag)
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
        debug!(
            operation = "get_commit",
            owner = owner,
            repo = repo,
            commit_sha = commit_sha,
            correlation_id = self.correlation_id,
            "Getting commit information"
        );

        let commit = self
            .execute_with_retry("get_commit", || async {
                self.client.commits(owner, repo).get(commit_sha).await
            })
            .await?;

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

        let parents: Vec<String> = commit.parents.into_iter().filter_map(|p| p.sha).collect();

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
        debug!(
            operation = "get_latest_release",
            owner = owner,
            repo = repo,
            correlation_id = self.correlation_id,
            "Getting latest release"
        );

        let release_result = self
            .execute_with_retry("get_latest_release", || async {
                self.client.repos(owner, repo).releases().get_latest().await
            })
            .await;

        let release = match release_result {
            Ok(release) => release,
            Err(CoreError::GitHub { source, .. }) => {
                // Check if this is a 404 (no releases found)
                if source.to_string().contains("404") {
                    debug!(
                        operation = "get_latest_release",
                        owner = owner,
                        repo = repo,
                        correlation_id = self.correlation_id,
                        "No releases found"
                    );
                    return Ok(None);
                }
                return Err(CoreError::GitHub {
                    source,
                    context: None,
                });
            }
            Err(e) => return Err(e),
        };

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
            operation = "get_latest_release",
            owner = owner,
            repo = repo,
            tag_name = release.tag_name.as_str(),
            correlation_id = self.correlation_id,
            "Found latest release"
        );
        Ok(Some(release))
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
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> CoreResult<PullRequest> {
        debug!(
            operation = "get_pull_request",
            owner = owner,
            repo = repo,
            pr_number = pr_number,
            correlation_id = self.correlation_id,
            "Getting pull request (not yet implemented)"
        );

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
        debug!(
            operation = "get_release_by_tag",
            owner = owner,
            repo = repo,
            tag = tag,
            correlation_id = self.correlation_id,
            "Getting release by tag"
        );

        let release = self
            .execute_with_retry("get_release_by_tag", || async {
                self.client
                    .repos(owner, repo)
                    .releases()
                    .get_by_tag(tag)
                    .await
            })
            .await?;

        let result = Release {
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
            operation = "get_release_by_tag",
            owner = owner,
            repo = repo,
            tag = tag,
            release_name = result.name.as_ref().unwrap_or(&result.tag_name),
            correlation_id = self.correlation_id,
            "Successfully retrieved release by tag"
        );
        Ok(result)
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
        debug!(
            operation = "get_repository",
            owner = owner,
            repo = repo,
            correlation_id = self.correlation_id,
            "Getting repository information"
        );

        self.execute_with_retry("get_repository", || async {
            self.client.repos(owner, repo).get().await
        })
        .await
        .map(|r| Repository {
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
        })
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
        debug!(
            operation = "list_releases",
            owner = owner,
            repo = repo,
            per_page = per_page.unwrap_or(30),
            page = page.unwrap_or(1),
            correlation_id = self.correlation_id,
            "Listing repository releases"
        );

        let releases_page = self
            .execute_with_retry("list_releases", || async {
                let repos = self.client.repos(owner, repo);
                let releases = repos.releases();
                let mut releases_handler = releases.list();

                if let Some(per_page) = per_page {
                    releases_handler = releases_handler.per_page(per_page);
                }

                if let Some(page) = page {
                    releases_handler = releases_handler.page(page);
                }

                releases_handler.send().await
            })
            .await?;

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

        debug!(
            operation = "list_releases",
            owner = owner,
            repo = repo,
            releases_count = releases.len(),
            correlation_id = self.correlation_id,
            "Successfully retrieved releases list"
        );
        Ok(releases)
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
        debug!(
            operation = "list_tags",
            owner = owner,
            repo = repo,
            per_page = per_page.unwrap_or(30),
            page = page.unwrap_or(1),
            correlation_id = self.correlation_id,
            "Listing repository tags"
        );

        let tags_page = self
            .execute_with_retry("list_tags", || async {
                let repos = self.client.repos(owner, repo);
                let mut tags_handler = repos.list_tags();

                if let Some(per_page) = per_page {
                    tags_handler = tags_handler.per_page(per_page);
                }

                if let Some(page) = page {
                    tags_handler = tags_handler.page(page);
                }

                tags_handler.send().await
            })
            .await?;

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

        debug!(
            tags_count = tags.len(),
            correlation_id = self.correlation_id,
            "Successfully retrieved tags"
        );
        Ok(tags)
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
        debug!(
            operation = "tag_exists",
            owner = owner,
            repo = repo,
            tag_name = tag_name,
            correlation_id = self.correlation_id,
            "Checking if tag exists"
        );

        // Use list_tags and check if our tag exists in the list
        match self.list_tags(owner, repo, Some(100), None).await {
            Ok(tags) => {
                let exists = tags.iter().any(|tag| tag.name == tag_name);
                debug!(
                    operation = "tag_exists",
                    owner = owner,
                    repo = repo,
                    tag_name = tag_name,
                    exists = exists,
                    correlation_id = self.correlation_id,
                    "Tag existence check completed"
                );
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
        owner: &str,
        repo: &str,
        pr_number: u64,
        title: Option<String>,
        body: Option<String>,
        state: Option<String>,
    ) -> CoreResult<PullRequest> {
        debug!(
            operation = "update_pull_request",
            owner = owner,
            repo = repo,
            pr_number = pr_number,
            has_title = title.is_some(),
            has_body = body.is_some(),
            state = state.as_deref(),
            correlation_id = self.correlation_id,
            "Updating pull request (not yet implemented)"
        );

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
        debug!(
            operation = "update_release",
            owner = owner,
            repo = repo,
            release_id = release_id,
            correlation_id = self.correlation_id,
            "Updating release"
        );

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

        let release: octocrab::models::repos::Release = self
            .execute_with_retry("update_release", || async {
                self.client
                    .patch(
                        &format!("/repos/{}/{}/releases/{}", owner, repo, release_id),
                        Some(&serde_json::Value::Object(update_params.clone())),
                    )
                    .await
            })
            .await?;

        let result = Release {
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
            operation = "update_release",
            owner = owner,
            repo = repo,
            release_id = result.id,
            release_name = result.name.as_ref().unwrap_or(&result.tag_name),
            correlation_id = self.correlation_id,
            "Successfully updated release"
        );
        Ok(result)
    }
}

// === BUILDER STRUCTS FOR ENHANCED DEVELOPER EXPERIENCE ===

/// Builder for configuring GitHub App authentication.
///
/// This builder provides a fluent API for configuring GitHub App authentication
/// with custom settings for retry logic, enterprise support, and JWT configuration.
pub struct GitHubAppBuilder {
    app_id: u64,
    private_key: String,
    github_base_url: Option<String>,
    jwt_expiration_seconds: Option<u64>,
    token_refresh_buffer_seconds: Option<u64>,
    retry_config: Option<RetryConfig>,
}

impl GitHubAppBuilder {
    /// Creates a new GitHub App builder.
    fn new(app_id: u64, private_key: String) -> Self {
        Self {
            app_id,
            private_key,
            github_base_url: None,
            jwt_expiration_seconds: None,
            token_refresh_buffer_seconds: None,
            retry_config: None,
        }
    }

    /// Sets the GitHub Enterprise Server base URL.
    ///
    /// # Arguments
    /// * `base_url` - The base URL for GitHub Enterprise Server
    pub fn github_enterprise(mut self, base_url: impl Into<String>) -> Self {
        self.github_base_url = Some(base_url.into());
        self
    }

    /// Sets the JWT expiration time.
    ///
    /// # Arguments
    /// * `duration` - The JWT expiration duration (max 10 minutes)
    pub fn jwt_expiration(mut self, duration: Duration) -> Self {
        self.jwt_expiration_seconds = Some(duration.as_secs().min(600));
        self
    }

    /// Sets the token refresh buffer time.
    ///
    /// # Arguments
    /// * `duration` - The buffer time before token expiration to trigger refresh
    pub fn token_refresh_buffer(mut self, duration: Duration) -> Self {
        self.token_refresh_buffer_seconds = Some(duration.as_secs());
        self
    }

    /// Configures retry behavior using a configuration function.
    ///
    /// # Arguments
    /// * `config_fn` - Function that takes a RetryConfigBuilder and returns a configured RetryConfig
    pub fn retry_config<F>(mut self, config_fn: F) -> Self
    where
        F: FnOnce(RetryConfigBuilder) -> RetryConfigBuilder,
    {
        let builder = RetryConfigBuilder::default();
        let configured_builder = config_fn(builder);
        self.retry_config = Some(configured_builder.build());
        self
    }

    /// Sets a custom retry configuration directly.
    ///
    /// # Arguments
    /// * `config` - The retry configuration to use
    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = Some(config);
        self
    }

    /// Builds the configured GitHubClient.
    ///
    /// # Returns
    /// Returns a fully configured `GitHubClient` instance.
    ///
    /// # Errors
    /// Returns an error if the configuration is invalid or the client cannot be created.
    pub async fn build(self) -> GitHubResult<GitHubClient> {
        let mut config = AuthConfig::new(self.app_id, self.private_key, self.github_base_url)?;

        if let Some(jwt_expiration) = self.jwt_expiration_seconds {
            config.jwt_expiration_seconds = jwt_expiration;
        }

        if let Some(token_refresh_buffer) = self.token_refresh_buffer_seconds {
            config.token_refresh_buffer_seconds = token_refresh_buffer;
        }

        let auth_manager = GitHubAuthManager::new(config)?;
        let mut client = GitHubClient::with_auth_manager(auth_manager).await?;

        if let Some(retry_config) = self.retry_config {
            client.retry_config = retry_config;
        }

        Ok(client)
    }
}

/// Builder for configuring personal access token authentication.
///
/// This builder provides a fluent API for configuring token-based authentication
/// with custom settings for retry logic and enterprise support.
pub struct TokenBuilder {
    token: String,
    github_base_url: Option<String>,
    retry_config: Option<RetryConfig>,
}

impl TokenBuilder {
    /// Creates a new token builder.
    fn new(token: String) -> Self {
        Self {
            token,
            github_base_url: None,
            retry_config: None,
        }
    }

    /// Sets the GitHub Enterprise Server base URL.
    ///
    /// # Arguments
    /// * `base_url` - The base URL for GitHub Enterprise Server
    pub fn github_enterprise(mut self, base_url: impl Into<String>) -> Self {
        self.github_base_url = Some(base_url.into());
        self
    }

    /// Configures retry behavior using a configuration function.
    ///
    /// # Arguments
    /// * `config_fn` - Function that takes a RetryConfigBuilder and returns a configured RetryConfig
    pub fn retry_config<F>(mut self, config_fn: F) -> Self
    where
        F: FnOnce(RetryConfigBuilder) -> RetryConfigBuilder,
    {
        let builder = RetryConfigBuilder::default();
        let configured_builder = config_fn(builder);
        self.retry_config = Some(configured_builder.build());
        self
    }

    /// Sets a custom retry configuration directly.
    ///
    /// # Arguments
    /// * `config` - The retry configuration to use
    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = Some(config);
        self
    }

    /// Builds the configured GitHubClient.
    ///
    /// # Returns
    /// Returns a fully configured `GitHubClient` instance.
    ///
    /// # Errors
    /// Returns an error if the configuration is invalid or the client cannot be created.
    pub async fn build(self) -> GitHubResult<GitHubClient> {
        let mut client = GitHubClient::from_token(self.token, self.github_base_url).await?;

        if let Some(retry_config) = self.retry_config {
            client.retry_config = retry_config;
        }

        Ok(client)
    }
}

/// Builder for configuring installation-specific authentication.
///
/// This builder provides a fluent API for configuring installation-specific authentication
/// with custom settings for retry logic, enterprise support, and JWT configuration.
pub struct InstallationBuilder {
    app_id: u64,
    private_key: String,
    installation_id: u64,
    github_base_url: Option<String>,
    jwt_expiration_seconds: Option<u64>,
    token_refresh_buffer_seconds: Option<u64>,
    retry_config: Option<RetryConfig>,
}

impl InstallationBuilder {
    /// Creates a new installation builder.
    fn new(app_id: u64, private_key: String, installation_id: u64) -> Self {
        Self {
            app_id,
            private_key,
            installation_id,
            github_base_url: None,
            jwt_expiration_seconds: None,
            token_refresh_buffer_seconds: None,
            retry_config: None,
        }
    }

    /// Sets the GitHub Enterprise Server base URL.
    ///
    /// # Arguments
    /// * `base_url` - The base URL for GitHub Enterprise Server
    pub fn github_enterprise(mut self, base_url: impl Into<String>) -> Self {
        self.github_base_url = Some(base_url.into());
        self
    }

    /// Sets the JWT expiration time.
    ///
    /// # Arguments
    /// * `duration` - The JWT expiration duration (max 10 minutes)
    pub fn jwt_expiration(mut self, duration: Duration) -> Self {
        self.jwt_expiration_seconds = Some(duration.as_secs().min(600));
        self
    }

    /// Sets the token refresh buffer time.
    ///
    /// # Arguments
    /// * `duration` - The buffer time before token expiration to trigger refresh
    pub fn token_refresh_buffer(mut self, duration: Duration) -> Self {
        self.token_refresh_buffer_seconds = Some(duration.as_secs());
        self
    }

    /// Configures retry behavior using a configuration function.
    ///
    /// # Arguments
    /// * `config_fn` - Function that takes a RetryConfigBuilder and returns a configured RetryConfig
    pub fn retry_config<F>(mut self, config_fn: F) -> Self
    where
        F: FnOnce(RetryConfigBuilder) -> RetryConfigBuilder,
    {
        let builder = RetryConfigBuilder::default();
        let configured_builder = config_fn(builder);
        self.retry_config = Some(configured_builder.build());
        self
    }

    /// Sets a custom retry configuration directly.
    ///
    /// # Arguments
    /// * `config` - The retry configuration to use
    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = Some(config);
        self
    }

    /// Builds the configured GitHubClient.
    ///
    /// # Returns
    /// Returns a fully configured `GitHubClient` instance.
    ///
    /// # Errors
    /// Returns an error if the configuration is invalid or the client cannot be created.
    pub async fn build(self) -> GitHubResult<GitHubClient> {
        let mut config = AuthConfig::new(self.app_id, self.private_key, self.github_base_url)?;

        if let Some(jwt_expiration) = self.jwt_expiration_seconds {
            config.jwt_expiration_seconds = jwt_expiration;
        }

        if let Some(token_refresh_buffer) = self.token_refresh_buffer_seconds {
            config.token_refresh_buffer_seconds = token_refresh_buffer;
        }

        let auth_manager = GitHubAuthManager::new(config)?;
        let client = auth_manager
            .create_installation_client(self.installation_id)
            .await?;

        let github_client = GitHubClient {
            client,
            auth_manager: Some(auth_manager),
            retry_config: self.retry_config.unwrap_or_default(),
            rate_limit_state: Arc::new(Mutex::new(RateLimitState::default())),
            correlation_id: uuid::Uuid::new_v4().to_string(),
        };

        Ok(github_client)
    }
}

/// Builder for configuring retry behavior.
///
/// This builder provides a fluent API for configuring retry logic with
/// exponential backoff, maximum delays, and attempt limits.
pub struct RetryConfigBuilder {
    max_attempts: u32,
    base_delay: Duration,
    max_delay: Duration,
    backoff_factor: f64,
}

impl Default for RetryConfigBuilder {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_factor: 2.0,
        }
    }
}

impl RetryConfigBuilder {
    /// Sets the maximum number of retry attempts.
    ///
    /// # Arguments
    /// * `attempts` - The maximum number of attempts (including the initial attempt)
    pub fn max_attempts(mut self, attempts: u32) -> Self {
        self.max_attempts = attempts;
        self
    }

    /// Sets the base delay between retries.
    ///
    /// # Arguments
    /// * `delay` - The initial delay duration
    pub fn base_delay(mut self, delay: Duration) -> Self {
        self.base_delay = delay;
        self
    }

    /// Sets the maximum delay between retries.
    ///
    /// # Arguments
    /// * `delay` - The maximum delay duration
    pub fn max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// Sets the backoff multiplication factor.
    ///
    /// # Arguments
    /// * `factor` - The factor by which delay is multiplied after each retry
    pub fn backoff_factor(mut self, factor: f64) -> Self {
        self.backoff_factor = factor;
        self
    }

    /// Builds the retry configuration.
    ///
    /// # Returns
    /// Returns a `RetryConfig` with the configured settings.
    pub fn build(self) -> RetryConfig {
        RetryConfig {
            max_attempts: self.max_attempts,
            base_delay: self.base_delay,
            max_delay: self.max_delay,
            backoff_factor: self.backoff_factor,
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
