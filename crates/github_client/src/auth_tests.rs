//! Tests for the authentication module.

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
