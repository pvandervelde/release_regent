//! Authentication module for github-bot-sdk integration
//!
//! Provides SecretProvider implementation for Azure Key Vault integration

use async_trait::async_trait;
use chrono::Duration;
use github_bot_sdk::{
    auth::{GitHubAppId, PrivateKey, SecretProvider},
    error::SecretError,
};
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

/// Azure Key Vault-based secret provider
///
/// Implements the SecretProvider trait for github-bot-sdk using Azure Key Vault
/// for secret storage and retrieval.
#[derive(Debug, Clone)]
pub struct AzureKeyVaultSecretProvider {
    app_id: GitHubAppId,
    private_key: PrivateKey,
    webhook_secret: String,
}

impl AzureKeyVaultSecretProvider {
    /// Create a new Azure Key Vault secret provider
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
impl SecretProvider for AzureKeyVaultSecretProvider {
    async fn get_private_key(&self) -> Result<PrivateKey, SecretError> {
        debug!("Retrieving private key");
        // In a real implementation, this would fetch from Azure Key Vault
        // For now, we return the cached key
        Ok(self.private_key.clone())
    }

    async fn get_app_id(&self) -> Result<GitHubAppId, SecretError> {
        debug!("Retrieving app ID");
        Ok(self.app_id)
    }

    async fn get_webhook_secret(&self) -> Result<String, SecretError> {
        debug!("Retrieving webhook secret");
        Ok(self.webhook_secret.clone())
    }

    fn cache_duration(&self) -> Duration {
        // Cache secrets for 1 hour
        Duration::hours(1)
    }
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
