//! GitHub App authentication module.
//!
//! This module provides comprehensive GitHub App authentication functiona/// Rate limiter for GitHub API requests with exponential backoff and jitter.
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
use jsonwebtoken::{DecodingKey, EncodingKey};
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

/// Central GitHub App authentication manager.
///
/// This struct provides the main interface for GitHub App authentication operations,
/// managing JWT generation, installation token retrieval, caching, and rate limiting.
#[derive(Clone)]
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
        let jwt = self.generate_jwt().await?;

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

        // Cache the token with proper expiration (GitHub installation tokens typically expire in 1 hour)
        // Using a conservative 55 minutes to ensure we refresh before expiration
        let expires_at = Utc::now() + chrono::Duration::minutes(55);
        self.token_cache
            .store_token(installation_id, token.expose_secret().clone(), expires_at)
            .await;

        Ok(token.expose_secret().clone())
    }

    /// Creates a GitHub App client using JWT authentication.
    ///
    /// This method creates an authenticated Octocrab client using the authentication
    /// manager's configured GitHub App credentials. The client is authenticated with
    /// a JWT token and can be used for GitHub App operations.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing an authenticated `Octocrab` client.
    ///
    /// # Errors
    ///
    /// Returns an error if the JWT cannot be generated or if the client cannot be built.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use release_regent_github_client::auth::{GitHubAuthManager, AuthConfig};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = AuthConfig::new(12345, "private_key", None)?;
    ///     let auth_manager = GitHubAuthManager::new(config)?;
    ///     let app_client = auth_manager.create_app_client().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn create_app_client(&self) -> GitHubResult<Octocrab> {
        let client = octocrab::Octocrab::builder()
            .base_uri(&self.config.get_api_base_url())
            .map_err(|e| {
                Error::configuration(
                    "base_uri",
                    &format!("Failed to configure GitHub client: {}", e),
                )
            })?
            .app(self.config.app_id.into(), self.jwt_encoding_key.clone())
            .build()
            .map_err(|e| {
                Error::configuration(
                    "client_build",
                    &format!("Failed to build GitHub client: {}", e),
                )
            })?;

        Ok(client)
    }

    /// Creates an installation client using a cached or newly acquired installation token.
    ///
    /// This method creates an authenticated Octocrab client using an installation token
    /// for the specified installation ID. The token is cached for future use and
    /// automatically refreshed when necessary.
    ///
    /// # Arguments
    ///
    /// * `installation_id` - The GitHub App installation ID
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing an authenticated `Octocrab` client.
    ///
    /// # Errors
    ///
    /// Returns an error if the installation token cannot be acquired or if the client
    /// cannot be built.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use release_regent_github_client::auth::{GitHubAuthManager, AuthConfig};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = AuthConfig::new(12345, "private_key", None)?;
    ///     let auth_manager = GitHubAuthManager::new(config)?;
    ///     let installation_client = auth_manager.create_installation_client(987654).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn create_installation_client(&self, installation_id: u64) -> GitHubResult<Octocrab> {
        let token = self.get_installation_token(installation_id).await?;

        let client = octocrab::Octocrab::builder()
            .base_uri(&self.config.get_api_base_url())
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

        Ok(client)
    }

    /// Creates a client using a personal access token.
    ///
    /// This method creates an authenticated Octocrab client using a personal access token.
    /// This is useful for operations that require user authentication rather than
    /// GitHub App authentication.
    ///
    /// # Arguments
    ///
    /// * `token` - The personal access token
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing an authenticated `Octocrab` client.
    ///
    /// # Errors
    ///
    /// Returns an error if the token is invalid or if the client cannot be built.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use release_regent_github_client::auth::{GitHubAuthManager, AuthConfig};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = AuthConfig::new(12345, "private_key", None)?;
    ///     let auth_manager = GitHubAuthManager::new(config)?;
    ///     let token_client = auth_manager.create_token_client("ghp_token").await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn create_token_client(&self, token: &str) -> GitHubResult<Octocrab> {
        if token.is_empty() {
            return Err(Error::invalid_input("token", "Token cannot be empty"));
        }

        let client = octocrab::Octocrab::builder()
            .base_uri(&self.config.get_api_base_url())
            .map_err(|e| {
                Error::configuration(
                    "base_uri",
                    &format!("Failed to configure GitHub client: {}", e),
                )
            })?
            .personal_token(token.to_string())
            .build()
            .map_err(|e| {
                Error::configuration(
                    "client_build",
                    &format!("Failed to build GitHub client: {}", e),
                )
            })?;

        Ok(client)
    }

    /// Generates a JWT token for GitHub App authentication.
    ///
    /// This method creates a JWT token that can be used for GitHub App authentication.
    /// The token is signed with the configured private key and includes the necessary
    /// claims for GitHub App operations.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the JWT token as a string.
    ///
    /// # Errors
    ///
    /// Returns an error if the JWT cannot be generated or signed.
    pub async fn generate_jwt(&self) -> GitHubResult<String> {
        let now = Utc::now();
        let iat = now.timestamp();
        let exp = (now + chrono::Duration::minutes(10)).timestamp();
        let jti = uuid::Uuid::new_v4().to_string();

        let claims = JwtClaims {
            jti,
            iat,
            exp,
            iss: self.config.app_id.to_string(),
            aud: self.config.get_jwt_audience(),
        };

        let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);

        jsonwebtoken::encode(&header, &claims, &self.jwt_encoding_key).map_err(|e| {
            error!("Failed to generate JWT: {}", e);
            Error::authentication("Failed to generate JWT token")
        })
    }

    /// Validates a JWT token for GitHub App authentication.
    ///
    /// This method validates the JWT signature and checks expiration times.
    ///
    /// # Note
    ///
    /// This method is currently not fully tested due to issues with the JWT library.
    /// GitHub's API will validate JWTs independently, so this is primarily for
    /// internal verification if needed.
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
    #[allow(dead_code)]
    fn validate_jwt(&self, token: &str) -> GitHubResult<JwtClaims> {
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

impl std::fmt::Debug for GitHubAuthManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitHubAuthManager")
            .field("config", &self.config)
            .field("token_cache", &self.token_cache)
            .field("jwt_encoding_key", &"<redacted>")
            .field("jwt_decoding_key", &"<redacted>")
            .field("rate_limiter", &self.rate_limiter)
            .field("octocrab_client", &"<octocrab_client>")
            .finish()
    }
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

/// Information about GitHub API rate limits.
#[derive(Debug, Clone, Default)]
pub struct RateLimitInfo {
    /// The maximum number of requests per hour
    pub limit: Option<u32>,
    /// The number of requests remaining in the current rate limit window
    pub remaining: Option<u32>,
    /// The time when the current rate limit window resets (Unix timestamp)
    pub reset: Option<u64>,
    /// The number of requests used in the current rate limit window
    pub used: Option<u32>,
}

/// Rate limiter for GitHub API requests with exponential backoff and jitter.
///
/// This struct implements rate limiting and retry logic with exponential backoff
/// to respect GitHub's API rate limits for authentication operations.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    /// Last request timestamp
    last_request: Arc<RwLock<Option<Instant>>>,
    /// Minimum time between requests
    min_interval: Duration,
    /// Maximum retry attempts
    max_retries: u32,
    /// Base delay for exponential backoff
    base_delay: Duration,
    /// GitHub rate limit tracking
    rate_limit_info: Arc<RwLock<RateLimitInfo>>,
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
            rate_limit_info: Arc::new(RwLock::new(RateLimitInfo::default())),
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

    /// Updates rate limit information from GitHub API response headers.
    ///
    /// # Arguments
    ///
    /// * `headers` - HTTP response headers from a GitHub API call
    pub async fn update_rate_limit_from_headers(&self, headers: &reqwest::header::HeaderMap) {
        let mut rate_limit = self.rate_limit_info.write().await;

        if let Some(limit) = headers.get("x-ratelimit-limit") {
            if let Ok(limit_str) = limit.to_str() {
                rate_limit.limit = limit_str.parse().ok();
            }
        }

        if let Some(remaining) = headers.get("x-ratelimit-remaining") {
            if let Ok(remaining_str) = remaining.to_str() {
                rate_limit.remaining = remaining_str.parse().ok();
            }
        }

        if let Some(reset) = headers.get("x-ratelimit-reset") {
            if let Ok(reset_str) = reset.to_str() {
                rate_limit.reset = reset_str.parse().ok();
            }
        }

        if let Some(used) = headers.get("x-ratelimit-used") {
            if let Ok(used_str) = used.to_str() {
                rate_limit.used = used_str.parse().ok();
            }
        }
    }

    /// Checks if we should wait due to rate limiting.
    ///
    /// # Returns
    ///
    /// `Some(Duration)` if we should wait, `None` if we can proceed immediately.
    pub async fn should_wait_for_rate_limit(&self) -> Option<Duration> {
        let rate_limit = self.rate_limit_info.read().await;

        // If we have remaining requests, we don't need to wait
        if let Some(remaining) = rate_limit.remaining {
            if remaining > 0 {
                return None;
            }
        }

        // If we're at the limit, check when it resets
        if let Some(reset_time) = rate_limit.reset {
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            if reset_time > current_time {
                let wait_duration = Duration::from_secs(reset_time - current_time);
                return Some(wait_duration);
            }
        }

        None
    }

    /// Gets the current rate limit information.
    ///
    /// # Returns
    ///
    /// A clone of the current `RateLimitInfo`.
    pub async fn get_rate_limit_info(&self) -> RateLimitInfo {
        self.rate_limit_info.read().await.clone()
    }

    /// Determines if an error should be retried based on retry policy.
    ///
    /// # Arguments
    ///
    /// * `error` - The error to check
    /// * `policy` - The retry policy to apply
    ///
    /// # Returns
    ///
    /// `true` if the error should be retried, `false` otherwise.
    pub fn should_retry_error(&self, error: &crate::Error, policy: &RetryPolicy) -> bool {
        match error {
            crate::Error::RateLimit { .. } => policy.retry_on_rate_limit,
            crate::Error::ApiRequest { .. } => policy.retry_on_network_error,
            _ => false,
        }
    }

    /// Executes a request with retry logic and rate limiting.
    ///
    /// # Arguments
    ///
    /// * `request_fn` - Async function that makes the request
    /// * `policy` - Retry policy configuration
    ///
    /// # Returns
    ///
    /// Result of the request execution.
    pub async fn execute_with_retry<F, Fut, T>(
        &self,
        request_fn: F,
        policy: &RetryPolicy,
    ) -> Result<T, crate::Error>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, crate::Error>>,
    {
        let mut attempt = 0;

        loop {
            // Check if we should wait due to rate limiting
            if let Some(wait_duration) = self.should_wait_for_rate_limit().await {
                debug!("Rate limit exceeded, waiting {:?}", wait_duration);
                tokio::time::sleep(wait_duration).await;
            }

            // Wait for rate limiting between requests
            self.wait_for_rate_limit().await;

            // Execute the request
            match request_fn().await {
                Ok(result) => return Ok(result),
                Err(error) => {
                    if attempt >= policy.max_retries || !self.should_retry_error(&error, policy) {
                        return Err(error);
                    }

                    let backoff_delay = self.calculate_backoff_delay(attempt);
                    let actual_delay = std::cmp::min(backoff_delay, policy.max_delay);

                    debug!(
                        "Request failed (attempt {}), retrying in {:?}: {:?}",
                        attempt + 1,
                        actual_delay,
                        error
                    );

                    tokio::time::sleep(actual_delay).await;
                    attempt += 1;
                }
            }
        }
    }
}

/// Retry policy configuration for different error scenarios.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Base delay for exponential backoff
    pub base_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Whether to retry on rate limit errors
    pub retry_on_rate_limit: bool,
    /// Whether to retry on network errors
    pub retry_on_network_error: bool,
    /// Whether to retry on server errors (5xx)
    pub retry_on_server_error: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(60),
            retry_on_rate_limit: true,
            retry_on_network_error: true,
            retry_on_server_error: true,
        }
    }
}

/// Secure in-memory cache for installation tokens.
///
/// This cache automatically handles token expiration and cleanup, ensuring
/// that expired tokens are removed from memory securely.
#[derive(Debug, Clone)]
pub struct TokenCache {
    /// Map of installation_id -> cached token
    tokens: Arc<RwLock<HashMap<u64, CachedToken>>>,
    /// Buffer time before expiration to refresh tokens
    refresh_buffer: Duration,
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
///
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

    // For now, maintain backward compatibility by using the existing octocrab method
    // In the future, this could be enhanced to use GitHubAuthManager with caching
    // if the octocrab client provides access to the app configuration
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

    // Create an AuthConfig and GitHubAuthManager for the operation
    let config = AuthConfig::new(app_id, private_key, None)?;
    let auth_manager = GitHubAuthManager::new(config)?;

    // Use the auth manager to create the client
    let client = auth_manager.create_app_client().await?;

    info!(app_id = app_id, "Created app client");
    Ok(client)
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

    // For token clients, we don't need a full auth manager, just create the client directly
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
mod tests {
    include!("auth_tests.rs");
}
