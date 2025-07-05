/// Tests for the authentication module.
use super::*;

#[tokio::test]
async fn test_auth_config_basic_creation() {
    // Test basic config fields without private key validation
    let config = AuthConfig {
        app_id: 12345,
        private_key: SecretString::new("test-key".to_string()),
        github_base_url: None,
        jwt_expiration_seconds: 600,
        token_refresh_buffer_seconds: 300,
    };

    assert_eq!(config.app_id, 12345);
    assert_eq!(config.jwt_expiration_seconds, 600);
    assert_eq!(config.token_refresh_buffer_seconds, 300);
    assert!(config.github_base_url.is_none());
}

#[tokio::test]
async fn test_auth_config_invalid_private_key() {
    let invalid_key = "not-a-valid-key";
    let config = AuthConfig::new(12345, invalid_key, None);
    assert!(config.is_err());
}

#[tokio::test]
async fn test_token_cache_new() {
    let cache = TokenCache::new(Duration::from_secs(300));
    assert_eq!(cache.token_count().await, 0);
}

#[tokio::test]
async fn test_token_cache_store_and_retrieve() {
    let cache = TokenCache::new(Duration::from_secs(300));
    let installation_id = 12345;
    let token = "test-token";
    let expires_at = Utc::now() + chrono::Duration::hours(1);

    cache
        .store_token(installation_id, token.to_string(), expires_at)
        .await;
    assert_eq!(cache.token_count().await, 1);

    let cached_token = cache.get_token(installation_id).await;
    assert!(cached_token.is_some());
    assert_eq!(cached_token.unwrap().installation_id, installation_id);
}

#[tokio::test]
async fn test_token_cache_expired_token() {
    let cache = TokenCache::new(Duration::from_secs(300));
    let installation_id = 12345;
    let token = "test-token";
    let expires_at = Utc::now() - chrono::Duration::hours(1); // Expired

    cache
        .store_token(installation_id, token.to_string(), expires_at)
        .await;

    // Should return None for expired token
    let cached_token = cache.get_token(installation_id).await;
    assert!(cached_token.is_none());
}

#[tokio::test]
async fn test_rate_limiter_default() {
    let limiter = RateLimiter::default();
    assert_eq!(limiter.max_retries(), 3);
}

#[tokio::test]
async fn test_rate_limiter_backoff_calculation() {
    let limiter = RateLimiter::default();
    let delay_0 = limiter.calculate_backoff_delay(0);
    let delay_1 = limiter.calculate_backoff_delay(1);
    let delay_2 = limiter.calculate_backoff_delay(2);

    // Each delay should be roughly double the previous (with jitter)
    assert!(delay_1 >= delay_0);
    assert!(delay_2 >= delay_1);
}

// JWT Generation and Validation Tests

const TEST_PRIVATE_KEY: &str = r#"-----BEGIN RSA PRIVATE KEY-----
MIIEpQIBAAKCAQEAr93E9k17/2pz40XGcl6Zn1fpwaWvZLbEr+d+ta7aXE1Jyz3q
rLcbD7n0djnbn/SSAc2t9luEuclnVrTQDpL2a6nJaudwka3iRS5ZkELL7DHmW0I1
UuqSPjRoC9U1LuyRgdJ/j2LW2sF1THQIbQK0cJhC6zWL8TF1S45a64WRxhDh/DhO
3v4KnvOkF7cYx68pCAcIxNrJxJNo0K1xNEPx1rVZermPHwX4O6Xh2Lv0Q8Wrzcoh
6wBn5fwMB/nIpGkKbJR6iBv6aqRXHsXIWCvCrXr7JtdQDShr0yxWYYhRmkcxS8JX
vQxU/twRcZVPs/Xy+bkIvlBRSXsU2YsHZ3K+AwIDAQABAoIBAQCPjJ+WxAxwoX3S
h0PCWwFeFS5SyGDv/ldxla3RstW9/cA8S1/gdt156rlmPzfLyp/bJP3YVz9xPrpB
BfnFy6OkocQspJk38Az/lyO4Iy28r8Ztuw83jQyuBazf67orgSIMK7u/WFgz2zFZ
pGS0Rj7uoPoOb3i4+Tva3mnMUQx59DYkFX8oVBmj1UrMS44HTI0WbETeFXyBe7NL
eynNYZrL5Hu1P0V48y8oCqWr+iWCQ8zdOtc+zlkFOh38xTzcM7RzxssdcyZNEi+Z
yA0vM4GZznuvD6rtk4MWmJidbpmlmnFx8BUSLJ3vUX/9wnUSS4i7XWgF0iZhrGm1
v9+RmdsxAoGBAOfD+v0Yx1ts41OQw8etsHWoSKKkrgP2QUKX+kR2lPEN7MNUywPI
388SqT8tUAmOU968BQnEINhq/qOkc31YoU6hTq9XU2qgQZaBPyUjtc/mIkNhXxf4
BX3+0hwMF3Zmaw8s4K+syRl5vBavxaLSQzU3o3SfExv7Jx2eVyAfcCgdAoGBAMJB
ct+tXtNzkutwPvc72GoGhj+Pz7C02d1eo0fOPChkZ5M4fsTM/bjbEaVANz1VrcJO
dkrrRBjKWTpMolH9K2UeuizCA9szY4FcEgSOjQLSwUaVvcXVzUromNO7D8o2eO7E
/cgFOoM8sWqtIDQA9t0oJoqo62SWn/HSoVMEUOSfAoGAagWBH57iM7SQGX4z0Xhn
LKua7qwe3rkcCXa0ifUlFVClaoWziTuvBm8m9TupTXXKcC3asCkETXxEmF92ZXTR
9cJc2GE+S5yb5FmjpT28wiooqmI0uiY/fO/A9guiAAvCFeMVtcd5ByplHIu0AVPm
YsXdBFBw0XAG6MmyWYOILxECgYEAr/25xFp/EdWgovtzoGwwuoYktItnH/IJfByU
k6aOLA4jJGdHuqb5q7OVvgB6y2/HV8XcOC4D6O3SfxYU7XEQErIO/CPeeIaLPRSc
IlLAAHHOt1NMtmLodlhatWDBgnNthu0j+0Z5Z5LiLgKhrVu+TElm+bLmaKBqUh2B
GQRNAYMCgYEA4cCk47DXZxVbDUVLoQZBjxEi2J3OB2979tlRxc6M0WViS6GRW7Si
bwyPdEotYK+BflLYzGq7xthbqQhQwCK+mnkvtELs3tCtkC3PMEpWbEs19cAzjS8v
wUjIF/5CdJgsfR3jgY75h+upW7LNZluBKJ3lpvVOnVCYrpWkZsab4NY=
-----END RSA PRIVATE KEY-----"#;

#[tokio::test]
async fn test_jwt_generation_basic() {
    let config = AuthConfig::new(12345, TEST_PRIVATE_KEY, None).unwrap();
    let auth_manager = GitHubAuthManager::new(config).unwrap();

    // Generate a JWT - we only test generation, not validation
    let jwt_result = auth_manager.generate_jwt();
    assert!(jwt_result.is_ok());

    let jwt = jwt_result.unwrap();
    assert!(!jwt.is_empty());

    // JWT should have the expected structure (header.payload.signature)
    let parts: Vec<&str> = jwt.split('.').collect();
    assert_eq!(
        parts.len(),
        3,
        "JWT should have three parts separated by dots"
    );
}

#[tokio::test]
async fn test_github_enterprise_jwt_generation() {
    let enterprise_url = Some("https://github.enterprise.com".to_string());
    let config = AuthConfig::new(12345, TEST_PRIVATE_KEY, enterprise_url).unwrap();
    let auth_manager = GitHubAuthManager::new(config).unwrap();

    // Test that JWT generation works with enterprise URLs
    let jwt_result = auth_manager.generate_jwt();
    assert!(jwt_result.is_ok());

    let jwt = jwt_result.unwrap();
    assert!(!jwt.is_empty());

    // JWT should have the expected structure
    let parts: Vec<&str> = jwt.split('.').collect();
    assert_eq!(
        parts.len(),
        3,
        "JWT should have three parts separated by dots"
    );
}

// Webhook Signature Verification Tests

#[test]
fn test_webhook_signature_verification_valid() {
    let payload = b"test payload";
    let secret = "test-secret";

    // Pre-computed HMAC-SHA256 for the test payload and secret
    let signature = "sha256=2f94a757d2246073e26781d117ce0183ebd87b4d66c460494376d5c37d71985b";

    let result = GitHubAuthManager::verify_webhook_signature(payload, signature, secret);
    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[test]
fn test_webhook_signature_verification_invalid() {
    let payload = b"test payload";
    let secret = "test-secret";
    let wrong_signature = "sha256=wrongsignature";

    let result = GitHubAuthManager::verify_webhook_signature(payload, wrong_signature, secret);
    // This should fail due to invalid hex encoding
    assert!(result.is_err());
}

#[test]
fn test_webhook_signature_verification_wrong_format() {
    let payload = b"test payload";
    let secret = "test-secret";
    let wrong_format = "md5=abcd1234"; // Wrong algorithm prefix

    let result = GitHubAuthManager::verify_webhook_signature(payload, wrong_format, secret);
    assert!(result.is_err());
}

#[test]
fn test_constant_time_compare() {
    let a = b"test";
    let b = b"test";
    let c = b"different";

    assert!(GitHubAuthManager::constant_time_compare(a, b));
    assert!(!GitHubAuthManager::constant_time_compare(a, c));
    assert!(!GitHubAuthManager::constant_time_compare(a, b"tests")); // Different lengths
}

// Integration-style tests

#[tokio::test]
async fn test_jwt_multiple_generation() {
    let config = AuthConfig::new(54321, TEST_PRIVATE_KEY, None).unwrap();
    let auth_manager = GitHubAuthManager::new(config).unwrap();

    // Generate multiple JWTs to test that generation works consistently
    for _i in 0..5 {
        let jwt_result = auth_manager.generate_jwt();
        assert!(jwt_result.is_ok());

        let jwt = jwt_result.unwrap();
        assert!(!jwt.is_empty());

        // JWT should have the expected structure
        let parts: Vec<&str> = jwt.split('.').collect();
        assert_eq!(
            parts.len(),
            3,
            "JWT should have three parts separated by dots"
        );
    }
}
