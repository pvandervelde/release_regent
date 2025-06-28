use super::*;

#[test]
fn test_authentication_error_creation() {
    let error = GitHubError::authentication("Invalid token");

    match error {
        GitHubError::Authentication { ref message } => {
            assert_eq!(message, "Invalid token");
        }
        _ => panic!("Expected Authentication error"),
    }

    assert_eq!(
        error.to_string(),
        "GitHub authentication failed: Invalid token"
    );
}

#[test]
fn test_api_request_error_creation() {
    let error = GitHubError::api_request(404, "Repository not found");

    match error {
        GitHubError::ApiRequest {
            status,
            ref message,
        } => {
            assert_eq!(status, 404);
            assert_eq!(message, "Repository not found");
        }
        _ => panic!("Expected ApiRequest error"),
    }

    assert_eq!(
        error.to_string(),
        "GitHub API request failed: 404 - Repository not found"
    );
}

#[test]
fn test_rate_limit_error_creation() {
    let reset_time = "2025-06-26T12:00:00Z";
    let error = GitHubError::rate_limit(reset_time);

    match error {
        GitHubError::RateLimit { reset_time: ref rt } => {
            assert_eq!(rt, reset_time);
        }
        _ => panic!("Expected RateLimit error"),
    }

    assert_eq!(
        error.to_string(),
        "GitHub API rate limit exceeded. Reset at: 2025-06-26T12:00:00Z"
    );
}

#[test]
fn test_not_found_error_creation() {
    let error = GitHubError::not_found("Pull Request", "123");

    match error {
        GitHubError::NotFound {
            ref resource_type,
            ref resource_id,
        } => {
            assert_eq!(resource_type, "Pull Request");
            assert_eq!(resource_id, "123");
        }
        _ => panic!("Expected NotFound error"),
    }

    assert_eq!(
        error.to_string(),
        "GitHub resource not found: Pull Request '123'"
    );
}

#[test]
fn test_permission_denied_error_creation() {
    let error = GitHubError::permission_denied("create release");

    match error {
        GitHubError::PermissionDenied { ref operation } => {
            assert_eq!(operation, "create release");
        }
        _ => panic!("Expected PermissionDenied error"),
    }

    assert_eq!(
        error.to_string(),
        "Insufficient permissions for GitHub operation: create release"
    );
}

#[test]
fn test_invalid_input_error_creation() {
    let error = GitHubError::invalid_input("version", "Invalid semantic version format");

    match error {
        GitHubError::InvalidInput {
            ref field,
            ref message,
        } => {
            assert_eq!(field, "version");
            assert_eq!(message, "Invalid semantic version format");
        }
        _ => panic!("Expected InvalidInput error"),
    }

    assert_eq!(
        error.to_string(),
        "Invalid input for GitHub API: version - Invalid semantic version format"
    );
}

#[test]
fn test_error_from_conversion() {
    let json_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
    let github_error = GitHubError::from(json_error);

    match github_error {
        GitHubError::Parsing { .. } => {
            // Expected
        }
        _ => panic!("Expected Parsing error from serde_json::Error"),
    }
}
