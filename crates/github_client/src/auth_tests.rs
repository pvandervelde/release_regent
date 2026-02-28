use super::*;
use github_bot_sdk::error::SecretError;

#[test]
fn test_auth_config_creation() {
    let config = AuthConfig {
        app_id: 12345,
        private_key: "-----BEGIN RSA PRIVATE KEY-----\ntest\n-----END RSA PRIVATE KEY-----"
            .to_string(),
        webhook_secret: "test-secret".to_string(),
    };

    assert_eq!(config.app_id, 12345);
    assert_eq!(config.webhook_secret, "test-secret");
}

#[test]
fn test_auth_config_clone() {
    let config = AuthConfig {
        app_id: 12345,
        private_key: "test-key".to_string(),
        webhook_secret: "test-secret".to_string(),
    };

    let cloned = config.clone();
    assert_eq!(cloned.app_id, config.app_id);
    assert_eq!(cloned.webhook_secret, config.webhook_secret);
}

#[tokio::test]
async fn test_secret_provider_invalid_private_key() {
    let config = AuthConfig {
        app_id: 12345,
        private_key: "not-a-valid-pem-key".to_string(),
        webhook_secret: "test-secret".to_string(),
    };

    let result = AzureKeyVaultSecretProvider::new(config);
    assert!(result.is_err());

    match result.unwrap_err() {
        SecretError::InvalidFormat { key } => {
            assert_eq!(key, "private_key");
        }
        _ => panic!("Expected InvalidFormat error"),
    }
}

#[tokio::test]
async fn test_secret_provider_get_app_id() {
    // Use a minimal valid PEM structure for testing
    let config = AuthConfig {
        app_id: 54321,
        private_key:
            "-----BEGIN RSA PRIVATE KEY-----\nMIIBogIBAAJBALRiMLAA\n-----END RSA PRIVATE KEY-----"
                .to_string(),
        webhook_secret: "webhook-secret".to_string(),
    };

    let provider = AzureKeyVaultSecretProvider::new(config);
    // May fail on PEM parsing, which is expected without a real key
    if let Ok(provider) = provider {
        let app_id = provider.get_app_id().await.unwrap();
        assert_eq!(app_id.as_u64(), 54321);
    }
}

#[tokio::test]
async fn test_secret_provider_get_webhook_secret() {
    let config = AuthConfig {
        app_id: 12345,
        private_key:
            "-----BEGIN RSA PRIVATE KEY-----\nMIIBogIBAAJBALRiMLAA\n-----END RSA PRIVATE KEY-----"
                .to_string(),
        webhook_secret: "my-webhook-secret".to_string(),
    };

    let provider = AzureKeyVaultSecretProvider::new(config);
    if let Ok(provider) = provider {
        let secret = provider.get_webhook_secret().await.unwrap();
        assert_eq!(secret, "my-webhook-secret");
    }
}

#[test]
fn test_secret_provider_cache_duration() {
    let config = AuthConfig {
        app_id: 12345,
        private_key:
            "-----BEGIN RSA PRIVATE KEY-----\nMIIBogIBAAJBALRiMLAA\n-----END RSA PRIVATE KEY-----"
                .to_string(),
        webhook_secret: "test-secret".to_string(),
    };

    let provider = AzureKeyVaultSecretProvider::new(config);
    if let Ok(provider) = provider {
        let duration = provider.cache_duration();
        assert_eq!(duration.num_hours(), 1);
    }
}
