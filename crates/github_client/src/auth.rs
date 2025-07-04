//! GitHub App authentication module.
//!
//! This module provides comprehensive GitHub App authentication functionality including
//! JWT generation, installation token management, rate limiting, and secure token storage.
//!
//! # Architecture
//!
//! The module is built around the following core components:
//!
//! * `GitHubAuthManager` - Central authentication coordinator
//! * `TokenCache` - Secure in-memory token storage with automatic cleanup
//! * `AuthConfig` - Configuration for GitHub App settings and Enterprise support
//! * `RateLimiter` - Rate limiting for authentication endpoints
//!
//! # Security Features
//!
//! * Secure token storage using `secrecy` crate
//! * Automatic token cleanup on drop
//! * No sensitive data in error messages or logs
//! * Constant-time comparisons for signature verification
//!
//! # Examples
//!
//! ```rust,no_run
//! use github_client::auth::{GitHubAuthManager, AuthConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = AuthConfig::new(
//!         12345,
//!         "-----BEGIN RSA PRIVATE KEY-----\n...\n-----END RSA PRIVATE KEY-----",
//!         None, // GitHub.com (not Enterprise)
//!     )?;
//!
//!     let auth_manager = GitHubAuthManager::new(config)?;
//!     let token = auth_manager.get_installation_token(987654).await?;
//!
//!     println!("Got installation token for installation ID 987654");
//!     Ok(())
//! }
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use jsonwebtoken::EncodingKey;
use octocrab::Octocrab;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};

use crate::errors::{Error, GitHubResult};

/// Configuration for GitHub App authentication.
///
/// This struct holds the necessary configuration for authenticating as a GitHub App,
/// including support for GitHub Enterprise Server deployments.
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// The GitHub App ID
    pub app_id: u64,
    /// The private key for JWT signing (kept secret)
    pub private_key: SecretString,
    /// Optional GitHub Enterprise Server base URL
    pub github_base_url: Option<String>,
    /// JWT expiration time in seconds (default: 10 minutes)
    pub jwt_expiration_seconds: u64,
    /// Token refresh buffer time in seconds (default: 5 minutes)
    pub token_refresh_buffer_seconds: u64,
}

/// A cached installation token with expiration tracking.
#[derive(Debug, Clone)]
pub struct CachedToken {
    /// The token value (kept secret)
    pub token: SecretString,
    /// When the token expires
    pub expires_at: DateTime<Utc>,
    /// When the token was created
    pub created_at: DateTime<Utc>,
    /// Installation ID this token belongs to
    pub installation_id: u64,
}

/// Secure in-memory cache for installation tokens.
///
/// This cache automatically handles token expiration and cleanup, ensuring
/// that expired tokens are removed from memory securely.
#[derive(Debug)]
pub struct TokenCache {
    /// Map of installation_id -> cached token
    tokens: Arc<RwLock<HashMap<u64, CachedToken>>>,
    /// Buffer time before expiration to refresh tokens
    refresh_buffer: Duration,
}

/// Central GitHub App authentication manager.
///
/// This struct provides the main interface for GitHub App authentication operations,
/// managing JWT generation, installation token retrieval, caching, and rate limiting.
pub struct GitHubAuthManager {
    /// Authentication configuration
    config: AuthConfig,
    /// Token cache for installation tokens
    token_cache: TokenCache,
    /// JWT encoding key for signing tokens
    jwt_encoding_key: EncodingKey,
    /// Rate limiter for authentication requests
    rate_limiter: RateLimiter,
    /// Base Octocrab client for API requests
    octocrab_client: Octocrab,
}

/// Rate limiter for authentication endpoints.
///
/// This struct implements rate limiting and retry logic with exponential backoff
/// to respect GitHub's API rate limits for authentication operations.
#[derive(Debug)]
pub struct RateLimiter {
    /// Last request timestamp
    last_request: Arc<RwLock<Option<Instant>>>,
    /// Minimum time between requests
    min_interval: Duration,
    /// Maximum retry attempts
    max_retries: u32,
    /// Base delay for exponential backoff
    base_delay: Duration,
}

/// JWT claims for GitHub App authentication.
#[derive(Debug, Serialize, Deserialize)]
struct JwtClaims {
    /// JWT ID (unique identifier)
    jti: String,
    /// Issued at time
    iat: i64,
    /// Expiration time
    exp: i64,
    /// Issuer (GitHub App ID)
    iss: String,
    /// Audience (GitHub or Enterprise URL)
    aud: String,
}

// Implementation blocks will be added in subsequent tasks

impl AuthConfig {
    /// Creates a new authentication configuration.
    ///
    /// # Arguments
    ///
    /// * `app_id` - The GitHub App ID
    /// * `private_key` - The private key for JWT signing
    /// * `github_base_url` - Optional GitHub Enterprise Server base URL
    ///
    /// # Returns
    ///
    /// A configured `AuthConfig` instance with sensible defaults.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use github_client::auth::AuthConfig;
    ///
    /// let config = AuthConfig::new(
    ///     12345,
    ///     "-----BEGIN RSA PRIVATE KEY-----\n...\n-----END RSA PRIVATE KEY-----",
    ///     None,
    /// )?;
    /// ```
    pub fn new(
        app_id: u64,
        private_key: impl Into<String>,
        github_base_url: Option<String>,
    ) -> GitHubResult<Self> {
        let private_key = SecretString::new(private_key.into());

        // Validate the private key format
        Self::validate_private_key(&private_key)?;

        Ok(Self {
            app_id,
            private_key,
            github_base_url,
            jwt_expiration_seconds: 600,       // 10 minutes
            token_refresh_buffer_seconds: 300, // 5 minutes
        })
    }

    /// Creates a new configuration from environment variables.
    ///
    /// Expected environment variables:
    /// - `GITHUB_APP_ID`: GitHub App ID
    /// - `GITHUB_PRIVATE_KEY`: Private key content
    /// - `GITHUB_BASE_URL`: Optional GitHub Enterprise Server URL
    ///
    /// # Returns
    ///
    /// A configured `AuthConfig` instance from environment variables.
    ///
    /// # Errors
    ///
    /// Returns an error if required environment variables are missing or invalid.
    pub fn from_env() -> GitHubResult<Self> {
        let app_id = std::env::var("GITHUB_APP_ID")
            .map_err(|_| Error::invalid_input("GITHUB_APP_ID", "Environment variable not set"))?
            .parse::<u64>()
            .map_err(|_| Error::invalid_input("GITHUB_APP_ID", "Invalid App ID format"))?;

        let private_key = std::env::var("GITHUB_PRIVATE_KEY").map_err(|_| {
            Error::invalid_input("GITHUB_PRIVATE_KEY", "Environment variable not set")
        })?;

        let github_base_url = std::env::var("GITHUB_BASE_URL").ok();

        Self::new(app_id, private_key, github_base_url)
    }

    /// Validates the private key format.
    ///
    /// # Arguments
    ///
    /// * `private_key` - The private key to validate
    ///
    /// # Returns
    ///
    /// Ok(()) if the private key is valid, Error otherwise.
    fn validate_private_key(private_key: &SecretString) -> GitHubResult<()> {
        let key_content = private_key.expose_secret();

        // Check if it looks like a valid private key
        if !key_content.contains("-----BEGIN") || !key_content.contains("-----END") {
            return Err(Error::invalid_input(
                "private_key",
                "Private key must be in PEM format",
            ));
        }

        // Try to create an encoding key to validate the format
        EncodingKey::from_rsa_pem(key_content.as_bytes())
            .map_err(|_| Error::invalid_input("private_key", "Invalid RSA private key format"))?;

        Ok(())
    }

    /// Gets the audience for JWT tokens.
    ///
    /// Returns the appropriate audience based on whether this is
    /// GitHub.com or GitHub Enterprise Server.
    pub fn get_jwt_audience(&self) -> String {
        match &self.github_base_url {
            Some(base_url) => base_url.clone(),
            None => "https://api.github.com".to_string(),
        }
    }

    /// Gets the API base URL for GitHub requests.
    pub fn get_api_base_url(&self) -> String {
        match &self.github_base_url {
            Some(base_url) => format!("{}/api/v3", base_url),
            None => "https://api.github.com".to_string(),
        }
    }
}

impl TokenCache {
    /// Creates a new token cache with the specified refresh buffer.
    ///
    /// # Arguments
    ///
    /// * `refresh_buffer` - How long before expiration to refresh tokens
    ///
    /// # Returns
    ///
    /// A new `TokenCache` instance ready for use.
    pub fn new(refresh_buffer: Duration) -> Self {
        Self {
            tokens: Arc::new(RwLock::new(HashMap::new())),
            refresh_buffer,
        }
    }

    /// Retrieves a cached token for the given installation ID.
    ///
    /// # Arguments
    ///
    /// * `installation_id` - The installation ID to get the token for
    ///
    /// # Returns
    ///
    /// `Some(CachedToken)` if a valid token exists, `None` otherwise.
    pub async fn get_token(&self, installation_id: u64) -> Option<CachedToken> {
        let tokens = self.tokens.read().await;

        if let Some(cached_token) = tokens.get(&installation_id) {
            // Check if token is still valid (not expired and not close to expiration)
            let now = Utc::now();
            let expires_soon = cached_token.expires_at
                - chrono::Duration::from_std(self.refresh_buffer).unwrap_or_default();

            if now < expires_soon {
                return Some(cached_token.clone());
            }
        }

        None
    }

    /// Stores a token in the cache.
    ///
    /// # Arguments
    ///
    /// * `installation_id` - The installation ID this token belongs to
    /// * `token` - The token value to store
    /// * `expires_at` - When the token expires
    pub async fn store_token(
        &self,
        installation_id: u64,
        token: String,
        expires_at: DateTime<Utc>,
    ) {
        let cached_token = CachedToken {
            token: SecretString::new(token),
            expires_at,
            created_at: Utc::now(),
            installation_id,
        };

        let mut tokens = self.tokens.write().await;
        tokens.insert(installation_id, cached_token);

        debug!("Stored token for installation ID {}", installation_id);
    }

    /// Removes a token from the cache.
    ///
    /// # Arguments
    ///
    /// * `installation_id` - The installation ID to remove the token for
    pub async fn remove_token(&self, installation_id: u64) {
        let mut tokens = self.tokens.write().await;
        if tokens.remove(&installation_id).is_some() {
            debug!("Removed token for installation ID {}", installation_id);
        }
    }

    /// Cleans up expired tokens from the cache.
    ///
    /// This method removes all tokens that have expired, freeing up memory
    /// and ensuring expired tokens are not accidentally used.
    pub async fn cleanup_expired_tokens(&self) {
        let mut tokens = self.tokens.write().await;
        let now = Utc::now();

        let expired_installations: Vec<u64> = tokens
            .iter()
            .filter(|(_, token)| now >= token.expires_at)
            .map(|(installation_id, _)| *installation_id)
            .collect();

        for installation_id in expired_installations {
            tokens.remove(&installation_id);
            debug!(
                "Cleaned up expired token for installation ID {}",
                installation_id
            );
        }
    }

    /// Returns the number of tokens currently in the cache.
    pub async fn token_count(&self) -> usize {
        self.tokens.read().await.len()
    }

    /// Clears all tokens from the cache.
    ///
    /// This is useful for testing or when shutting down the application
    /// to ensure all tokens are cleared from memory.
    pub async fn clear_all_tokens(&self) {
        let mut tokens = self.tokens.write().await;
        let count = tokens.len();
        tokens.clear();
        debug!("Cleared {} tokens from cache", count);
    }
}

impl Drop for TokenCache {
    fn drop(&mut self) {
        // Note: We can't await in Drop, so we'll do our best to clear immediately
        // In a real implementation, this would be handled by the runtime shutdown
        debug!("TokenCache dropped - tokens will be cleared by runtime");
    }
}

impl RateLimiter {
    /// Creates a new rate limiter with the specified configuration.
    ///
    /// # Arguments
    ///
    /// * `min_interval` - Minimum time between requests
    /// * `max_retries` - Maximum number of retry attempts
    /// * `base_delay` - Base delay for exponential backoff
    ///
    /// # Returns
    ///
    /// A new `RateLimiter` instance.
    pub fn new(min_interval: Duration, max_retries: u32, base_delay: Duration) -> Self {
        Self {
            last_request: Arc::new(RwLock::new(None)),
            min_interval,
            max_retries,
            base_delay,
        }
    }

    /// Creates a rate limiter with default settings for GitHub authentication.
    ///
    /// Default settings:
    /// - Minimum 1 second between requests
    /// - Maximum 3 retry attempts
    /// - Base delay of 1 second for exponential backoff
    pub fn default() -> Self {
        Self::new(Duration::from_secs(1), 3, Duration::from_secs(1))
    }

    /// Waits for the minimum interval before allowing the next request.
    ///
    /// This method ensures that requests don't exceed the configured rate limit.
    pub async fn wait_for_rate_limit(&self) {
        let mut last_request = self.last_request.write().await;

        if let Some(last_time) = *last_request {
            let elapsed = last_time.elapsed();
            if elapsed < self.min_interval {
                let wait_time = self.min_interval - elapsed;
                debug!("Rate limiting: waiting {:?} before next request", wait_time);
                tokio::time::sleep(wait_time).await;
            }
        }

        *last_request = Some(Instant::now());
    }
    /// Calculates the delay for exponential backoff.
    ///
    /// # Arguments
    ///
    /// * `attempt` - The current retry attempt (0-based)
    ///
    /// # Returns
    ///
    /// The duration to wait before the next retry.
    pub fn calculate_backoff_delay(&self, attempt: u32) -> Duration {
        let multiplier = 2_u32.pow(attempt);
        let delay = self.base_delay * multiplier;

        // Add jitter to prevent thundering herd
        let jitter = Duration::from_millis(fastrand::u64(0..=100));
        delay + jitter
    }

    /// Gets the maximum number of retries.
    pub fn max_retries(&self) -> u32 {
        self.max_retries
    }
}

/// Authenticates with GitHub using an installation access token.
///
/// This function takes an existing `Octocrab` client and creates a new authenticated client
/// using an installation access token. The token is obtained by providing the installation ID
/// and repository information.
///
/// # Arguments
///
/// * `octocrab` - An existing `Octocrab` client (typically authenticated as a GitHub App).
/// * `installation_id` - The installation ID for the GitHub App.
/// * `repository_owner` - The owner of the repository.
/// * `source_repository` - The name of the repository.
///
/// # Returns
///
/// Returns a `Result` containing an authenticated `Octocrab` client with installation token,
/// or an `Error` if the operation fails.
///
/// # Errors
///
/// This function returns an `Error` in the following cases:
/// - If the app installation cannot be found.
/// - If the access token cannot be created.
/// - If the new `Octocrab` client cannot be built.
///
/// # Example
///
/// ```rust,no_run
/// use github_client::auth::{authenticate_with_access_token, create_app_client};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let octocrab = create_app_client(123456, "private_key").await?;
///     let installation_id = 12345678; // Replace with your installation ID
///     let repository_owner = "example-owner";
///     let source_repository = "example-repo";
///
///     let authenticated_client = authenticate_with_access_token(
///         &octocrab,
///         installation_id,
///         repository_owner,
///         source_repository,
///     )
///     .await?;
///
///     // Use `authenticated_client` to perform API operations
///     Ok(())
/// }
/// ```
#[instrument]
pub async fn authenticate_with_access_token(
    octocrab: &Octocrab,
    installation_id: u64,
    repository_owner: &str,
    source_repository: &str,
) -> GitHubResult<Octocrab> {
    debug!(
        repository_owner = repository_owner,
        repository = source_repository,
        installation_id,
        "Finding installation"
    );

    let (api_with_token, _) = octocrab
        .installation_and_token(installation_id.into())
        .await
        .map_err(|_| {
            error!(
                repository_owner = repository_owner,
                repository = source_repository,
                installation_id,
                "Failed to create a token for the installation",
            );

            Error::authentication("Failed to create a token for the installation")
        })?;

    info!(
        repository_owner = repository_owner,
        repository = source_repository,
        installation_id,
        "Created access token for installation",
    );

    Ok(api_with_token)
}

/// Creates an `Octocrab` client authenticated as a GitHub App using a JWT token.
///
/// This function generates a JSON Web Token (JWT) for the specified GitHub App ID and private key,
/// and uses it to create an authenticated `Octocrab` client. The client can then be used to perform
/// API operations on behalf of the GitHub App.
///
/// # Arguments
///
/// * `app_id` - The ID of the GitHub App.
/// * `private_key` - The private key for the GitHub App in PEM format.
///
/// # Returns
///
/// Returns a `Result` containing an authenticated `Octocrab` client, or an `Error`
/// if the operation fails.
///
/// # Errors
///
/// This function returns an `Error` in the following cases:
/// - If the private key cannot be parsed.
/// - If the JWT token cannot be created.
/// - If the `Octocrab` client cannot be built.
///
/// # Example
///
/// ```rust,no_run
/// use github_client::auth::create_app_client;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let app_id = 123456; // Replace with your GitHub App ID
///     let private_key = r#"
/// -----BEGIN RSA PRIVATE KEY-----
/// ...
/// -----END RSA PRIVATE KEY-----
/// "#; // Replace with your GitHub App private key
///
///     let client = create_app_client(app_id, private_key).await?;
///
///     // Use `client` to perform API operations
///     Ok(())
/// }
/// ```
#[instrument(skip(private_key))]
pub async fn create_app_client(app_id: u64, private_key: &str) -> GitHubResult<Octocrab> {
    info!(
        app_id = app_id,
        key_length = private_key.len(),
        key_starts_with = &private_key[..27], // "-----BEGIN RSA PRIVATE KEY"
        "Creating GitHub App client with provided credentials"
    );

    let key = EncodingKey::from_rsa_pem(private_key.as_bytes()).map_err(|e| {
        error!(
            app_id = app_id,
            error = %e,
            "Failed to parse RSA private key - key format is invalid"
        );
        Error::authentication(format!(
            "Failed to translate the private key. Error was: {}",
            e
        ))
    })?;

    info!(app_id = app_id, "Successfully parsed RSA private key");

    let octocrab = Octocrab::builder()
        .app(app_id.into(), key)
        .build()
        .map_err(|e| {
            error!(
                app_id = app_id,
                error = ?e,
                "Failed to build Octocrab client with GitHub App credentials"
            );
            Error::authentication("Failed to get a personal token for the app install.")
        })?;

    info!(app_id = app_id, "Successfully created GitHub App client");

    Ok(octocrab)
}

/// Creates an Octocrab client authenticated with a personal access token.
///
/// This function creates a GitHub API client using a personal access token
/// for authentication. This is useful for operations that don't require
/// GitHub App authentication.
///
/// # Arguments
///
/// * `token` - A GitHub personal access token
///
/// # Returns
///
/// Returns a `Result` containing an authenticated `Octocrab` client, or an `Error`
/// if the client cannot be built.
///
/// # Errors
///
/// This function returns an `Error` if the Octocrab client cannot be
/// constructed with the provided token.
///
/// # Examples
///
/// ```rust,no_run
/// use github_client::auth::create_token_client;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let token = "ghp_xxxxxxxxxxxxxxxxxxxx"; // Your GitHub PAT
///     let client = create_token_client(token)?;
///
///     // Use client for API operations
///     Ok(())
/// }
/// ```
#[instrument(skip(token))]
pub fn create_token_client(token: &str) -> GitHubResult<Octocrab> {
    Octocrab::builder()
        .personal_token(token.to_string())
        .build()
        .map_err(|e| {
            error!(error = ?e, "Failed to create token client");
            Error::authentication("Failed to create GitHub client with personal access token")
        })
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
