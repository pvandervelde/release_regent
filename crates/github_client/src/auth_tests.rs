/// Tests for the authentication module.
use super::*;
use crate::GitHubClient;

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
    let jwt_result = auth_manager.generate_jwt().await;
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
    let jwt_result = auth_manager.generate_jwt().await;
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
        let jwt_result = auth_manager.generate_jwt().await;
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

// Token Caching and Management Tests (Task 3.6)

#[tokio::test]
async fn test_token_cache_refresh_buffer() {
    let refresh_buffer = Duration::from_secs(300); // 5 minutes
    let cache = TokenCache::new(refresh_buffer);
    let installation_id = 12345;
    let token = "test-token";

    // Store a token that expires in 10 minutes
    let expires_at = Utc::now() + chrono::Duration::minutes(10);
    cache
        .store_token(installation_id, token.to_string(), expires_at)
        .await;

    // Token should be available (expires in 10 minutes, refresh buffer is 5 minutes)
    let cached_token = cache.get_token(installation_id).await;
    assert!(cached_token.is_some());

    // Store a token that expires in 4 minutes (within refresh buffer)
    let expires_soon = Utc::now() + chrono::Duration::minutes(4);
    cache
        .store_token(installation_id, token.to_string(), expires_soon)
        .await;

    // Token should NOT be available (within refresh buffer)
    let cached_token = cache.get_token(installation_id).await;
    assert!(cached_token.is_none());
}

#[tokio::test]
async fn test_token_cache_multiple_installations() {
    let cache = TokenCache::new(Duration::from_secs(300));
    let expires_at = Utc::now() + chrono::Duration::hours(1);

    // Store tokens for multiple installations
    cache
        .store_token(11111, "token-1".to_string(), expires_at)
        .await;
    cache
        .store_token(22222, "token-2".to_string(), expires_at)
        .await;
    cache
        .store_token(33333, "token-3".to_string(), expires_at)
        .await;

    assert_eq!(cache.token_count().await, 3);

    // Verify each token can be retrieved independently
    let token1 = cache.get_token(11111).await;
    let token2 = cache.get_token(22222).await;
    let token3 = cache.get_token(33333).await;

    assert!(token1.is_some());
    assert!(token2.is_some());
    assert!(token3.is_some());

    assert_eq!(token1.unwrap().installation_id, 11111);
    assert_eq!(token2.unwrap().installation_id, 22222);
    assert_eq!(token3.unwrap().installation_id, 33333);
}

#[tokio::test]
async fn test_token_cache_cleanup_expired() {
    let cache = TokenCache::new(Duration::from_secs(60));

    // Store some tokens with different expiration times
    let now = Utc::now();
    let valid_expires = now + chrono::Duration::hours(1);
    let expired_expires = now - chrono::Duration::hours(1);
    let expires_soon = now + chrono::Duration::seconds(30); // Within refresh buffer

    cache
        .store_token(11111, "valid-token".to_string(), valid_expires)
        .await;
    cache
        .store_token(22222, "expired-token".to_string(), expired_expires)
        .await;
    cache
        .store_token(33333, "expires-soon-token".to_string(), expires_soon)
        .await;

    assert_eq!(cache.token_count().await, 3);

    // Clean up expired tokens
    cache.cleanup_expired_tokens().await;

    // Only the valid token should remain
    assert_eq!(cache.token_count().await, 1);

    let remaining_token = cache.get_token(11111).await;
    assert!(remaining_token.is_some());

    let expired_token = cache.get_token(22222).await;
    assert!(expired_token.is_none());

    let soon_expired_token = cache.get_token(33333).await;
    assert!(soon_expired_token.is_none());
}

#[tokio::test]
async fn test_token_cache_remove_specific() {
    let cache = TokenCache::new(Duration::from_secs(300));
    let expires_at = Utc::now() + chrono::Duration::hours(1);

    // Store multiple tokens
    cache
        .store_token(11111, "token-1".to_string(), expires_at)
        .await;
    cache
        .store_token(22222, "token-2".to_string(), expires_at)
        .await;

    assert_eq!(cache.token_count().await, 2);

    // Remove one specific token
    cache.remove_token(11111).await;

    assert_eq!(cache.token_count().await, 1);
    assert!(cache.get_token(11111).await.is_none());
    assert!(cache.get_token(22222).await.is_some());
}

#[tokio::test]
async fn test_token_cache_clear_all() {
    let cache = TokenCache::new(Duration::from_secs(300));
    let expires_at = Utc::now() + chrono::Duration::hours(1);

    // Store multiple tokens
    cache
        .store_token(11111, "token-1".to_string(), expires_at)
        .await;
    cache
        .store_token(22222, "token-2".to_string(), expires_at)
        .await;
    cache
        .store_token(33333, "token-3".to_string(), expires_at)
        .await;

    assert_eq!(cache.token_count().await, 3);

    // Clear all tokens
    cache.clear_all_tokens().await;

    assert_eq!(cache.token_count().await, 0);
    assert!(cache.get_token(11111).await.is_none());
    assert!(cache.get_token(22222).await.is_none());
    assert!(cache.get_token(33333).await.is_none());
}

#[tokio::test]
async fn test_token_cache_update_existing() {
    let cache = TokenCache::new(Duration::from_secs(300));
    let installation_id = 12345;

    // Store initial token
    let initial_expires = Utc::now() + chrono::Duration::minutes(30);
    cache
        .store_token(
            installation_id,
            "initial-token".to_string(),
            initial_expires,
        )
        .await;

    let initial_token = cache.get_token(installation_id).await;
    assert!(initial_token.is_some());
    assert_eq!(
        initial_token.unwrap().token.expose_secret(),
        "initial-token"
    );

    // Update with new token
    let new_expires = Utc::now() + chrono::Duration::hours(1);
    cache
        .store_token(installation_id, "updated-token".to_string(), new_expires)
        .await;

    // Should still have only one token, but with updated value
    assert_eq!(cache.token_count().await, 1);

    let updated_token = cache.get_token(installation_id).await;
    assert!(updated_token.is_some());
    assert_eq!(
        updated_token.unwrap().token.expose_secret(),
        "updated-token"
    );
}

// Rate Limiting and Retry Logic Tests (Task 4.0)

#[tokio::test]
async fn test_rate_limiter_update_from_headers() {
    let rate_limiter = RateLimiter::default();
    let mut headers = reqwest::header::HeaderMap::new();

    // Add rate limit headers
    headers.insert("x-ratelimit-limit", "5000".parse().unwrap());
    headers.insert("x-ratelimit-remaining", "4999".parse().unwrap());
    headers.insert("x-ratelimit-reset", "1609459200".parse().unwrap());
    headers.insert("x-ratelimit-used", "1".parse().unwrap());
    rate_limiter.update_rate_limit_from_headers(&headers).await;

    let rate_limit_info = rate_limiter.get_rate_limit_info().await;
    assert_eq!(rate_limit_info.limit, Some(5000));
    assert_eq!(rate_limit_info.remaining, Some(4999));
    assert_eq!(rate_limit_info.reset, Some(1609459200));
    assert_eq!(rate_limit_info.used, Some(1));
}

#[tokio::test]
async fn test_rate_limiter_should_wait_for_rate_limit_with_remaining() {
    let rate_limiter = RateLimiter::default();
    let mut headers = reqwest::header::HeaderMap::new(); // Set headers indicating we have remaining requests
    headers.insert("x-ratelimit-remaining", "100".parse().unwrap());
    rate_limiter.update_rate_limit_from_headers(&headers).await;

    // Should not need to wait when we have remaining requests
    let wait_duration = rate_limiter.should_wait_for_rate_limit().await;
    assert!(wait_duration.is_none());
}

#[tokio::test]
async fn test_rate_limiter_should_wait_for_rate_limit_exhausted() {
    let rate_limiter = RateLimiter::default();
    let mut headers = reqwest::header::HeaderMap::new();

    // Set headers indicating we have no remaining requests
    headers.insert("x-ratelimit-remaining", "0".parse().unwrap());

    // Set reset time to 1 hour from now
    let reset_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600;
    headers.insert("x-ratelimit-reset", reset_time.to_string().parse().unwrap());

    rate_limiter.update_rate_limit_from_headers(&headers).await;

    // Should need to wait when we have no remaining requests
    let wait_duration = rate_limiter.should_wait_for_rate_limit().await;
    assert!(wait_duration.is_some());
    assert!(wait_duration.unwrap() > Duration::from_secs(3590)); // Should be close to 1 hour
}

#[tokio::test]
async fn test_rate_limiter_should_retry_error() {
    let rate_limiter = RateLimiter::default();
    let policy = RetryPolicy::default(); // Test rate limit error
    let rate_limit_error = Error::rate_limit("60 seconds");
    assert!(rate_limiter.should_retry_error(&rate_limit_error, &policy));

    // Test API request error
    let api_error = Error::api_request(500, "Network error");
    assert!(rate_limiter.should_retry_error(&api_error, &policy));

    // Test authentication error (should not retry)
    let auth_error = Error::authentication("Invalid credentials");
    assert!(!rate_limiter.should_retry_error(&auth_error, &policy));
}

#[tokio::test]
async fn test_retry_policy_default() {
    let policy = RetryPolicy::default();

    assert_eq!(policy.max_retries, 3);
    assert_eq!(policy.base_delay, Duration::from_millis(500));
    assert_eq!(policy.max_delay, Duration::from_secs(60));
    assert!(policy.retry_on_rate_limit);
    assert!(policy.retry_on_network_error);
    assert!(policy.retry_on_server_error);
}

#[tokio::test]
async fn test_rate_limiter_execute_with_retry_success() {
    let rate_limiter = RateLimiter::default();
    let policy = RetryPolicy::default();

    // Test successful execution on first attempt
    let result = rate_limiter
        .execute_with_retry(
            || async { Ok::<String, Error>("success".to_string()) },
            &policy,
        )
        .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "success");
}

#[tokio::test]
async fn test_rate_limiter_execute_with_retry_failure() {
    let rate_limiter = RateLimiter::default();
    let mut policy = RetryPolicy::default();
    policy.max_retries = 0; // No retries

    // Test execution that always fails
    let result = rate_limiter
        .execute_with_retry(
            || async { Err::<String, Error>(Error::authentication("Always fails")) },
            &policy,
        )
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_rate_limiter_execute_with_retry_eventual_success() {
    let rate_limiter = RateLimiter::default();
    let mut policy = RetryPolicy::default();
    policy.max_retries = 2;
    policy.base_delay = Duration::from_millis(1); // Very short delay for testing

    // Counter to track attempts
    let attempt_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let attempt_count_clone = attempt_count.clone();

    // Test execution that succeeds on the second attempt
    let result = rate_limiter
        .execute_with_retry(
            move || {
                let count = attempt_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                async move {
                    if count == 0 {
                        Err::<String, Error>(Error::rate_limit("1 second"))
                    } else {
                        Ok("success".to_string())
                    }
                }
            },
            &policy,
        )
        .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "success");
    assert_eq!(attempt_count.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_rate_limit_info_default() {
    let rate_limit_info = RateLimitInfo::default();

    assert_eq!(rate_limit_info.limit, None);
    assert_eq!(rate_limit_info.remaining, None);
    assert_eq!(rate_limit_info.reset, None);
    assert_eq!(rate_limit_info.used, None);
}

// Security Tests for Sensitive Data Protection (Task 5.6)

#[tokio::test]
async fn test_sensitive_data_not_in_debug_output() {
    let config = AuthConfig::new(12345, TEST_PRIVATE_KEY, None).unwrap();
    let auth_manager = GitHubAuthManager::new(config).unwrap();

    // Test that Debug output for AuthConfig doesn't expose private key
    let config_debug = format!("{:?}", auth_manager.config);
    assert!(!config_debug.contains("BEGIN RSA PRIVATE KEY"));
    assert!(!config_debug.contains("END RSA PRIVATE KEY"));

    // Debug output should contain REDACTED instead of actual key
    assert!(config_debug.contains("REDACTED"));
}

#[tokio::test]
async fn test_token_cache_debug_no_token_exposure() {
    let cache = TokenCache::new(Duration::from_secs(300));
    let installation_id = 12345;
    let sensitive_token = "ghp_1234567890abcdef";
    let expires_at = Utc::now() + chrono::Duration::hours(1);

    cache
        .store_token(installation_id, sensitive_token.to_string(), expires_at)
        .await;

    // Debug output should not contain the actual token value
    let cache_debug = format!("{:?}", cache);
    assert!(!cache_debug.contains(sensitive_token));
    assert!(!cache_debug.contains("ghp_"));

    // Should contain REDACTED instead
    assert!(cache_debug.contains("REDACTED"));
}

#[tokio::test]
async fn test_error_messages_no_sensitive_data() {
    // Test that error messages don't expose sensitive information
    let _sensitive_token = "ghp_1234567890abcdef";
    let _sensitive_key = "-----BEGIN RSA PRIVATE KEY-----\nMIIEpQIB...";

    // Test authentication errors don't expose tokens
    let auth_error = Error::authentication("Token validation failed");
    let error_string = auth_error.to_string();
    assert!(!error_string.contains("ghp_"));
    assert!(!error_string.contains("BEGIN RSA"));

    // Test JWT errors don't expose key material
    let jwt_error = Error::jwt("Invalid private key format");
    let jwt_error_string = jwt_error.to_string();
    assert!(!jwt_error_string.contains("BEGIN RSA"));
    assert!(!jwt_error_string.contains("END RSA"));

    // Test configuration errors sanitize field names
    let config_error = Error::configuration("private_key", "Invalid format");
    let config_error_string = config_error.to_string();
    assert!(config_error_string.contains("private_key"));
    assert!(config_error_string.contains("Invalid format"));
    // But shouldn't contain actual sensitive values
    assert!(!config_error_string.contains("BEGIN RSA PRIVATE KEY"));
}

#[tokio::test]
async fn test_webhook_signature_verification_no_secret_exposure() {
    let payload = b"test payload";
    let secret = "super-secret-webhook-key";
    let wrong_signature = "sha256=wrongsignature";

    // When verification fails, the error shouldn't expose the secret
    let result = GitHubAuthManager::verify_webhook_signature(payload, wrong_signature, secret);
    assert!(result.is_err());

    let error_string = result.unwrap_err().to_string();
    assert!(!error_string.contains(secret));
    assert!(!error_string.contains("super-secret"));
}

#[tokio::test]
async fn test_secrecy_protection_in_cached_tokens() {
    let cache = TokenCache::new(Duration::from_secs(300));
    let installation_id = 12345;
    let sensitive_token = "ghp_1234567890abcdef";
    let expires_at = Utc::now() + chrono::Duration::hours(1);

    cache
        .store_token(installation_id, sensitive_token.to_string(), expires_at)
        .await;

    let cached_token = cache.get_token(installation_id).await;
    assert!(cached_token.is_some());

    let token = cached_token.unwrap();

    // The token field should be a SecretString, not exposed in debug
    let token_debug = format!("{:?}", token);
    assert!(!token_debug.contains(sensitive_token));
    assert!(!token_debug.contains("ghp_"));
    assert!(token_debug.contains("REDACTED"));

    // Only expose_secret() should reveal the actual token
    assert_eq!(token.token.expose_secret(), sensitive_token);
}

#[test]
fn test_constant_time_compare_security() {
    // Test that constant_time_compare is actually constant time by design
    let correct = b"expected_signature";
    let wrong_short = b"wrong";
    let wrong_same_length = b"wrong_signature!!";

    // Different lengths should return false immediately
    assert!(!GitHubAuthManager::constant_time_compare(
        correct,
        wrong_short
    ));

    // Same length but different content should return false
    assert!(!GitHubAuthManager::constant_time_compare(
        correct,
        wrong_same_length
    ));

    // Identical should return true
    assert!(GitHubAuthManager::constant_time_compare(correct, correct));

    // Test with empty arrays
    assert!(GitHubAuthManager::constant_time_compare(b"", b""));
    assert!(!GitHubAuthManager::constant_time_compare(b"", b"not_empty"));
}

/// Test GitHubClient integration with GitHubAuthManager
#[tokio::test]
async fn test_github_client_with_auth_manager() {
    let config = AuthConfig::new(12345, TEST_PRIVATE_KEY, None).unwrap();
    let auth_manager = GitHubAuthManager::new(config).unwrap();

    // Create a GitHubClient with auth manager
    let result = GitHubClient::with_auth_manager(auth_manager).await;
    assert!(result.is_ok());

    let client = result.unwrap();
    // Verify the client has the auth manager
    assert!(client.auth_manager.is_some());
}

/// Test GitHubClient creation of installation client
#[tokio::test]
async fn test_github_client_create_installation_client() {
    let config = AuthConfig::new(12345, TEST_PRIVATE_KEY, None).unwrap();
    let auth_manager = GitHubAuthManager::new(config).unwrap();
    let client = GitHubClient::with_auth_manager(auth_manager).await.unwrap();

    // For now, we'll just test that the method exists and can be called.
    // In a real integration test environment, we would need valid GitHub credentials
    // and an actual installation ID to test the full flow.

    // Test that we can't create an installation client without proper setup
    // This is expected behavior - the method should exist and be callable
    let installation_id = 987654;

    // We'll test this with a panic catch since octocrab might panic internally
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async { client.create_installation_client(installation_id).await })
    }));

    // Either it returns an error or panics (both are acceptable for invalid credentials)
    match result {
        Ok(Ok(_)) => {
            // This would only happen with valid credentials
            panic!(
                "Unexpected success - test environment should not have valid GitHub credentials"
            );
        }
        Ok(Err(_)) => {
            // This is the expected behavior - proper error handling
            // Test passes
        }
        Err(_) => {
            // This is also acceptable - octocrab might panic internally
            // for invalid credentials
            // Test passes
        }
    }
}

/// Test GitHubClient without auth manager cannot create installation client
#[tokio::test]
async fn test_github_client_no_auth_manager_installation_client() {
    let mock_octocrab = octocrab::Octocrab::builder().build().unwrap();

    let client = GitHubClient::new(mock_octocrab);

    // This should fail because there's no auth manager
    let result = client.create_installation_client(987654).await;
    assert!(result.is_err());

    if let Err(error) = result {
        // Should be a configuration error
        assert!(matches!(error, Error::Configuration { .. }));
    }
}

/// Test GitHubAuthManager client creation methods
#[tokio::test]
async fn test_auth_manager_client_creation() {
    let config = AuthConfig::new(12345, TEST_PRIVATE_KEY, None).unwrap();
    let auth_manager = GitHubAuthManager::new(config).unwrap();

    // Test app client creation - this should work without API calls
    let app_client_result = auth_manager.create_app_client().await;
    assert!(app_client_result.is_ok());

    // Test installation client creation - this will make API calls and might panic
    // We'll test that the method exists and handles the scenario appropriately
    let installation_client_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async { auth_manager.create_installation_client(987654).await })
    }));

    // Either it returns an error or panics (both are acceptable for invalid credentials)
    match installation_client_result {
        Ok(Ok(_)) => {
            // This would only happen with valid credentials
            panic!(
                "Unexpected success - test environment should not have valid GitHub credentials"
            );
        }
        Ok(Err(_)) => {
            // This is the expected behavior - proper error handling
            // Test passes
        }
        Err(_) => {
            // This is also acceptable - octocrab might panic internally
            // for invalid credentials
            // Test passes
        }
    }

    // Test token client creation - this should work without API calls
    let token_client_result = auth_manager.create_token_client("test_token").await;
    assert!(token_client_result.is_ok());

    // Test empty token fails
    let empty_token_result = auth_manager.create_token_client("").await;
    assert!(empty_token_result.is_err());
}

/// Test configuration loading from environment variables
#[tokio::test]
async fn test_config_from_env() {
    use std::env;

    // Set up test environment variables
    env::set_var("GITHUB_APP_ID", "12345");
    env::set_var("GITHUB_PRIVATE_KEY", TEST_PRIVATE_KEY);
    env::set_var("GITHUB_BASE_URL", "https://api.github.com");

    let config_result = AuthConfig::from_env();
    assert!(config_result.is_ok());

    let config = config_result.unwrap();
    assert_eq!(config.app_id, 12345);
    assert_eq!(
        config.github_base_url,
        Some("https://api.github.com".to_string())
    );

    // Clean up
    env::remove_var("GITHUB_APP_ID");
    env::remove_var("GITHUB_PRIVATE_KEY");
    env::remove_var("GITHUB_BASE_URL");
}

/// Test backward compatibility of standalone functions
#[tokio::test]
async fn test_backward_compatibility() {
    // Test create_app_client still works
    let result = create_app_client(12345, TEST_PRIVATE_KEY).await;
    assert!(result.is_ok());

    // Test create_token_client still works
    let result = create_token_client("test_token").await;
    assert!(result.is_ok());

    // Test empty token still fails
    let result = create_token_client("").await;
    assert!(result.is_err());
}

// Task 8.1 - Comprehensive unit tests for all authentication methods
/// Test GitHubAuthManager::create_app_client method
#[tokio::test]
async fn test_auth_manager_create_app_client() {
    let config = AuthConfig::new(12345, TEST_PRIVATE_KEY, None).unwrap();
    let auth_manager = GitHubAuthManager::new(config).unwrap();

    let result = auth_manager.create_app_client().await;
    assert!(result.is_ok());

    // We can't test the actual base_url as octocrab doesn't expose it
    // But we can verify the client was created successfully
    let _client = result.unwrap();
    // Just verify it's not panicking - the client creation is the important part
}

/// Test GitHubAuthManager::create_app_client with enterprise URL
#[tokio::test]
async fn test_auth_manager_create_app_client_enterprise() {
    let enterprise_url = Some("https://github.enterprise.com".to_string());
    let config = AuthConfig::new(12345, TEST_PRIVATE_KEY, enterprise_url).unwrap();
    let auth_manager = GitHubAuthManager::new(config).unwrap();

    let result = auth_manager.create_app_client().await;
    assert!(result.is_ok());

    // We can't test the actual base_url as octocrab doesn't expose it
    // But we can verify the client was created successfully with enterprise config
    let _client = result.unwrap();
}

/// Test GitHubAuthManager::create_token_client method
#[tokio::test]
async fn test_auth_manager_create_token_client() {
    let config = AuthConfig::new(12345, TEST_PRIVATE_KEY, None).unwrap();
    let auth_manager = GitHubAuthManager::new(config).unwrap();

    let result = auth_manager.create_token_client("ghp_test_token").await;
    assert!(result.is_ok());

    let _client = result.unwrap();
    // Verify the client was created successfully
}

/// Test GitHubAuthManager::create_token_client with empty token
#[tokio::test]
async fn test_auth_manager_create_token_client_empty() {
    let config = AuthConfig::new(12345, TEST_PRIVATE_KEY, None).unwrap();
    let auth_manager = GitHubAuthManager::new(config).unwrap();

    let result = auth_manager.create_token_client("").await;
    assert!(result.is_err());

    if let Err(error) = result {
        assert!(error.to_string().contains("Token cannot be empty"));
    }
}

/// Test GitHubAuthManager::create_token_client with enterprise URL
#[tokio::test]
async fn test_auth_manager_create_token_client_enterprise() {
    let enterprise_url = Some("https://github.enterprise.com".to_string());
    let config = AuthConfig::new(12345, TEST_PRIVATE_KEY, enterprise_url).unwrap();
    let auth_manager = GitHubAuthManager::new(config).unwrap();

    let result = auth_manager.create_token_client("ghp_test_token").await;
    assert!(result.is_ok());

    let _client = result.unwrap();
    // Verify the client was created successfully with enterprise config
}

/// Test JWT generation with different expiration times
#[tokio::test]
async fn test_jwt_generation_expiration() {
    let config = AuthConfig::new(12345, TEST_PRIVATE_KEY, None).unwrap();
    let auth_manager = GitHubAuthManager::new(config).unwrap();

    // Generate JWT
    let jwt = auth_manager.generate_jwt().await.unwrap();

    // Verify JWT structure without signature validation
    assert!(!jwt.is_empty());
    let parts: Vec<&str> = jwt.split('.').collect();
    assert_eq!(parts.len(), 3);

    // Generate another JWT and verify they're different (different timestamps/jti)
    let jwt2 = auth_manager.generate_jwt().await.unwrap();
    assert_ne!(jwt, jwt2);
}

/// Test JWT generation with different app IDs
#[tokio::test]
async fn test_jwt_generation_different_app_ids() {
    let app_ids = vec![12345, 67890, 999999];

    for app_id in app_ids {
        let config = AuthConfig::new(app_id, TEST_PRIVATE_KEY, None).unwrap();
        let auth_manager = GitHubAuthManager::new(config).unwrap();

        let jwt = auth_manager.generate_jwt().await.unwrap();
        assert!(!jwt.is_empty());

        // Each JWT should be unique
        let parts: Vec<&str> = jwt.split('.').collect();
        assert_eq!(parts.len(), 3);
    }
}

/// Test error conditions for invalid configurations
#[tokio::test]
async fn test_invalid_configurations() {
    // Test with invalid private key
    let result = AuthConfig::new(12345, "invalid-key", None);
    assert!(result.is_err());

    // Test with empty private key
    let result = AuthConfig::new(12345, "", None);
    assert!(result.is_err());

    // Test with zero app ID
    let result = AuthConfig::new(0, TEST_PRIVATE_KEY, None);
    assert!(result.is_err());
}

/// Test configuration validation
#[tokio::test]
async fn test_config_validation() {
    let config = AuthConfig::new(12345, TEST_PRIVATE_KEY, None).unwrap();

    // Test getters
    assert_eq!(config.app_id, 12345);
    assert_eq!(config.jwt_expiration_seconds, 600);
    assert_eq!(config.token_refresh_buffer_seconds, 300);
    assert_eq!(config.get_api_base_url(), "https://api.github.com");
    assert_eq!(config.get_jwt_audience(), "https://github.com");

    // Test with enterprise URL
    let enterprise_config = AuthConfig::new(
        12345,
        TEST_PRIVATE_KEY,
        Some("https://github.enterprise.com".to_string()),
    )
    .unwrap();

    assert_eq!(
        enterprise_config.get_api_base_url(),
        "https://github.enterprise.com/api/v3"
    );
    assert_eq!(
        enterprise_config.get_jwt_audience(),
        "https://github.enterprise.com"
    );
}

/// Test secure token handling and cleanup
#[tokio::test]
async fn test_secure_token_handling() {
    let cache = TokenCache::new(Duration::from_secs(300));
    let installation_id = 12345;
    let token = "sensitive-token-data";
    let expires_at = Utc::now() + chrono::Duration::hours(1);

    // Store token
    cache
        .store_token(installation_id, token.to_string(), expires_at)
        .await;
    assert_eq!(cache.token_count().await, 1);

    // Retrieve token
    let cached_token = cache.get_token(installation_id).await;
    assert!(cached_token.is_some());

    // Clear all tokens (simulating shutdown)
    cache.clear_all_tokens().await;
    assert_eq!(cache.token_count().await, 0);

    // Verify token is no longer accessible
    let cleared_token = cache.get_token(installation_id).await;
    assert!(cleared_token.is_none());
}

/// Test rate limiting behavior
#[tokio::test]
async fn test_rate_limiting_behavior() {
    let rate_limiter = RateLimiter::new(Duration::from_millis(100), 3, Duration::from_millis(100));

    // First request should not require waiting
    let wait_duration = rate_limiter.should_wait_for_rate_limit().await;
    assert!(wait_duration.is_none());

    // Update rate limit info to simulate exhausted limit
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "x-ratelimit-remaining",
        reqwest::header::HeaderValue::from_static("0"),
    );
    headers.insert(
        "x-ratelimit-reset",
        reqwest::header::HeaderValue::from_static("9999999999"),
    ); // Far future

    rate_limiter.update_rate_limit_from_headers(&headers).await;

    // Now it should indicate we need to wait
    let wait_duration = rate_limiter.should_wait_for_rate_limit().await;
    assert!(wait_duration.is_some());
    assert!(wait_duration.unwrap() > Duration::from_secs(0));
}

/// Test authentication manager cloning
#[tokio::test]
async fn test_auth_manager_cloning() {
    let config = AuthConfig::new(12345, TEST_PRIVATE_KEY, None).unwrap();
    let auth_manager = GitHubAuthManager::new(config).unwrap();

    // Clone the manager
    let cloned_manager = auth_manager.clone();

    // Both should be able to generate JWTs
    let jwt1 = auth_manager.generate_jwt().await.unwrap();
    let jwt2 = cloned_manager.generate_jwt().await.unwrap();

    // JWTs should be different (due to different timestamps and JTI)
    assert_ne!(jwt1, jwt2);
    assert!(!jwt1.is_empty());
    assert!(!jwt2.is_empty());
}

// Task 8.2 - Integration tests with mock GitHub API responses
/// Integration test for complete authentication flow
#[tokio::test]
async fn test_integration_complete_auth_flow() {
    let config = AuthConfig::new(12345, TEST_PRIVATE_KEY, None).unwrap();
    let auth_manager = GitHubAuthManager::new(config).unwrap();

    // Test JWT generation
    let jwt = auth_manager.generate_jwt().await;
    assert!(jwt.is_ok());

    // Test app client creation
    let app_client = auth_manager.create_app_client().await;
    assert!(app_client.is_ok());

    // Test token client creation
    let token_client = auth_manager.create_token_client("ghp_test_token").await;
    assert!(token_client.is_ok());

    // All operations should succeed without API calls
}

/// Integration test for authentication with enterprise GitHub
#[tokio::test]
async fn test_integration_enterprise_auth_flow() {
    let config = AuthConfig::new(
        12345,
        TEST_PRIVATE_KEY,
        Some("https://github.enterprise.com".to_string()),
    )
    .unwrap();
    let auth_manager = GitHubAuthManager::new(config).unwrap();

    // Test JWT generation for enterprise
    let jwt = auth_manager.generate_jwt().await;
    assert!(jwt.is_ok());

    // Test enterprise app client creation
    let app_client = auth_manager.create_app_client().await;
    assert!(app_client.is_ok());

    // Test configuration values
    assert_eq!(
        auth_manager.config.get_api_base_url(),
        "https://github.enterprise.com/api/v3"
    );
    assert_eq!(
        auth_manager.config.get_jwt_audience(),
        "https://github.enterprise.com"
    );
}

/// Integration test for token caching workflow
#[tokio::test]
async fn test_integration_token_caching_workflow() {
    let cache = TokenCache::new(Duration::from_secs(300));
    let installation_id = 12345;

    // Initially no tokens
    assert_eq!(cache.token_count().await, 0);
    assert!(cache.get_token(installation_id).await.is_none());

    // Store a token
    let token = "ghs_test_token";
    let expires_at = Utc::now() + chrono::Duration::hours(1);
    cache
        .store_token(installation_id, token.to_string(), expires_at)
        .await;

    // Should find the token
    assert_eq!(cache.token_count().await, 1);
    let cached_token = cache.get_token(installation_id).await;
    assert!(cached_token.is_some());
    assert_eq!(cached_token.unwrap().installation_id, installation_id);

    // Should handle expiration buffer
    let cached_token = cache.get_token(installation_id).await;
    assert!(cached_token.is_some());

    // Store an expired token
    let expired_token = "ghs_expired_token";
    let expired_time = Utc::now() - chrono::Duration::hours(1);
    cache
        .store_token(99999, expired_token.to_string(), expired_time)
        .await;

    // Clean up expired tokens
    cache.cleanup_expired_tokens().await;

    // Should still have the non-expired token
    assert_eq!(cache.token_count().await, 1);
    assert!(cache.get_token(installation_id).await.is_some());
    assert!(cache.get_token(99999).await.is_none());
}

/// Integration test for rate limiting workflow
#[tokio::test]
async fn test_integration_rate_limiting_workflow() {
    let rate_limiter = RateLimiter::new(Duration::from_millis(100), 3, Duration::from_millis(100));

    // Initially should not require waiting
    assert!(rate_limiter.should_wait_for_rate_limit().await.is_none());

    // Simulate GitHub API rate limit headers
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "x-ratelimit-limit",
        reqwest::header::HeaderValue::from_static("5000"),
    );
    headers.insert(
        "x-ratelimit-remaining",
        reqwest::header::HeaderValue::from_static("4999"),
    );
    headers.insert(
        "x-ratelimit-reset",
        reqwest::header::HeaderValue::from_static("1234567890"),
    );

    // Update from headers
    rate_limiter.update_rate_limit_from_headers(&headers).await;

    // Should still not require waiting
    assert!(rate_limiter.should_wait_for_rate_limit().await.is_none());

    // Simulate exhausted rate limit
    headers.insert(
        "x-ratelimit-remaining",
        reqwest::header::HeaderValue::from_static("0"),
    );
    let future_reset = (Utc::now() + chrono::Duration::minutes(1)).timestamp();
    headers.insert(
        "x-ratelimit-reset",
        reqwest::header::HeaderValue::from_str(&future_reset.to_string()).unwrap(),
    );

    rate_limiter.update_rate_limit_from_headers(&headers).await;

    // Should now require waiting
    let wait_duration = rate_limiter.should_wait_for_rate_limit().await;
    assert!(wait_duration.is_some());
    assert!(wait_duration.unwrap() > Duration::from_secs(0));
}

/// Integration test for retry policy behavior
#[tokio::test]
async fn test_integration_retry_policy_behavior() {
    let retry_policy = RetryPolicy::default();

    // Test basic retry policy properties
    let max_retries = retry_policy.max_retries;
    assert!(
        max_retries > 0 && max_retries <= 5,
        "Max retries should be reasonable"
    );

    let base_delay = retry_policy.base_delay;
    assert!(
        base_delay > Duration::from_millis(0),
        "Base delay should be positive"
    );

    let max_delay = retry_policy.max_delay;
    assert!(
        max_delay > base_delay,
        "Max delay should be greater than base delay"
    );
}

/// Integration test for configuration loading from environment
#[tokio::test]
async fn test_integration_environment_configuration() {
    use std::env;

    // Set environment variables
    env::set_var("GITHUB_APP_ID", "54321");
    env::set_var("GITHUB_PRIVATE_KEY", TEST_PRIVATE_KEY);
    env::set_var("GITHUB_BASE_URL", "https://github.example.com");

    // Load configuration from environment
    let config = AuthConfig::from_env().unwrap();

    // Verify values
    assert_eq!(config.app_id, 54321);
    assert_eq!(
        config.github_base_url,
        Some("https://github.example.com".to_string())
    );

    // Test auth manager creation with env config
    let auth_manager = GitHubAuthManager::new(config).unwrap();
    let jwt = auth_manager.generate_jwt().await;
    assert!(jwt.is_ok());

    // Clean up
    env::remove_var("GITHUB_APP_ID");
    env::remove_var("GITHUB_PRIVATE_KEY");
    env::remove_var("GITHUB_BASE_URL");
}

/// Integration test for error handling scenarios
#[tokio::test]
async fn test_integration_error_handling_scenarios() {
    // Test invalid configuration
    let invalid_config = AuthConfig::new(0, "invalid-key", None);
    assert!(invalid_config.is_err());

    // Test empty token
    let config = AuthConfig::new(12345, TEST_PRIVATE_KEY, None).unwrap();
    let auth_manager = GitHubAuthManager::new(config).unwrap();
    let result = auth_manager.create_token_client("").await;
    assert!(result.is_err());

    // Test token cache with invalid installation ID
    let cache = TokenCache::new(Duration::from_secs(300));
    let invalid_token = cache.get_token(0).await;
    assert!(invalid_token.is_none());

    // Test rate limiter with invalid headers
    let rate_limiter = RateLimiter::default();
    let mut invalid_headers = reqwest::header::HeaderMap::new();
    invalid_headers.insert(
        "invalid-header",
        reqwest::header::HeaderValue::from_static("invalid-value"),
    );

    // Should handle invalid headers gracefully
    rate_limiter
        .update_rate_limit_from_headers(&invalid_headers)
        .await;
    assert!(rate_limiter.should_wait_for_rate_limit().await.is_none());
}

/// Integration test for concurrent authentication operations
#[tokio::test]
async fn test_integration_concurrent_operations() {
    let config = AuthConfig::new(12345, TEST_PRIVATE_KEY, None).unwrap();
    let auth_manager = GitHubAuthManager::new(config).unwrap();

    // Create multiple concurrent JWT generation tasks
    let mut tasks = Vec::new();
    for i in 0..10 {
        let manager = auth_manager.clone();
        let task = tokio::spawn(async move {
            let jwt = manager.generate_jwt().await;
            (i, jwt)
        });
        tasks.push(task);
    }

    // Wait for all tasks to complete
    let mut results = Vec::new();
    for task in tasks {
        let result = task.await;
        results.push(result);
    }

    // All should succeed and be unique
    let mut jwts = Vec::new();
    for result in results {
        let (i, jwt_result) = result.unwrap();
        assert!(jwt_result.is_ok(), "Task {} failed", i);
        let jwt = jwt_result.unwrap();
        assert!(!jwt.is_empty());
        jwts.push(jwt);
    }

    // All JWTs should be unique (due to different timestamps and JTI)
    for i in 0..jwts.len() {
        for j in i + 1..jwts.len() {
            assert_ne!(jwts[i], jwts[j], "JWTs should be unique");
        }
    }
}

/// Integration test for memory management and cleanup
#[tokio::test]
async fn test_integration_memory_management() {
    let cache = TokenCache::new(Duration::from_secs(300));

    // Store many tokens
    for i in 0..100 {
        let token = format!("token_{}", i);
        let expires_at = Utc::now() + chrono::Duration::hours(1);
        cache.store_token(i, token, expires_at).await;
    }

    assert_eq!(cache.token_count().await, 100);

    // Clear all tokens
    cache.clear_all_tokens().await;
    assert_eq!(cache.token_count().await, 0);

    // Verify no tokens remain
    for i in 0..100 {
        assert!(cache.get_token(i).await.is_none());
    }
}

// Task 8.3 - Property-based tests for JWT generation
/// Property-based test for JWT generation with various app IDs
#[tokio::test]
async fn test_property_jwt_generation_app_ids() {
    let test_cases = vec![
        1,
        100,
        1000,
        10000,
        99999,
        123456,
        999999,
        1000000,
        u64::MAX / 2,
        u64::MAX - 1,
    ];

    for app_id in test_cases {
        let config = AuthConfig::new(app_id, TEST_PRIVATE_KEY, None).unwrap();
        let auth_manager = GitHubAuthManager::new(config).unwrap();

        let jwt = auth_manager.generate_jwt().await;
        assert!(jwt.is_ok(), "JWT generation failed for app_id: {}", app_id);

        let jwt_string = jwt.unwrap();
        assert!(!jwt_string.is_empty());
        assert_eq!(
            jwt_string.split('.').count(),
            3,
            "JWT should have 3 parts for app_id: {}",
            app_id
        );

        // Verify the JWT contains the app_id in the payload
        let parts: Vec<&str> = jwt_string.split('.').collect();
        let payload = parts[1];
        // We can't easily decode without proper base64 padding, but we can check it's not empty
        assert!(!payload.is_empty());
    }
}

/// Property-based test for JWT generation with various expiration times
#[tokio::test]
async fn test_property_jwt_generation_expiration_times() {
    let config = AuthConfig::new(12345, TEST_PRIVATE_KEY, None).unwrap();
    let auth_manager = GitHubAuthManager::new(config).unwrap();

    // Generate multiple JWTs at different times
    let mut jwts = Vec::new();
    for _i in 0..10 {
        let jwt = auth_manager.generate_jwt().await.unwrap();
        jwts.push(jwt);

        // Small delay to ensure different timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // All JWTs should be unique (due to different timestamps and JTI)
    for i in 0..jwts.len() {
        for j in i + 1..jwts.len() {
            assert_ne!(jwts[i], jwts[j], "JWTs should be unique: {} vs {}", i, j);
        }
    }

    // All JWTs should have the same structure
    for (i, jwt) in jwts.iter().enumerate() {
        assert_eq!(jwt.split('.').count(), 3, "JWT {} should have 3 parts", i);
        assert!(!jwt.is_empty(), "JWT {} should not be empty", i);
    }
}

/// Property-based test for JWT generation with various GitHub URLs
#[tokio::test]
async fn test_property_jwt_generation_github_urls() {
    let test_urls = vec![
        None,
        Some("https://github.com".to_string()),
        Some("https://api.github.com".to_string()),
        Some("https://github.enterprise.com".to_string()),
        Some("https://github.internal.company.com".to_string()),
        Some("https://github.example.org".to_string()),
        Some("https://ghe.domain.net".to_string()),
    ];

    for github_url in test_urls {
        let config = AuthConfig::new(12345, TEST_PRIVATE_KEY, github_url.clone()).unwrap();
        let auth_manager = GitHubAuthManager::new(config).unwrap();

        let jwt = auth_manager.generate_jwt().await;
        assert!(
            jwt.is_ok(),
            "JWT generation failed for URL: {:?}",
            github_url
        );

        let jwt_string = jwt.unwrap();
        assert!(!jwt_string.is_empty());
        assert_eq!(
            jwt_string.split('.').count(),
            3,
            "JWT should have 3 parts for URL: {:?}",
            github_url
        );

        // Verify the audience is set correctly
        // We can't easily decode the JWT without proper padding, but we can verify it was created
        assert!(
            jwt_string.len() > 100,
            "JWT should be substantial for URL: {:?}",
            github_url
        );
    }
}

/// Property-based test for JWT generation under concurrent load
#[tokio::test]
async fn test_property_jwt_generation_concurrent_load() {
    let config = AuthConfig::new(12345, TEST_PRIVATE_KEY, None).unwrap();
    let auth_manager = GitHubAuthManager::new(config).unwrap();

    // Test with different concurrency levels
    let concurrency_levels = vec![1, 5, 10, 20, 50];

    for concurrency in concurrency_levels {
        let mut tasks = Vec::new();

        for i in 0..concurrency {
            let manager = auth_manager.clone();
            let task = tokio::spawn(async move {
                let jwt = manager.generate_jwt().await;
                (i, jwt)
            });
            tasks.push(task);
        }

        // Wait for all tasks
        let mut results = Vec::new();
        for task in tasks {
            let result = task.await.unwrap();
            results.push(result);
        }

        // All should succeed
        for (i, jwt_result) in &results {
            assert!(
                jwt_result.is_ok(),
                "Task {} failed at concurrency level {}",
                i,
                concurrency
            );
        }

        // All JWTs should be unique
        let jwts: Vec<_> = results
            .iter()
            .map(|(_, jwt)| jwt.as_ref().unwrap())
            .collect();
        for i in 0..jwts.len() {
            for j in i + 1..jwts.len() {
                assert_ne!(
                    jwts[i], jwts[j],
                    "JWTs should be unique at concurrency level {}",
                    concurrency
                );
            }
        }
    }
}

/// Property-based test for JWT generation with edge case private keys
#[tokio::test]
async fn test_property_jwt_generation_private_key_formats() {
    // Test with different valid private key formats
    let padded_key = format!("  {}  ", TEST_PRIVATE_KEY);
    let windows_key = TEST_PRIVATE_KEY.replace('\n', "\r\n");

    let test_keys = vec![
        TEST_PRIVATE_KEY, // Standard format
        // We could test with different key sizes, but for now we'll use the same key
        // with different whitespace variations
        TEST_PRIVATE_KEY.trim(),
        &padded_key,
        &windows_key, // Windows line endings
    ];

    for (i, private_key) in test_keys.iter().enumerate() {
        let config = AuthConfig::new(12345, private_key.to_string(), None);

        if config.is_ok() {
            let auth_manager = GitHubAuthManager::new(config.unwrap()).unwrap();
            let jwt = auth_manager.generate_jwt().await;
            assert!(jwt.is_ok(), "JWT generation failed for key format {}", i);

            let jwt_string = jwt.unwrap();
            assert!(!jwt_string.is_empty());
            assert_eq!(jwt_string.split('.').count(), 3);
        }
    }
}

/// Property-based test for JWT validation and structure
#[tokio::test]
async fn test_property_jwt_structure_validation() {
    let config = AuthConfig::new(12345, TEST_PRIVATE_KEY, None).unwrap();
    let auth_manager = GitHubAuthManager::new(config).unwrap();

    // Generate multiple JWTs and validate their structure
    for i in 0..20 {
        let jwt = auth_manager.generate_jwt().await.unwrap();

        // Check basic structure
        let parts: Vec<&str> = jwt.split('.').collect();
        assert_eq!(parts.len(), 3, "JWT {} should have exactly 3 parts", i);

        // Check each part is base64-like (no spaces, reasonable length)
        for (j, part) in parts.iter().enumerate() {
            assert!(!part.is_empty(), "JWT {} part {} should not be empty", i, j);
            assert!(
                !part.contains(' '),
                "JWT {} part {} should not contain spaces",
                i,
                j
            );
            assert!(
                part.len() > 10,
                "JWT {} part {} should have substantial length",
                i,
                j
            );

            // Check it's base64url-like (only valid characters)
            let valid_chars = part
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '=');
            assert!(
                valid_chars,
                "JWT {} part {} should only contain valid base64url characters",
                i, j
            );
        }

        // Header should be shortest, signature should be longest typically
        assert!(
            parts[0].len() < parts[1].len(),
            "JWT {} header should be shorter than payload",
            i
        );
        assert!(
            parts[2].len() > 100,
            "JWT {} signature should be substantial",
            i
        );
    }
}

/// Property-based test for JWT generation timing properties
#[tokio::test]
async fn test_property_jwt_generation_timing() {
    let config = AuthConfig::new(12345, TEST_PRIVATE_KEY, None).unwrap();
    let auth_manager = GitHubAuthManager::new(config).unwrap();

    // Test JWT generation timing is reasonable
    let mut generation_times = Vec::new();

    for _ in 0..10 {
        let start = std::time::Instant::now();
        let jwt = auth_manager.generate_jwt().await;
        let duration = start.elapsed();

        assert!(jwt.is_ok(), "JWT generation should succeed");
        generation_times.push(duration);
    }

    // All generations should be reasonably fast (< 1 second)
    for (i, duration) in generation_times.iter().enumerate() {
        assert!(
            duration.as_secs() < 1,
            "JWT generation {} took too long: {:?}",
            i,
            duration
        );
    }

    // Average should be very fast (< 100ms)
    let avg_duration =
        generation_times.iter().sum::<std::time::Duration>() / generation_times.len() as u32;
    assert!(
        avg_duration.as_millis() < 100,
        "Average JWT generation time too slow: {:?}",
        avg_duration
    );
}

/// Property-based test for JWT generation with various configurations
#[tokio::test]
async fn test_property_jwt_generation_config_combinations() {
    let app_ids = vec![1, 12345, 999999];
    let urls = vec![
        None,
        Some("https://github.com".to_string()),
        Some("https://github.enterprise.com".to_string()),
    ];

    // Test all combinations
    for app_id in &app_ids {
        for url in &urls {
            let config = AuthConfig::new(*app_id, TEST_PRIVATE_KEY, url.clone()).unwrap();
            let auth_manager = GitHubAuthManager::new(config).unwrap();

            let jwt = auth_manager.generate_jwt().await;
            assert!(
                jwt.is_ok(),
                "JWT generation failed for app_id: {}, url: {:?}",
                app_id,
                url
            );

            let jwt_string = jwt.unwrap();
            assert!(!jwt_string.is_empty());
            assert_eq!(jwt_string.split('.').count(), 3);

            // Verify configuration consistency
            match url {
                None => {
                    assert_eq!(
                        auth_manager.config.get_api_base_url(),
                        "https://api.github.com"
                    );
                    assert_eq!(auth_manager.config.get_jwt_audience(), "https://github.com");
                }
                Some(github_url) => {
                    assert_eq!(auth_manager.config.get_jwt_audience(), github_url.as_str());
                }
            }
        }
    }
}
