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
