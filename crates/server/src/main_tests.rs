use super::*;
use std::sync::{LazyLock, Mutex};
use tempfile::TempDir;
use tracing_test::traced_test;

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

/// Clears the `CONFIG_DIR` environment variable.
///
/// Must only be called while holding [`ENV_LOCK`].
fn clear_config_dir_env_var() {
    std::env::remove_var("CONFIG_DIR");
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
// resolve_config_dir
// ──────────────────────────────────────────────────────────────────────────────
//
// Mutation-audit gap (QA Engineer, post-implementation): cargo-mutants found
// that `resolve_config_dir`'s body could be replaced with
// `Ok(Default::default())` (i.e. always returning an empty `PathBuf`) without
// any test failing. `resolve_config_dir` is one of the primary functions in
// scope for the repository allow-list/exclude-list feature (it supplies the
// directory `resolve_repo_scope`/`load_repo_scope_toml` search for
// `release-regent.toml`), so an untested "always empty path" regression is a
// genuine correctness gap, not a cosmetic one: a deployment that sets
// `CONFIG_DIR` to point at a mounted config volume would silently fall back
// to reading `release-regent.toml` (or nothing) from the process's current
// working directory instead.

#[test]
fn test_resolve_config_dir_env_var_present_returns_that_path() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_config_dir_env_var();
    std::env::set_var("CONFIG_DIR", "some/configured/directory");

    let result = resolve_config_dir();

    std::env::remove_var("CONFIG_DIR");

    assert_eq!(
        result.expect("resolve_config_dir must succeed when CONFIG_DIR is set"),
        std::path::PathBuf::from("some/configured/directory"),
        "CONFIG_DIR must be used verbatim, not silently discarded"
    );
}

#[test]
fn test_resolve_config_dir_env_var_absent_returns_current_working_directory() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_config_dir_env_var();

    let result = resolve_config_dir();

    assert_eq!(
        result.expect("resolve_config_dir must succeed when CONFIG_DIR is unset"),
        std::env::current_dir().expect("test process must have a current directory"),
        "an unset CONFIG_DIR must fall back to the current working directory, not an empty path"
    );
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

/// Regression test for a security finding: `release-regent.toml` is the same
/// file used for the broader app configuration hierarchy (not exclusive to
/// the allow/exclude-list keys). If a TOML syntax error occurs on a line
/// containing a secret-looking value, the parse-error message returned by
/// [`load_repo_scope_toml`] must not embed the raw source line (which the
/// `toml` crate's `Display` impl otherwise includes verbatim), because this
/// error propagates via `?` out of `main()` and is printed to stderr/container
/// logs on startup failure.
#[test]
fn test_resolve_repo_scope_malformed_toml_error_does_not_leak_secret_value() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_repo_scope_env_vars();
    let temp_dir = TempDir::new().expect("must create temp dir");
    let secret = "ghp_SuperSecretToken1234567890ABCDEF";
    write_repo_scope_toml(
        temp_dir.path(),
        &format!("github_webhook_secret = {secret}"),
    );

    let result = resolve_repo_scope(temp_dir.path());

    let err = result.expect_err("malformed TOML syntax must be a fatal error");
    let rendered = format!("{err}");
    assert!(
        !rendered.contains(secret),
        "error message must not leak the raw source line containing a secret-looking \
         value, got: {rendered}"
    );
}

// ──────────────────────────────────────────────────────────────────────────────
// line_col_at — multi-byte UTF-8 char-boundary panic (code-review finding 1
// on PR #216)
// ──────────────────────────────────────────────────────────────────────────────
//
// `line_col_at` is called from `load_repo_scope_toml`'s TOML-parse-error path
// with a byte offset derived from `toml::de::Error::span().start`. Its body
// slices `text[line_start..byte_offset]` directly. `line_start` is always a
// valid char boundary (it is only ever `0`, or `idx + ch.len_utf8()` taken
// from a `char_indices()` iteration), but `byte_offset` is an arbitrary
// `usize` accepted at the function's signature and is only clamped to
// `text.len()` — never snapped to the nearest char boundary. Any
// `byte_offset` that lands strictly inside a multi-byte UTF-8 sequence causes
// `str` slicing to panic with "byte index N is not a char boundary".

#[test]
fn test_line_col_at_byte_offset_mid_multibyte_char_does_not_panic() {
    // "café = 1": bytes are c=0 a=1 f=2 é=3..5 (2-byte UTF-8) ' '=5 '='=6
    // ' '=7 1=8. Char boundaries: {0,1,2,3,5,6,7,8,9}. byte_offset=4 sits
    // strictly inside the 2-byte é sequence and is well below text.len()
    // (9), so it is not affected by the `.min(text.len())` clamp.
    let text = "café = 1";
    assert_eq!(
        text.len(),
        9,
        "fixture assumption: é must encode as 2 UTF-8 bytes"
    );
    assert!(
        !text.is_char_boundary(4),
        "fixture assumption: byte offset 4 must fall inside a multi-byte character"
    );

    let (line, column) = line_col_at(text, 4);

    assert_eq!(line, 1, "single-line text: line must remain 1");
    assert!(
        (1..=text.chars().count() + 1).contains(&column),
        "column must be a plausible 1-based column within the line, got {column}"
    );
}

#[test]
fn test_line_col_at_byte_offset_mid_multibyte_char_after_newline_does_not_panic() {
    // A first line establishes a real line increment (exercising the
    // `ch == '\n'` branch), then the offending offset lands inside the
    // 4-byte 🎉 emoji on the second line.
    //
    // Byte layout of "café = 1\nemoji = 🎉x":
    //   c=0 a=1 f=2 é=3..5 ' '=5 '='=6 ' '=7 1=8 '\n'=9
    //   e=10 m=11 o=12 j=13 i=14 ' '=15 '='=16 ' '=17 🎉=18..22 x=22
    // (length 23). Char boundaries near the emoji: {18, 22}. byte_offset=20
    // sits strictly inside the 4-byte 🎉 sequence.
    let text = "café = 1\nemoji = \u{1F389}x";
    let newline_idx = text.find('\n').expect("fixture must contain a newline");
    assert!(
        newline_idx < 20,
        "fixture assumption: byte offset 20 must be after the newline"
    );
    assert!(
        !text.is_char_boundary(20),
        "fixture assumption: byte offset 20 must fall inside the multi-byte emoji"
    );
    assert!(
        20 < text.len(),
        "fixture assumption: offset must be below text.len(), not clamped away"
    );

    let (line, column) = line_col_at(text, 20);

    assert_eq!(
        line, 2,
        "byte offset 20 lies on the second (post-newline) line"
    );
    assert!(
        column >= 1,
        "column must be a valid 1-based column, got {column}"
    );
}

// ──────────────────────────────────────────────────────────────────────────────
// line_col_at — property tests (Tier 3): never panics on arbitrary input
// ──────────────────────────────────────────────────────────────────────────────

mod line_col_at_property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// General form of the multi-byte-boundary bug reproduced above:
        /// `line_col_at` must never panic for ANY `(text, byte_offset)`
        /// pair, including byte offsets that were never produced by
        /// iterating `text.char_indices()` (e.g. offsets landing
        /// mid-character).
        ///
        /// `byte_offset` is deliberately generated relative to `text.len()`
        /// (`0..=text.len() + 4`) rather than over the full `usize` range: a
        /// uniformly-random `usize` almost always lands far past
        /// `text.len()` and gets clamped straight to a valid end-of-string
        /// boundary by `.min(text.len())`, which would make this property
        /// pass against the buggy implementation almost every run and defeat
        /// the point of an adversarial test. Restricting the range to just
        /// past `text.len()` keeps a realistic proportion of generated cases
        /// landing inside multi-byte characters whenever `text` contains
        /// any, which is exactly the scenario that panics today.
        #[test]
        fn prop_line_col_at_never_panics_on_arbitrary_input(
            (text, byte_offset) in proptest::arbitrary::any::<String>().prop_flat_map(|text| {
                let max_offset = text.len().saturating_add(4);
                (Just(text), 0..=max_offset)
            })
        ) {
            let (line, column) = line_col_at(&text, byte_offset);
            prop_assert!(line >= 1, "line must be 1-based, got {line}");
            prop_assert!(column >= 1, "column must be 1-based, got {column}");
        }

        /// Complementary invariant covering the full documented signature
        /// (`byte_offset: usize` — including offsets far beyond
        /// `text.len()`, exercising the `.min(text.len())` clamp path
        /// itself never panics for arbitrary huge offsets).
        #[test]
        fn prop_line_col_at_never_panics_for_offsets_across_full_usize_range(
            text in proptest::arbitrary::any::<String>(),
            byte_offset in proptest::arbitrary::any::<usize>(),
        ) {
            let (line, column) = line_col_at(&text, byte_offset);
            prop_assert!(line >= 1, "line must be 1-based, got {line}");
            prop_assert!(column >= 1, "column must be 1-based, got {column}");
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// detect_misplaced_repo_scope_keys — misplaced repo-scope key footgun
// (code-review finding 2 on PR #216)
// ──────────────────────────────────────────────────────────────────────────────
//
// TOML grammar means a bare `key = value` line written AFTER a `[section]`
// header belongs to that section, not the root table. `RepoScopeToml` only
// ever looks at the root table (via `toml::from_str::<RepoScopeToml>`), so a
// misplaced `allowed_repositories`/`excluded_repositories` key silently
// deserializes to `None` and `resolve_repo_scope` falls back to the
// wildcard-allow default with zero signal to the operator. These tests cover
// `detect_misplaced_repo_scope_keys`, the function responsible for spotting
// this footgun so the caller can emit a `tracing::warn!`.

#[test]
fn test_detect_misplaced_repo_scope_keys_nested_in_section_is_reported() {
    let contents = r#"
[core]
release_branch_prefix = "release"

allowed_repositories = ["myorg/*"]
"#;

    let found = detect_misplaced_repo_scope_keys(contents);

    assert_eq!(
        found,
        vec![("allowed_repositories".to_string(), "core".to_string())],
        "a key written after a [section] header must be reported as nested in that section"
    );
}

#[test]
fn test_detect_misplaced_repo_scope_keys_correct_root_placement_reports_nothing() {
    let contents = r#"
allowed_repositories = ["myorg/*"]

[core]
release_branch_prefix = "release"
"#;

    let found = detect_misplaced_repo_scope_keys(contents);

    assert!(
        found.is_empty(),
        "a key correctly placed at the root table (before any [section] header) \
         must never be reported as misplaced, got: {found:?}"
    );
}

#[test]
fn test_detect_misplaced_repo_scope_keys_neither_key_present_reports_nothing() {
    let contents = r#"
[core]
release_branch_prefix = "release"

[versioning]
strategy = "conventional"
"#;

    let found = detect_misplaced_repo_scope_keys(contents);

    assert!(
        found.is_empty(),
        "a file containing neither repo-scope key anywhere must report nothing, got: {found:?}"
    );
}

#[test]
fn test_detect_misplaced_repo_scope_keys_only_reports_the_misplaced_one_of_two() {
    let contents = r#"
allowed_repositories = ["myorg/*"]

[core]
release_branch_prefix = "release"
excluded_repositories = ["myorg/legacy-secrets"]
"#;

    let found = detect_misplaced_repo_scope_keys(contents);

    assert_eq!(
        found,
        vec![("excluded_repositories".to_string(), "core".to_string())],
        "only the misplaced key must be reported; the correctly root-placed \
         allowed_repositories must not appear, got: {found:?}"
    );
}

#[test]
fn test_detect_misplaced_repo_scope_keys_nested_two_levels_deep_reports_dotted_section() {
    let contents = r#"
[core]
release_branch_prefix = "release"

[core.branches]
main = "main"
excluded_repositories = ["myorg/legacy-secrets"]
"#;

    let found = detect_misplaced_repo_scope_keys(contents);

    assert_eq!(
        found,
        vec![(
            "excluded_repositories".to_string(),
            "core.branches".to_string()
        )],
        "a key nested two sections deep must report the full dotted section path, got: {found:?}"
    );
}

#[test]
fn test_detect_misplaced_repo_scope_keys_malformed_toml_returns_empty_not_panic() {
    // Best-effort diagnostic: a genuine TOML syntax error is already handled
    // as a fatal error by `load_repo_scope_toml`'s existing parse-error path
    // (see `test_resolve_repo_scope_malformed_toml_syntax_returns_error`).
    // This function must not duplicate that failure mode by panicking.
    let found = detect_misplaced_repo_scope_keys("this is not valid TOML syntax [[[");

    assert!(
        found.is_empty(),
        "malformed TOML input must resolve to an empty Vec, not panic or error, got: {found:?}"
    );
}

// ──────────────────────────────────────────────────────────────────────────────
// resolve_repo_scope — misplaced repo-scope key footgun is logged
// (end-to-end wiring for code-review finding 2)
// ──────────────────────────────────────────────────────────────────────────────

#[test]
#[traced_test]
fn test_resolve_repo_scope_misplaced_allowed_repositories_logs_warning() {
    // This is a detection-and-warn fix, NOT a behavior change: the fallback
    // to the wildcard-allow default must remain unchanged (deliberately NOT
    // asserted as an `Err` here), but the operator must receive a loud
    // signal that their `allowed_repositories` key was silently ignored.
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    clear_repo_scope_env_vars();
    let temp_dir = TempDir::new().expect("must create temp dir");
    write_repo_scope_toml(
        temp_dir.path(),
        r#"
[core]
release_branch_prefix = "release"

allowed_repositories = ["myorg/*"]
"#,
    );

    let (allowed, _excluded) =
        resolve_repo_scope(temp_dir.path()).expect("resolve_repo_scope must still succeed");

    assert_eq!(
        allowed,
        vec!["*".to_string()],
        "behavior must be unchanged: the misplaced key still silently falls back \
         to the wildcard default"
    );
    assert!(
        logs_contain("allowed_repositories"),
        "a warning identifying the misplaced key name must be logged"
    );
    assert!(
        logs_contain("release-regent.toml"),
        "a warning identifying the offending config file path must be logged"
    );
}
