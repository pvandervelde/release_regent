use super::*;

#[test]
fn test_error_api_variant() {
    let error = Error::Api {
        message: "Repository not found".to_string(),
        source: None,
    };

    assert_eq!(error.to_string(), "GitHub API error: Repository not found");
}

#[test]
fn test_error_auth_variant() {
    let error = Error::Auth {
        message: "Invalid credentials".to_string(),
        source: None,
    };

    assert_eq!(
        error.to_string(),
        "Authentication error: Invalid credentials"
    );
}

#[test]
fn test_error_network_variant() {
    let error = Error::Network {
        message: "Connection timeout".to_string(),
        source: None,
    };

    assert_eq!(error.to_string(), "Network error: Connection timeout");
}

#[test]
fn test_error_not_found_variant() {
    let error = Error::NotFound {
        resource: "release:v1.0.0".to_string(),
    };

    assert_eq!(error.to_string(), "Resource not found: release:v1.0.0");
}

#[test]
fn test_error_invalid_input_variant() {
    let error = Error::InvalidInput {
        message: "Tag name cannot be empty".to_string(),
    };

    assert_eq!(error.to_string(), "Invalid input: Tag name cannot be empty");
}

#[test]
fn test_error_rate_limit_variant() {
    let error = Error::RateLimit;
    assert_eq!(error.to_string(), "Rate limit exceeded");
}

#[test]
fn test_error_other_variant() {
    let error = Error::Other {
        message: "Unknown error occurred".to_string(),
        source: None,
    };

    assert_eq!(
        error.to_string(),
        "GitHub client error: Unknown error occurred"
    );
}

#[test]
fn test_error_to_core_error_api() {
    let error = Error::Api {
        message: "API error".to_string(),
        source: None,
    };

    let core_error: release_regent_core::CoreError = error.into();
    match core_error {
        release_regent_core::CoreError::GitHub { .. } => {}
        _ => panic!("Expected GitHub CoreError variant"),
    }
}

#[test]
fn test_error_to_core_error_auth() {
    let error = Error::Auth {
        message: "Auth error".to_string(),
        source: None,
    };

    let core_error: release_regent_core::CoreError = error.into();
    match core_error {
        release_regent_core::CoreError::GitHub { .. } => {}
        _ => panic!("Expected GitHub CoreError variant"),
    }
}

#[test]
fn test_error_to_core_error_network() {
    let error = Error::Network {
        message: "Network error".to_string(),
        source: None,
    };

    let core_error: release_regent_core::CoreError = error.into();
    match core_error {
        release_regent_core::CoreError::Network { .. } => {}
        _ => panic!("Expected Network CoreError variant"),
    }
}

#[test]
fn test_error_to_core_error_not_found() {
    let error = Error::NotFound {
        resource: "test".to_string(),
    };

    let core_error: release_regent_core::CoreError = error.into();
    match core_error {
        release_regent_core::CoreError::GitHub { .. } => {}
        _ => panic!("Expected GitHub CoreError variant"),
    }
}

#[test]
fn test_error_to_core_error_rate_limit() {
    let error = Error::RateLimit;

    let core_error: release_regent_core::CoreError = error.into();
    match core_error {
        release_regent_core::CoreError::RateLimit { .. } => {}
        _ => panic!("Expected RateLimit CoreError variant"),
    }
}
