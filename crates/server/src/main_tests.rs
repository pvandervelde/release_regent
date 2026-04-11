use super::*;
use std::sync::{LazyLock, Mutex};

// ──────────────────────────────────────────────────────────────────────────────
// Test-env serialization lock
// ──────────────────────────────────────────────────────────────────────────────

/// Mutex that serializes every test that mutates global process environment.
///
/// `std::env::set_var`/`remove_var` are not thread-safe when tests run in
/// parallel (Rust's default). All env-var tests acquire this guard as their
/// first statement so they run sequentially without data races.
static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(Mutex::default);

// ──────────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Clears all GitHub App environment variables.
///
/// Must only be called while holding [`ENV_LOCK`].
fn clear_github_app_env_vars() {
    std::env::remove_var("GITHUB_APP_ID");
    std::env::remove_var("GITHUB_PRIVATE_KEY");
    std::env::remove_var("GITHUB_INSTALLATION_ID");
}

// ──────────────────────────────────────────────────────────────────────────────
// read_github_credentials_from_env — missing variable paths
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_read_github_credentials_missing_app_id_returns_environment_error() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_github_app_env_vars();

    let result = read_github_credentials_from_env();

    assert!(
        result.is_err(),
        "Expected error when GITHUB_APP_ID is absent"
    );
    let err = result.unwrap_err();
    match err {
        errors::Error::Environment { variable, .. } => {
            assert_eq!(variable, "GITHUB_APP_ID");
        }
        other => panic!("Expected Environment error for GITHUB_APP_ID, got: {other:?}"),
    }
}

#[test]
fn test_read_github_credentials_missing_private_key_returns_environment_error() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_github_app_env_vars();
    std::env::set_var("GITHUB_APP_ID", "12345");

    let result = read_github_credentials_from_env();

    std::env::remove_var("GITHUB_APP_ID");

    assert!(
        result.is_err(),
        "Expected error when GITHUB_PRIVATE_KEY is absent"
    );
    let err = result.unwrap_err();
    match err {
        errors::Error::Environment { variable, .. } => {
            assert_eq!(variable, "GITHUB_PRIVATE_KEY");
        }
        other => panic!("Expected Environment error for GITHUB_PRIVATE_KEY, got: {other:?}"),
    }
}

#[test]
fn test_read_github_credentials_missing_installation_id_returns_environment_error() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_github_app_env_vars();
    std::env::set_var("GITHUB_APP_ID", "12345");
    std::env::set_var("GITHUB_PRIVATE_KEY", "some-key");

    let result = read_github_credentials_from_env();

    std::env::remove_var("GITHUB_APP_ID");
    std::env::remove_var("GITHUB_PRIVATE_KEY");

    assert!(
        result.is_err(),
        "Expected error when GITHUB_INSTALLATION_ID is absent"
    );
    let err = result.unwrap_err();
    match err {
        errors::Error::Environment { variable, .. } => {
            assert_eq!(variable, "GITHUB_INSTALLATION_ID");
        }
        other => panic!("Expected Environment error for GITHUB_INSTALLATION_ID, got: {other:?}"),
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// read_github_credentials_from_env — malformed value paths
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_read_github_credentials_non_numeric_app_id_returns_environment_error() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_github_app_env_vars();
    std::env::set_var("GITHUB_APP_ID", "not-a-number");

    let result = read_github_credentials_from_env();

    std::env::remove_var("GITHUB_APP_ID");

    assert!(
        result.is_err(),
        "Expected error for non-numeric GITHUB_APP_ID"
    );
    let err = result.unwrap_err();
    match err {
        errors::Error::Environment { variable, message } => {
            assert_eq!(variable, "GITHUB_APP_ID");
            assert!(
                message.contains("must be a number"),
                "Expected 'must be a number' in message, got: {message}"
            );
        }
        other => panic!("Expected Environment error, got: {other:?}"),
    }
}

#[test]
fn test_read_github_credentials_non_numeric_installation_id_returns_environment_error() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_github_app_env_vars();
    std::env::set_var("GITHUB_APP_ID", "12345");
    std::env::set_var("GITHUB_PRIVATE_KEY", "some-key");
    std::env::set_var("GITHUB_INSTALLATION_ID", "not-a-number");

    let result = read_github_credentials_from_env();

    std::env::remove_var("GITHUB_APP_ID");
    std::env::remove_var("GITHUB_PRIVATE_KEY");
    std::env::remove_var("GITHUB_INSTALLATION_ID");

    assert!(
        result.is_err(),
        "Expected error for non-numeric GITHUB_INSTALLATION_ID"
    );
    let err = result.unwrap_err();
    match err {
        errors::Error::Environment { variable, message } => {
            assert_eq!(variable, "GITHUB_INSTALLATION_ID");
            assert!(
                message.contains("must be a number"),
                "Expected 'must be a number' in message, got: {message}"
            );
        }
        other => panic!("Expected Environment error, got: {other:?}"),
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// read_github_credentials_from_env — happy path
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_read_github_credentials_all_valid_returns_parsed_values() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_github_app_env_vars();
    std::env::set_var("GITHUB_APP_ID", "99999");
    std::env::set_var("GITHUB_PRIVATE_KEY", "-----BEGIN RSA PRIVATE KEY-----");
    std::env::set_var("GITHUB_INSTALLATION_ID", "12345678");

    let result = read_github_credentials_from_env();

    std::env::remove_var("GITHUB_APP_ID");
    std::env::remove_var("GITHUB_PRIVATE_KEY");
    std::env::remove_var("GITHUB_INSTALLATION_ID");

    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    let (app_id, private_key, installation_id) = result.unwrap();
    assert_eq!(app_id, 99_999_u64);
    assert_eq!(private_key, "-----BEGIN RSA PRIVATE KEY-----");
    assert_eq!(installation_id, 12_345_678_u64);
}

// ──────────────────────────────────────────────────────────────────────────────
// build_server_processor — success and error paths (task 1.5)
// ──────────────────────────────────────────────────────────────────────────────

/// A valid RSA-2048 private key used only in tests.
///
/// This key is a development/testing artefact shared in the `github_client`
/// crate test fixtures. It is not registered as a GitHub App key on any real
/// installation and grants no access to any system.
const TEST_RSA_PRIVATE_KEY: &str = include_str!("../../github_client/test_key.pem");

/// `build_server_processor` constructs a real `ReleaseRegentProcessor` when all
/// required environment variables are present and the private key is valid PEM.
///
/// The processor is constructed but never contacts the GitHub API during
/// construction — token exchange only happens on the first API call.
#[tokio::test]
async fn test_build_server_processor_with_valid_credentials_succeeds() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_github_app_env_vars();
    std::env::set_var("GITHUB_APP_ID", "99999");
    std::env::set_var("GITHUB_PRIVATE_KEY", TEST_RSA_PRIVATE_KEY);
    std::env::set_var("GITHUB_INSTALLATION_ID", "12345678");

    let result = build_server_processor("test-webhook-secret".to_string()).await;

    std::env::remove_var("GITHUB_APP_ID");
    std::env::remove_var("GITHUB_PRIVATE_KEY");
    std::env::remove_var("GITHUB_INSTALLATION_ID");

    assert!(
        result.is_ok(),
        "Expected Ok when all credentials are valid, got: {:?}",
        result.err()
    );
}

/// `build_server_processor` returns a `GitHub` error when the private key is
/// not valid PEM — the error originates from key parsing, before any network
/// call is made.
#[tokio::test]
async fn test_build_server_processor_with_invalid_pem_returns_github_error() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_github_app_env_vars();
    std::env::set_var("GITHUB_APP_ID", "99999");
    std::env::set_var("GITHUB_PRIVATE_KEY", "not-a-pem-key");
    std::env::set_var("GITHUB_INSTALLATION_ID", "12345678");

    let result = build_server_processor("test-webhook-secret".to_string()).await;

    std::env::remove_var("GITHUB_APP_ID");
    std::env::remove_var("GITHUB_PRIVATE_KEY");
    std::env::remove_var("GITHUB_INSTALLATION_ID");

    assert!(result.is_err(), "Expected error for invalid PEM key");
    // `GitHubClient::from_config` returns `CoreError::GitHub`, which maps to
    // `errors::Error::Core` via the `#[from]` impl — NOT `errors::Error::GitHub`.
    // (The `errors::Error::GitHub` variant is for direct `github_client::Error`
    // returns that are NOT wrapped in a `CoreError` first.)
    match result {
        Err(errors::Error::Core { .. }) => {}
        Err(other) => panic!("Expected Core error variant for invalid PEM, got: {other:?}"),
        Ok(_) => panic!("Expected Err but got Ok"),
    }
}
