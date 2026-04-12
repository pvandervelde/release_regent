//! Authentication module for github-bot-sdk integration
//!
//! Provides [`SecretProvider`], [`JwtSigner`], and [`GitHubApiClient`] implementations
//! for use with github-bot-sdk in production deployments.

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use github_bot_sdk::{
    auth::{
        GitHubApiClient, GitHubAppId, Installation, InstallationId, InstallationPermissions,
        InstallationToken, JsonWebToken, JwtClaims, JwtSigner, PrivateKey, RateLimitInfo,
        Repository, RepositoryId, SecretProvider,
    },
    error::{ApiError, SecretError, SigningError, ValidationError},
};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use tracing::debug;

/// Configuration for GitHub App authentication
#[derive(Clone)]
pub struct AuthConfig {
    /// GitHub App ID
    pub app_id: u64,
    /// Private key in PEM format
    pub private_key: String,
    /// Webhook secret for signature validation
    pub webhook_secret: String,
}

/// Environment-variable-based secret provider.
///
/// Implements the [`SecretProvider`] trait for github-bot-sdk by returning values
/// that were loaded from environment variables at process startup and passed in via
/// [`AuthConfig`].
///
/// # Deployment model
///
/// Release Regent runs as a container. The container runtime (Azure Container Apps,
/// AKS + CSI Driver, AWS ECS, etc.) injects secrets as environment variables before
/// the process starts. This struct simply holds those pre-loaded values — there is no
/// need for the application to call a secret-management API at runtime.
#[derive(Debug, Clone)]
pub struct EnvSecretProvider {
    app_id: GitHubAppId,
    private_key: PrivateKey,
    webhook_secret: String,
}

impl EnvSecretProvider {
    /// Create a new environment secret provider from pre-loaded [`AuthConfig`] values.
    ///
    /// # Errors
    ///
    /// Returns [`SecretError::InvalidFormat`] if the private key in `config` is not
    /// a valid PEM-encoded RSA private key.
    pub fn new(config: AuthConfig) -> Result<Self, SecretError> {
        let app_id = GitHubAppId::new(config.app_id);

        // Parse the private key
        let private_key =
            PrivateKey::from_pem(&config.private_key).map_err(|_e| SecretError::InvalidFormat {
                key: "private_key".to_string(),
            })?;

        Ok(Self {
            app_id,
            private_key,
            webhook_secret: config.webhook_secret,
        })
    }
}

#[async_trait]
impl SecretProvider for EnvSecretProvider {
    async fn get_private_key(&self) -> Result<PrivateKey, SecretError> {
        debug!(provider = "env", "Retrieving private key");
        Ok(self.private_key.clone())
    }

    async fn get_app_id(&self) -> Result<GitHubAppId, SecretError> {
        debug!(provider = "env", "Retrieving app ID");
        Ok(self.app_id)
    }

    async fn get_webhook_secret(&self) -> Result<String, SecretError> {
        debug!(provider = "env", "Retrieving webhook secret");
        Ok(self.webhook_secret.clone())
    }

    fn cache_duration(&self) -> Duration {
        Duration::hours(1)
    }
}

// ============================================================================
// DefaultJwtSigner
// ============================================================================

/// Default JWT signer using RS256 algorithm.
///
/// Implements [`JwtSigner`] using the `jsonwebtoken` crate to produce RS256-signed
/// JWTs suitable for GitHub App authentication.
#[derive(Debug, Clone)]
pub struct DefaultJwtSigner;

impl DefaultJwtSigner {
    /// Create a new [`DefaultJwtSigner`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for DefaultJwtSigner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl JwtSigner for DefaultJwtSigner {
    async fn sign_jwt(
        &self,
        claims: JwtClaims,
        private_key: &PrivateKey,
    ) -> Result<JsonWebToken, SigningError> {
        let encoding_key = EncodingKey::from_rsa_pem(private_key.key_data()).map_err(|e| {
            SigningError::InvalidKey {
                message: format!("Failed to create encoding key: {e}"),
            }
        })?;

        let app_id = claims.iss;
        let exp = claims.exp;
        let header = Header::new(Algorithm::RS256);

        let token_string =
            encode(&header, &claims, &encoding_key).map_err(|e| SigningError::EncodingFailed {
                message: format!("Failed to encode JWT: {e}"),
            })?;

        let expires_at = DateTime::from_timestamp(exp, 0).unwrap_or_else(Utc::now);
        Ok(JsonWebToken::new(token_string, app_id, expires_at))
    }

    fn validate_private_key(&self, key: &PrivateKey) -> Result<(), ValidationError> {
        EncodingKey::from_rsa_pem(key.key_data())
            .map(|_| ())
            .map_err(|e| ValidationError::InvalidFormat {
                field: "private_key".to_string(),
                message: format!("Invalid RSA private key: {e}"),
            })
    }
}

// ============================================================================
// DefaultGitHubApiClient
// ============================================================================

/// HTTP-based GitHub API client for authentication operations.
///
/// Implements [`GitHubApiClient`] using reqwest for the network calls needed
/// to exchange JWT tokens for installation access tokens.
#[derive(Debug, Clone)]
pub struct DefaultGitHubApiClient {
    http_client: reqwest::Client,
    api_base_url: String,
    user_agent: String,
}

impl DefaultGitHubApiClient {
    /// Create a new [`DefaultGitHubApiClient`] pointing at `https://api.github.com`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::new(),
            api_base_url: "https://api.github.com".to_string(),
            user_agent: "release-regent/0.1.0".to_string(),
        }
    }
}

impl Default for DefaultGitHubApiClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GitHubApiClient for DefaultGitHubApiClient {
    async fn create_installation_access_token(
        &self,
        installation_id: InstallationId,
        jwt: &JsonWebToken,
    ) -> Result<InstallationToken, ApiError> {
        #[derive(serde::Deserialize)]
        struct TokenResponse {
            token: String,
            expires_at: DateTime<Utc>,
        }

        let url = format!(
            "{}/app/installations/{}/access_tokens",
            self.api_base_url,
            installation_id.as_u64()
        );

        let bearer_token = format!("Bearer {}", jwt.token());
        let response = self
            .http_client
            .post(&url)
            .header("Authorization", bearer_token)
            .header("User-Agent", &self.user_agent)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .map_err(|e| ApiError::HttpError {
                status: 0,
                message: format!("Network error sending token request: {e}"),
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ApiError::HttpError {
                status,
                message: body,
            });
        }

        let token_response: TokenResponse =
            response.json().await.map_err(|e| ApiError::HttpError {
                status: 0,
                message: format!("Failed to parse token response: {e}"),
            })?;

        Ok(InstallationToken::new(
            token_response.token,
            installation_id,
            token_response.expires_at,
            InstallationPermissions::default(),
            vec![],
        ))
    }

    async fn list_app_installations(
        &self,
        _jwt: &JsonWebToken,
    ) -> Result<Vec<Installation>, ApiError> {
        // Not required for CLI production path
        Ok(vec![])
    }

    async fn list_installation_repositories(
        &self,
        _installation_id: InstallationId,
        _token: &InstallationToken,
    ) -> Result<Vec<Repository>, ApiError> {
        // Not required for CLI production path
        Ok(vec![])
    }

    async fn get_repository(
        &self,
        _repo_id: RepositoryId,
        _token: &InstallationToken,
    ) -> Result<Repository, ApiError> {
        Err(ApiError::NotFound)
    }

    async fn get_rate_limit(&self, _token: &InstallationToken) -> Result<RateLimitInfo, ApiError> {
        Err(ApiError::NotFound)
    }
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
