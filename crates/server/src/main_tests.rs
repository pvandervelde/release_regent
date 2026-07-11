use super::*;
use std::sync::{LazyLock, Mutex};
use tempfile::TempDir;

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
}

/// Clears the repository allow-list/exclude-list environment variables.
///
/// Must only be called while holding [`ENV_LOCK`].
fn clear_repo_scope_env_vars() {
    std::env::remove_var("ALLOWED_REPOS");
    std::env::remove_var("EXCLUDED_REPOS");
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

// ──────────────────────────────────────────────────────────────────────────────
// read_github_credentials_from_env — happy path
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_read_github_credentials_all_valid_returns_parsed_values() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_github_app_env_vars();
    std::env::set_var("GITHUB_APP_ID", "99999");
    std::env::set_var("GITHUB_PRIVATE_KEY", "-----BEGIN RSA PRIVATE KEY-----");

    let result = read_github_credentials_from_env();

    std::env::remove_var("GITHUB_APP_ID");
    std::env::remove_var("GITHUB_PRIVATE_KEY");

    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    let (app_id, private_key) = result.unwrap();
    assert_eq!(app_id, 99_999_u64);
    assert_eq!(private_key, "-----BEGIN RSA PRIVATE KEY-----");
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

    let result = build_server_processor("test-webhook-secret".to_string()).await;

    std::env::remove_var("GITHUB_APP_ID");
    std::env::remove_var("GITHUB_PRIVATE_KEY");

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

    let result = build_server_processor("test-webhook-secret".to_string()).await;

    std::env::remove_var("GITHUB_APP_ID");
    std::env::remove_var("GITHUB_PRIVATE_KEY");

    assert!(result.is_err(), "Expected error for invalid PEM key");
    // `GitHubClient::from_config` returns `CoreError::GitHub`, which maps to
    // `errors::Error::Core` via the `#[from]` impl — NOT `errors::Error::GitHub`.
    // (The `errors::Error::GitHub` variant is for direct `github_client::Error`
    // returns that are NOT wrapped in a `CoreError` first.)
    match result {
        Err(errors::Error::Core {
            source: release_regent_core::CoreError::GitHub { .. },
        }) => {
            // Expected: the invalid PEM causes EnvSecretProvider::new to return
            // SecretError::InvalidFormat, which is wrapped as CoreError::GitHub by
            // GitHubClient::from_config, then converted to Error::Core here.
        }
        Err(other) => panic!("Expected Core(GitHub) error variant for invalid PEM, got: {other:?}"),
        Ok(_) => panic!("Expected Err but got Ok"),
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// parse_comma_separated
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_parse_comma_separated_trims_whitespace_and_filters_empty_entries() {
    let result = parse_comma_separated(" myorg/a , , myorg/b ");
    assert_eq!(result, vec!["myorg/a".to_string(), "myorg/b".to_string()]);
}

#[test]
fn test_parse_comma_separated_empty_string_returns_empty_vec() {
    let result = parse_comma_separated("");
    assert!(result.is_empty());
}

#[test]
fn test_parse_comma_separated_single_entry_returns_single_element_vec() {
    let result = parse_comma_separated("*");
    assert_eq!(result, vec!["*".to_string()]);
}

// ──────────────────────────────────────────────────────────────────────────────
// resolve_repo_scope — precedence and default semantics (BA-68, BA-69, BA-74)
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_resolve_repo_scope_no_env_no_file_defaults_allow_wildcard_and_exclude_empty() {
    // BA-68: an unset allow-list config must act on every repository —
    // equivalent to `["*"]`. BA-74: an unset exclude-list must exclude nothing.
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_repo_scope_env_vars();
    let temp_dir = TempDir::new().expect("must create temp dir");

    let (allowed, excluded) =
        resolve_repo_scope(temp_dir.path()).expect("resolve_repo_scope must succeed");

    assert_eq!(allowed, vec!["*".to_string()]);
    assert!(excluded.is_empty());
}

#[test]
fn test_resolve_repo_scope_missing_config_directory_defaults_allow_wildcard() {
    // The config directory itself need not exist — this must not error/panic,
    // and must fall back to the same defaults as a directory with no config file.
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_repo_scope_env_vars();
    let nonexistent = std::path::PathBuf::from("this-directory-does-not-exist-rr-test");

    let (allowed, excluded) =
        resolve_repo_scope(&nonexistent).expect("resolve_repo_scope must succeed");

    assert_eq!(allowed, vec!["*".to_string()]);
    assert!(excluded.is_empty());
}

#[test]
fn test_resolve_repo_scope_env_allowed_repos_present_used_verbatim() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_repo_scope_env_vars();
    std::env::set_var("ALLOWED_REPOS", "myorg/a,myorg/b");
    let temp_dir = TempDir::new().expect("must create temp dir");

    let (allowed, _excluded) =
        resolve_repo_scope(temp_dir.path()).expect("resolve_repo_scope must succeed");

    std::env::remove_var("ALLOWED_REPOS");

    assert_eq!(allowed, vec!["myorg/a".to_string(), "myorg/b".to_string()]);
}

#[test]
fn test_resolve_repo_scope_env_allowed_repos_explicit_empty_string_returns_empty_vec() {
    // BA-69: an explicitly empty allow-list must deny every repository. This
    // test locks in that `ALLOWED_REPOS=""` (explicitly configured, present in
    // the environment) resolves to an empty Vec — DIFFERENT from the "unset"
    // case, which defaults to `["*"]`.
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_repo_scope_env_vars();
    std::env::set_var("ALLOWED_REPOS", "");
    let temp_dir = TempDir::new().expect("must create temp dir");

    let (allowed, _excluded) =
        resolve_repo_scope(temp_dir.path()).expect("resolve_repo_scope must succeed");

    std::env::remove_var("ALLOWED_REPOS");

    assert!(
        allowed.is_empty(),
        "explicit empty ALLOWED_REPOS must resolve to an empty Vec, not the wildcard default"
    );
}

#[test]
fn test_resolve_repo_scope_env_excluded_repos_present_used_verbatim() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_repo_scope_env_vars();
    std::env::set_var("EXCLUDED_REPOS", "myorg/legacy-secrets");
    let temp_dir = TempDir::new().expect("must create temp dir");

    let (_allowed, excluded) =
        resolve_repo_scope(temp_dir.path()).expect("resolve_repo_scope must succeed");

    std::env::remove_var("EXCLUDED_REPOS");

    assert_eq!(excluded, vec!["myorg/legacy-secrets".to_string()]);
}

#[test]
fn test_resolve_repo_scope_env_var_trims_whitespace_and_filters_empty_entries() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_repo_scope_env_vars();
    std::env::set_var("ALLOWED_REPOS", " myorg/a , , myorg/b ");
    let temp_dir = TempDir::new().expect("must create temp dir");

    let (allowed, _excluded) =
        resolve_repo_scope(temp_dir.path()).expect("resolve_repo_scope must succeed");

    std::env::remove_var("ALLOWED_REPOS");

    assert_eq!(allowed, vec!["myorg/a".to_string(), "myorg/b".to_string()]);
}

// ──────────────────────────────────────────────────────────────────────────────
// resolve_repo_scope — release-regent.toml fallback
// ──────────────────────────────────────────────────────────────────────────────

/// Write a `release-regent.toml` fixture into `dir` with a plain top-level
/// `allowed_repositories`/`excluded_repositories` array (must appear BEFORE any
/// `[section]` header to remain part of the TOML root table rather than being
/// absorbed into that section).
fn write_repo_scope_toml(dir: &std::path::Path, contents: &str) {
    std::fs::write(dir.join("release-regent.toml"), contents).expect("must write fixture file");
}

#[test]
fn test_resolve_repo_scope_env_absent_falls_back_to_toml_allowed_repositories() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_repo_scope_env_vars();
    let temp_dir = TempDir::new().expect("must create temp dir");
    write_repo_scope_toml(
        temp_dir.path(),
        r#"
allowed_repositories = ["myorg/*"]

[core]
version_prefix = "v"
"#,
    );

    let (allowed, _excluded) =
        resolve_repo_scope(temp_dir.path()).expect("resolve_repo_scope must succeed");

    assert_eq!(allowed, vec!["myorg/*".to_string()]);
}

#[test]
fn test_resolve_repo_scope_env_absent_falls_back_to_toml_excluded_repositories() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_repo_scope_env_vars();
    let temp_dir = TempDir::new().expect("must create temp dir");
    write_repo_scope_toml(
        temp_dir.path(),
        r#"
allowed_repositories = ["myorg/*"]
excluded_repositories = ["myorg/legacy-secrets"]
"#,
    );

    let (_allowed, excluded) =
        resolve_repo_scope(temp_dir.path()).expect("resolve_repo_scope must succeed");

    assert_eq!(excluded, vec!["myorg/legacy-secrets".to_string()]);
}

#[test]
fn test_resolve_repo_scope_env_present_takes_precedence_over_toml_file() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_repo_scope_env_vars();
    std::env::set_var("ALLOWED_REPOS", "fromenv/*");
    let temp_dir = TempDir::new().expect("must create temp dir");
    write_repo_scope_toml(temp_dir.path(), r#"allowed_repositories = ["fromfile/*"]"#);

    let (allowed, _excluded) =
        resolve_repo_scope(temp_dir.path()).expect("resolve_repo_scope must succeed");

    std::env::remove_var("ALLOWED_REPOS");

    assert_eq!(
        allowed,
        vec!["fromenv/*".to_string()],
        "ALLOWED_REPOS env var must take precedence over release-regent.toml"
    );
}

#[test]
fn test_resolve_repo_scope_toml_file_present_but_missing_allowed_key_defaults_to_wildcard() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_repo_scope_env_vars();
    let temp_dir = TempDir::new().expect("must create temp dir");
    write_repo_scope_toml(
        temp_dir.path(),
        r#"
[core]
version_prefix = "v"
"#,
    );

    let (allowed, excluded) =
        resolve_repo_scope(temp_dir.path()).expect("resolve_repo_scope must succeed");

    assert_eq!(
        allowed,
        vec!["*".to_string()],
        "missing allowed_repositories key must default to wildcard, not error"
    );
    assert!(excluded.is_empty());
}

#[test]
fn test_resolve_repo_scope_toml_file_with_unrelated_sections_does_not_error() {
    // The file may also contain the unrelated ReleaseRegentConfig sections
    // ([core], [versioning], ...). Their presence must not cause an error.
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_repo_scope_env_vars();
    let temp_dir = TempDir::new().expect("must create temp dir");
    write_repo_scope_toml(
        temp_dir.path(),
        r#"
allowed_repositories = ["myorg/*"]
excluded_repositories = ["myorg/legacy-secrets"]

[core]
version_prefix = "v"

[core.branches]
main = "main"

[versioning]
strategy = "conventional"
allow_override = true
"#,
    );

    let (allowed, excluded) =
        resolve_repo_scope(temp_dir.path()).expect("resolve_repo_scope must succeed");

    assert_eq!(allowed, vec!["myorg/*".to_string()]);
    assert_eq!(excluded, vec!["myorg/legacy-secrets".to_string()]);
}

#[test]
fn test_resolve_repo_scope_toml_excluded_repositories_explicit_empty_array_returns_empty_vec() {
    // BA-74: an explicitly empty exclude-list (`[]`) must still resolve to an
    // empty Vec (same observable result as "absent" at this layer — the
    // semantic distinction from BA-69 is enforced downstream, at
    // `ReleaseRegentWebhookHandler::is_allowed`, not by this function).
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_repo_scope_env_vars();
    let temp_dir = TempDir::new().expect("must create temp dir");
    write_repo_scope_toml(
        temp_dir.path(),
        r#"
allowed_repositories = ["myorg/*"]
excluded_repositories = []
"#,
    );

    let (_allowed, excluded) =
        resolve_repo_scope(temp_dir.path()).expect("resolve_repo_scope must succeed");

    assert!(excluded.is_empty());
}

#[test]
fn test_resolve_repo_scope_missing_toml_file_defaults_allow_wildcard() {
    // No release-regent.toml at all in an otherwise-existing directory.
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_repo_scope_env_vars();
    let temp_dir = TempDir::new().expect("must create temp dir");
    // Deliberately do not write any file into temp_dir.

    let (allowed, excluded) =
        resolve_repo_scope(temp_dir.path()).expect("resolve_repo_scope must succeed");

    assert_eq!(allowed, vec!["*".to_string()]);
    assert!(excluded.is_empty());
}

/// Tech-lead decision (resolves the RED-phase documented gap): a
/// `release-regent.toml` that exists but fails to parse as valid TOML syntax
/// must be a fatal startup error — never silently treated as "file absent,
/// use defaults". This is consistent with a malformed glob pattern
/// (BA-71/BA-75) also being a fatal startup error, and with the project's
/// fail-fast philosophy. Only a genuinely *missing* file, or a
/// present-but-parseable file that simply omits the
/// `allowed_repositories`/`excluded_repositories` keys, falls back to
/// defaults.
#[test]
fn test_resolve_repo_scope_malformed_toml_syntax_returns_error() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_repo_scope_env_vars();
    let temp_dir = TempDir::new().expect("must create temp dir");
    write_repo_scope_toml(temp_dir.path(), "this is not valid TOML syntax [[[");

    let result = resolve_repo_scope(temp_dir.path());

    assert!(
        result.is_err(),
        "malformed TOML syntax must be a fatal error, not a silent fallback to defaults"
    );
}
