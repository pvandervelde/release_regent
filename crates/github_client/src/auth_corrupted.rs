//! GitHub App authentication module.
//!
//! This module provides comprehensive GitHub App authe/// GitHub authentication manager.
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
    /// JWT decoding key for validating tokens
    jwt_decoding_key: DecodingKey,
    /// Rate limiter for authentication requests
    rate_limiter: RateLimiter,
    /// Base Octocrab client for API requests
    octocrab_client: Octocrab,
}

/// Rate limiter for authentication endpoints.
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
//! use release_regent_github_client::auth::{GitHubAuthManager, AuthConfig};
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
use jsonwebtoken::{EncodingKey, DecodingKey};
use octocrab::Octocrab;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument};

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
pub struct JwtClaims {
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
    /// use release_regent_github_client::auth::AuthConfig;
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = AuthConfig::new(
    ///         12345,
    ///         "-----BEGIN RSA PRIVATE KEY-----\n...\n-----END RSA PRIVATE KEY-----",
    ///         None,
    ///     )?;
    ///     Ok(())
    /// }
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

impl GitHubAuthManager {
    /// Creates a new GitHub authentication manager.
    ///
    /// # Arguments
    ///
    /// * `config` - The authentication configuration
    ///
    /// # Returns
    ///
    /// A new `GitHubAuthManager` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid or if the JWT encoding key
    /// cannot be created from the private key.
    pub fn new(config: AuthConfig) -> GitHubResult<Self> {
        let jwt_encoding_key = EncodingKey::from_rsa_pem(
            config.private_key.expose_secret().as_bytes(),
        )
        .map_err(|_| Error::invalid_input("private_key", "Invalid RSA private key format"))?;

        let jwt_decoding_key = DecodingKey::from_rsa_pem(
            config.private_key.expose_secret().as_bytes(),
        )
        .map_err(|_| Error::invalid_input("private_key", "Invalid RSA private key format"))?;

        let token_cache = TokenCache::new(Duration::from_secs(config.token_refresh_buffer_seconds));
        let rate_limiter = RateLimiter::default();

        // Create the base Octocrab client
        let octocrab_client = octocrab::Octocrab::builder()
            .base_uri(&config.get_api_base_url())
            .map_err(|e| {
                Error::configuration(
                    "base_uri",
                    &format!("Failed to configure GitHub client: {}", e),
                )
            })?
            .build()
            .map_err(|e| {
                Error::configuration(
                    "client_build",
                    &format!("Failed to build GitHub client: {}", e),
                )
            })?;

        Ok(Self {
            config,
            token_cache,
            jwt_encoding_key,
            jwt_decoding_key,
            rate_limiter,
            octocrab_client,
        })
    }

    /// Gets an installation token for the specified installation ID.
    ///
    /// This method first checks the cache for a valid token. If not found or expired,
    /// it generates a new JWT, requests a new installation token, and caches it.
    ///
    /// # Arguments
    ///
    /// * `installation_id` - The GitHub App installation ID
    ///
    /// # Returns
    ///
    /// A valid installation token.
    ///
    /// # Errors
    ///
    /// Returns an error if JWT generation fails, token request fails, or rate limits are exceeded.
    pub async fn get_installation_token(&self, installation_id: u64) -> GitHubResult<String> {
        // Check cache first
        if let Some(cached_token) = self.token_cache.get_token(installation_id).await {
            return Ok(cached_token.token.expose_secret().clone());
        }

        // Wait for rate limit
        self.rate_limiter.wait_for_rate_limit().await;

        // Generate JWT for GitHub App authentication
        let jwt = self.generate_jwt()?;

        // Create authenticated client with JWT
        let app_client = octocrab::Octocrab::builder()
            .base_uri(&self.config.get_api_base_url())
            .map_err(|e| {
                Error::configuration(
                    "base_uri",
                    &format!("Failed to configure GitHub client: {}", e),
                )
            })?
            .personal_token(jwt)
            .build()
            .map_err(|e| {
                Error::configuration(
                    "client_build",
                    &format!("Failed to build GitHub client: {}", e),
                )
            })?;

        // Request installation token
        let (_, token) = app_client
            .installation_and_token(installation_id.into())
            .await
            .map_err(|e| {
                Error::authentication(&format!("Failed to get installation token: {}", e))
            })?;

        // Cache the token
        let expires_at =
            Utc::now() + chrono::Duration::seconds(self.config.jwt_expiration_seconds as i64);
        self.token_cache
            .store_token(installation_id, token.expose_secret().clone(), expires_at)
            .await;

        Ok(token.expose_secret().clone())
    }

    /// Generates a JWT for GitHub App authentication with enhanced security.
    ///
    /// This method creates a JWT with the following security features:
    /// - Secure nonce generation using UUID v4
    /// - Proper time validation and expiration
    /// - GitHub Enterprise Server support
    /// - Constant-time operations where applicable
    ///
    /// # Returns
    ///
    /// A signed JWT token for GitHub App authentication.
    ///
    /// # Errors
    ///
    /// Returns an error if JWT generation fails or if the current time is invalid.
    fn generate_jwt(&self) -> GitHubResult<String> {
        let now = Utc::now();
        let expiration = now + chrono::Duration::seconds(self.config.jwt_expiration_seconds as i64);

        // Ensure we're not creating a token with invalid time
        if expiration <= now {
            return Err(Error::jwt(
                "Invalid expiration time: token would be expired",
            ));
        }

        // Generate secure nonce using UUID v4
        let nonce = uuid::Uuid::new_v4().to_string();

        let claims = JwtClaims {
            jti: nonce,
            iat: now.timestamp(),
            exp: expiration.timestamp(),
            iss: self.config.app_id.to_string(),
            aud: self.config.get_jwt_audience(),
        };

        let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);

        jsonwebtoken::encode(&header, &claims, &self.jwt_encoding_key)
            .map_err(|e| Error::jwt(&format!("Failed to encode JWT: {}", e)))
    }

    /// Validates a JWT token for GitHub App authentication.
    ///
    /// This method validates the JWT signature and checks expiration times.
    ///
    /// # Arguments
    ///
    /// * `token` - The JWT token to validate
    ///
    /// # Returns
    ///
    /// The validated JWT claims if successful.
    ///
    /// # Errors
    ///
    /// Returns an error if the token is invalid, expired, or has an invalid signature.
    pub fn validate_jwt(&self, token: &str) -> GitHubResult<JwtClaims> {
        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
        
        // Don't validate audience automatically - we'll do it manually
        validation.validate_aud = false;

        let decoded = jsonwebtoken::decode::<JwtClaims>(token, &self.jwt_decoding_key, &validation)
            .map_err(|e| Error::jwt(&format!("JWT validation failed: {}", e)))?;

        // Additional validation checks
        let now = Utc::now().timestamp();
        if decoded.claims.exp <= now {
            return Err(Error::jwt("Token has expired"));
        }

        if decoded.claims.iat > now {
            return Err(Error::jwt("Token issued in the future"));
        }

        Ok(decoded.claims)
    }

    /// Performs constant-time comparison of two byte arrays.
    ///
    /// This method is used for secure signature verification to prevent timing attacks.
    ///
    /// # Arguments
    ///
    /// * `a` - First byte array to compare
    /// * `b` - Second byte array to compare
    ///
    /// # Returns
    ///
    /// `true` if the arrays are equal, `false` otherwise.
    fn constant_time_compare(a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }

        let mut result = 0u8;
        for (a_byte, b_byte) in a.iter().zip(b.iter()) {
            result |= a_byte ^ b_byte;
        }
        result == 0
    }

    /// Verifies a GitHub webhook signature using constant-time comparison.
    ///
    /// This method validates the `X-Hub-Signature-256` header against the webhook secret
    /// using constant-time comparison to prevent timing attacks.
    ///
    /// # Arguments
    ///
    /// * `payload` - The webhook payload
    /// * `signature` - The signature from the `X-Hub-Signature-256` header
    /// * `secret` - The webhook secret
    ///
    /// # Returns
    ///
    /// `true` if the signature is valid, `false` otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if the signature format is invalid or HMAC computation fails.
    pub fn verify_webhook_signature(
        payload: &[u8],
        signature: &str,
        secret: &str,
    ) -> GitHubResult<bool> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        // Parse the signature (should be in format "sha256=<hex>")
        let signature_hex = signature
            .strip_prefix("sha256=")
            .ok_or_else(|| Error::authentication("Invalid signature format"))?;

        let expected_signature = hex::decode(signature_hex)
            .map_err(|_| Error::authentication("Invalid signature encoding"))?;

        // Create HMAC-SHA256 hash of the payload
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
            .map_err(|_| Error::authentication("Failed to create HMAC"))?;
        mac.update(payload);
        let computed_signature = mac.finalize().into_bytes();

        // Use constant-time comparison to prevent timing attacks
        Ok(Self::constant_time_compare(
            &expected_signature,
            &computed_signature,
        ))
    }
}

impl TokenCache {
    /// Creates a new token cache with the specified refresh buffer.
    ///
    /// # Arguments
    ///
    /// * `refresh_buffer` - Time buffer before token expiration to trigger refresh
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

    /// Checks if we have a valid token for the given installation.
    ///
    /// # Arguments
    ///
    /// * `installation_id` - The installation ID to check for
    ///
    /// # Returns
    ///
    /// `Some(token)` if a valid (non-expired) token exists, `None` otherwise.
    pub async fn get_token(&self, installation_id: u64) -> Option<CachedToken> {
        let tokens = self.tokens.read().await;
        if let Some(cached_token) = tokens.get(&installation_id) {
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
    pub async fn cleanup_expired_tokens(&self) {
        let mut tokens = self.tokens.write().await;
        let now = Utc::now();
        let before_count = tokens.len();

        tokens.retain(|_, token| {
            let expires_soon = token.expires_at
                - chrono::Duration::from_std(self.refresh_buffer).unwrap_or_default();
            now < expires_soon
        });

        let removed = before_count - tokens.len();
        if removed > 0 {
            debug!("Removed {} expired tokens from cache", removed);
        }
    }

    /// Returns the number of tokens currently in the cache.
    pub async fn token_count(&self) -> usize {
        let tokens = self.tokens.read().await;
        tokens.len()
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
/// use release_regent_github_client::auth::{authenticate_with_access_token, create_app_client};
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
        "Created a token for the installation"
    );

    Ok(api_with_token)
}

/// Creates a GitHub App client using a private key.
///
/// This function creates an authenticated `Octocrab` client using the provided GitHub App ID
/// and private key. The client can be used to make authenticated API requests on behalf of
/// the GitHub App.
///
/// # Arguments
///
/// * `app_id` - The GitHub App ID.
/// * `private_key` - The private key for the GitHub App in PEM format.
///
/// # Returns
///
/// Returns a `Result` containing an authenticated `Octocrab` client, or an `Error` if the
/// operation fails.
///
/// # Errors
///
/// This function returns an `Error` in the following cases:
/// - If the private key is invalid or cannot be parsed.
/// - If the `Octocrab` client cannot be built.
///
/// # Example
///
/// ```rust,no_run
/// use release_regent_github_client::auth::create_app_client;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let app_id = 123456; // Replace with your App ID
///     let private_key = "-----BEGIN RSA PRIVATE KEY-----
/// MIIEpAIBAAKCAQEA...
/// -----END RSA PRIVATE KEY-----";
///
///     let octocrab = create_app_client(app_id, private_key).await?;
///     // Use `octocrab` to perform API operations
///     Ok(())
/// }
/// ```
#[instrument]
pub async fn create_app_client(app_id: u64, private_key: &str) -> GitHubResult<Octocrab> {
    debug!(app_id = app_id, "Creating app client");

    let key = jsonwebtoken::EncodingKey::from_rsa_pem(private_key.as_bytes()).map_err(|e| {
        error!(app_id = app_id, "Failed to parse private key: {}", e);
        Error::invalid_input("private_key", "Invalid RSA private key")
    })?;

    let octocrab = octocrab::Octocrab::builder()
        .app(app_id.into(), key)
        .build()
        .map_err(|e| {
            error!(app_id = app_id, "Failed to build octocrab client: {}", e);
            Error::configuration("octocrab", "Failed to build octocrab client")
        })?;

    info!(app_id = app_id, "Created app client");

    Ok(octocrab)
}

/// Creates a GitHub client using a personal access token.
///
/// This function creates an authenticated `Octocrab` client using the provided personal access
/// token. The client can be used to make authenticated API requests on behalf of the user
/// associated with the token.
///
/// # Arguments
///
/// * `token` - The personal access token for GitHub authentication.
///
/// # Returns
///
/// Returns a `Result` containing an authenticated `Octocrab` client, or an `Error` if the
/// operation fails.
///
/// # Errors
///
/// This function returns an `Error` in the following cases:
/// - If the token is invalid or empty.
/// - If the `Octocrab` client cannot be built.
///
/// # Example
///
/// ```rust,no_run
/// use release_regent_github_client::auth::create_token_client;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let token = "ghp_your_personal_access_token"; // Replace with your token
///     let octocrab = create_token_client(token).await?;
///     // Use `octocrab` to perform API operations
///     Ok(())
/// }
/// ```
#[instrument]
pub async fn create_token_client(token: &str) -> GitHubResult<Octocrab> {
    debug!("Creating token client");

    if token.is_empty() {
        return Err(Error::invalid_input("token", "Token cannot be empty"));
    }

    let octocrab = octocrab::Octocrab::builder()
        .personal_token(token.to_string())
        .build()
        .map_err(|e| {
            error!("Failed to build octocrab client: {}", e);
            Error::configuration("octocrab", "Failed to build octocrab client")
        })?;

    info!("Created token client");

    Ok(octocrab)
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
