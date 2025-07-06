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
fn test_http_request_error_creation() {
    let error = Error::http_request(400, "Bad Request");

    match error {
        Error::HttpRequest {
            status,
            ref message,
        } => {
            assert_eq!(status, 400);
            assert_eq!(message, "Bad Request");
        }
        _ => panic!("Expected HttpRequest error"),
    }

    assert_eq!(error.to_string(), "HTTP request error: 400 - Bad Request");
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
