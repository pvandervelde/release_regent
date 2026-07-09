use super::*;

#[test]
fn test_authentication_error_creation() {
    let error = Error::authentication("Invalid GitHub App token");

    match error {
        Error::Authentication { ref message } => {
            assert_eq!(message, "Invalid GitHub App token");
        }
        _ => panic!("Expected Authentication error"),
    }

    assert_eq!(
        error.to_string(),
        "Authentication failed: Invalid GitHub App token"
    );
}

#[test]
fn test_environment_error_creation() {
    let error = Error::environment("GITHUB_APP_ID", "Environment variable not set");

    match error {
        Error::Environment {
            ref variable,
            ref message,
        } => {
            assert_eq!(variable, "GITHUB_APP_ID");
            assert_eq!(message, "Environment variable not set");
        }
        _ => panic!("Expected Environment error"),
    }

    assert_eq!(
        error.to_string(),
        "Environment configuration error: GITHUB_APP_ID - Environment variable not set"
    );
}

#[test]
fn test_internal_error_creation() {
    let error = Error::internal("Unexpected state during processing");

    match error {
        Error::Internal { ref message } => {
            assert_eq!(message, "Unexpected state during processing");
        }
        _ => panic!("Expected Internal error"),
    }

    assert_eq!(
        error.to_string(),
        "Internal processing error: Unexpected state during processing"
    );
}

#[test]
fn test_invalid_repo_pattern_error_creation_for_allowed_list() {
    let error = Error::invalid_repo_pattern("allowed_repos", "myorg/[", "unterminated bracket");

    match error {
        Error::InvalidRepoPattern {
            ref list_name,
            ref pattern,
            ref message,
        } => {
            assert_eq!(list_name, "allowed_repos");
            assert_eq!(pattern, "myorg/[");
            assert_eq!(message, "unterminated bracket");
        }
        _ => panic!("Expected InvalidRepoPattern error"),
    }

    assert_eq!(
        error.to_string(),
        "Invalid glob pattern in allowed_repos: 'myorg/[' - unterminated bracket"
    );
}

/// BA-75 requires the error to identify *which list* (exclude vs allow) the
/// offending pattern came from. This test uses `"excluded_repos"` specifically
/// so a hardcoded/constant `list_name` in the implementation cannot pass both
/// this test and the sibling `allowed_repos` test above.
#[test]
fn test_invalid_repo_pattern_error_creation_for_excluded_list() {
    let error = Error::invalid_repo_pattern("excluded_repos", "myorg/**[", "invalid range");

    match error {
        Error::InvalidRepoPattern { ref list_name, .. } => {
            assert_eq!(list_name, "excluded_repos");
        }
        _ => panic!("Expected InvalidRepoPattern error"),
    }

    assert!(
        error.to_string().contains("excluded_repos"),
        "error message must identify the excluded_repos list, got: {error}"
    );
}

#[test]
fn test_json_error_conversion() {
    let json_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
    let function_error = Error::from(json_error);

    match function_error {
        Error::Json { .. } => {
            // Expected
        }
        _ => panic!("Expected Json error from serde_json::Error"),
    }
}
