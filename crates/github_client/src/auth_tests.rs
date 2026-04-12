use super::*;
use github_bot_sdk::auth::GitHubAppId;
use github_bot_sdk::error::{SecretError, SigningError};

/// A valid RSA-2048 test private key used only in tests.
///
/// This key is a development/testing artefact. It is not registered as a
/// GitHub App key and grants no access to any real system.
const TEST_RSA_PRIVATE_KEY: &str = include_str!("../test_key.pem");

// ─────────────────────────────────────────────────────────────────────────────
// AuthConfig
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_auth_config_create_with_valid_fields_stores_values() {
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
fn test_auth_config_clone_produces_equal_values() {
    let config = AuthConfig {
        app_id: 12345,
        private_key: "test-key".to_string(),
        webhook_secret: "test-secret".to_string(),
    };

    let cloned = config.clone();
    assert_eq!(cloned.app_id, config.app_id);
    assert_eq!(cloned.webhook_secret, config.webhook_secret);
}

// ─────────────────────────────────────────────────────────────────────────────
// EnvSecretProvider
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_env_secret_provider_new_with_invalid_pem_returns_invalid_format_error() {
    let config = AuthConfig {
        app_id: 12345,
        private_key: "not-a-valid-pem-key".to_string(),
        webhook_secret: "test-secret".to_string(),
    };

    let result = EnvSecretProvider::new(config);

    assert!(result.is_err());
    match result.unwrap_err() {
        SecretError::InvalidFormat { key } => {
            assert_eq!(key, "private_key");
        }
        other => panic!("Expected InvalidFormat error, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_env_secret_provider_get_app_id_returns_configured_app_id() {
    let config = AuthConfig {
        app_id: 54321,
        private_key: TEST_RSA_PRIVATE_KEY.to_string(),
        webhook_secret: "webhook-secret".to_string(),
    };

    let provider = EnvSecretProvider::new(config).expect("valid PEM key should succeed");
    let app_id = provider
        .get_app_id()
        .await
        .expect("get_app_id should succeed");

    assert_eq!(app_id.as_u64(), 54321);
}

#[tokio::test]
async fn test_env_secret_provider_get_private_key_returns_configured_key() {
    let config = AuthConfig {
        app_id: 12345,
        private_key: TEST_RSA_PRIVATE_KEY.to_string(),
        webhook_secret: "test-secret".to_string(),
    };

    let provider = EnvSecretProvider::new(config).expect("valid PEM key should succeed");
    let key = provider
        .get_private_key()
        .await
        .expect("get_private_key should succeed");

    assert!(!key.key_data().is_empty());
}

#[tokio::test]
async fn test_env_secret_provider_get_webhook_secret_returns_configured_secret() {
    let config = AuthConfig {
        app_id: 12345,
        private_key: TEST_RSA_PRIVATE_KEY.to_string(),
        webhook_secret: "my-webhook-secret".to_string(),
    };

    let provider = EnvSecretProvider::new(config).expect("valid PEM key should succeed");
    let secret = provider
        .get_webhook_secret()
        .await
        .expect("get_webhook_secret should succeed");

    assert_eq!(secret, "my-webhook-secret");
}

#[test]
fn test_env_secret_provider_cache_duration_is_one_hour() {
    let config = AuthConfig {
        app_id: 12345,
        private_key: TEST_RSA_PRIVATE_KEY.to_string(),
        webhook_secret: "test-secret".to_string(),
    };

    let provider = EnvSecretProvider::new(config).expect("valid PEM key should succeed");
    let duration = provider.cache_duration();

    assert_eq!(duration.num_hours(), 1);
}

// ─────────────────────────────────────────────────────────────────────────────
// DefaultJwtSigner
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_sign_jwt_with_out_of_range_exp_returns_signing_failed_error() {
    let signer = DefaultJwtSigner::new();
    let private_key = PrivateKey::from_pem(TEST_RSA_PRIVATE_KEY).expect("valid key");

    // i64::MAX seconds since epoch — far beyond any representable DateTime<Utc>
    let claims = JwtClaims {
        iss: GitHubAppId::new(123),
        iat: 0,
        exp: i64::MAX,
    };

    let result = signer.sign_jwt(claims, &private_key).await;

    assert!(result.is_err(), "Expected Err for out-of-range exp, got Ok");
    match result.unwrap_err() {
        SigningError::SigningFailed { message } => {
            assert!(
                message.contains("out of range"),
                "Expected 'out of range' in error message, got: {message}"
            );
        }
        other => panic!("Expected SigningFailed error, got: {other:?}"),
    }
}
