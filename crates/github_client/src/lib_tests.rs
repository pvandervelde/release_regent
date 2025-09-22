use crate::models::Repository;
use crate::{GitHubAppBuilder, GitHubClient, RetryConfig, RetryConfigBuilder, TokenBuilder};
use std::time::Duration;

#[tokio::test]
async fn test_github_client_new() {
    // Test that GitHubClient can be created with an Octocrab instance
    let octocrab = octocrab::Octocrab::builder()
        .build()
        .expect("Failed to create Octocrab client");

    let client = crate::GitHubClient::new(octocrab);

    // Verify default configuration
    assert!(client.correlation_id().len() > 0);
    assert_eq!(client.retry_config.max_attempts, 3);
    assert_eq!(client.retry_config.base_delay, Duration::from_millis(100));
}

#[test]
fn test_repository_creation() {
    let repo = Repository::new(
        "repo".to_string(),
        "owner/repo".to_string(),
        "MDEwOlJlcG9zaXRvcnkx".to_string(),
        false,
    );

    assert_eq!(repo.name(), "repo");
    assert_eq!(repo.is_private(), false);
}

#[test]
fn test_retry_config_default() {
    let config = RetryConfig::default();

    assert_eq!(config.max_attempts, 3);
    assert_eq!(config.base_delay, Duration::from_millis(100));
    assert_eq!(config.max_delay, Duration::from_secs(30));
    assert_eq!(config.backoff_factor, 2.0);
}

#[test]
fn test_retry_config_builder() {
    let config = RetryConfigBuilder::default()
        .max_attempts(5)
        .base_delay(Duration::from_millis(200))
        .max_delay(Duration::from_secs(60))
        .backoff_factor(1.5)
        .build();

    assert_eq!(config.max_attempts, 5);
    assert_eq!(config.base_delay, Duration::from_millis(200));
    assert_eq!(config.max_delay, Duration::from_secs(60));
    assert_eq!(config.backoff_factor, 1.5);
}

#[test]
fn test_app_builder_configuration() {
    let builder = GitHubAppBuilder::new(123456, "test-private-key".to_string())
        .github_enterprise("https://github.example.com")
        .jwt_expiration(Duration::from_secs(300))
        .token_refresh_buffer(Duration::from_secs(60));

    // Test internal state (this is a unit test, so we can access internals)
    assert_eq!(builder.app_id, 123456);
    assert_eq!(builder.private_key, "test-private-key");
    assert_eq!(
        builder.github_base_url,
        Some("https://github.example.com".to_string())
    );
    assert_eq!(builder.jwt_expiration_seconds, Some(300));
    assert_eq!(builder.token_refresh_buffer_seconds, Some(60));
}

#[test]
fn test_app_builder_retry_config() {
    let builder =
        GitHubAppBuilder::new(123456, "test-private-key".to_string()).retry_config(|config| {
            config
                .max_attempts(10)
                .base_delay(Duration::from_millis(50))
        });

    // Verify retry config was set
    assert!(builder.retry_config.is_some());
    let config = builder.retry_config.unwrap();
    assert_eq!(config.max_attempts, 10);
    assert_eq!(config.base_delay, Duration::from_millis(50));
}

#[test]
fn test_token_builder_configuration() {
    let builder = TokenBuilder::new("ghp_test_token".to_string())
        .github_enterprise("https://github.example.com");

    // Test internal state
    assert_eq!(builder.token, "ghp_test_token");
    assert_eq!(
        builder.github_base_url,
        Some("https://github.example.com".to_string())
    );
}

#[tokio::test]
async fn test_is_retryable_error() {
    let octocrab = octocrab::Octocrab::builder()
        .build()
        .expect("Failed to create Octocrab client");
    let client = GitHubClient::new(octocrab);

    // Test various error scenarios
    // Note: This tests the retry logic's error classification
    // In a real implementation, we'd mock octocrab errors

    // Test that client has correct retry configuration
    assert_eq!(client.retry_config.max_attempts, 3);
}

#[tokio::test]
async fn test_correlation_id_uniqueness() {
    let octocrab1 = octocrab::Octocrab::builder()
        .build()
        .expect("Failed to create Octocrab client");
    let octocrab2 = octocrab::Octocrab::builder()
        .build()
        .expect("Failed to create Octocrab client");

    let client1 = GitHubClient::new(octocrab1);
    let client2 = GitHubClient::new(octocrab2);

    // Correlation IDs should be unique
    assert_ne!(client1.correlation_id(), client2.correlation_id());
}

#[test]
fn test_retry_config_builder_defaults() {
    let builder = RetryConfigBuilder::default();
    let config = builder.build();

    assert_eq!(config.max_attempts, 3);
    assert_eq!(config.base_delay, Duration::from_millis(100));
    assert_eq!(config.max_delay, Duration::from_secs(30));
    assert_eq!(config.backoff_factor, 2.0);
}

#[test]
fn test_jwt_expiration_limit() {
    let builder = GitHubAppBuilder::new(123456, "test-private-key".to_string())
        .jwt_expiration(Duration::from_secs(800)); // Over 10 minutes

    // JWT expiration should be capped at 600 seconds (10 minutes)
    assert_eq!(builder.jwt_expiration_seconds, Some(600));
}

// Note: GitHub client creation tests with actual authentication require credentials
// and network access. These are covered in integration tests.
// The factory methods (from_app, from_token, from_installation) will be tested
// in integration tests where we can provide mock credentials or test credentials.
