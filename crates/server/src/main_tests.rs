use super::*;

// ──────────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Clears all GitHub App environment variables.
///
/// Called at the start of each env-var test to guarantee a clean slate
/// regardless of what other tests may have set.
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
