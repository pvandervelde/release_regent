use super::*;

#[test]
fn test_config_error_creation() {
    let error = CoreError::config("Invalid YAML format");

    match error {
        CoreError::Config { ref message } => {
            assert_eq!(message, "Invalid YAML format");
        }
        _ => panic!("Expected Config error"),
    }

    assert_eq!(
        error.to_string(),
        "Configuration error: Invalid YAML format"
    );
}

#[test]
fn test_internal_state_error_creation() {
    let error = CoreError::internal_state("Unexpected state transition");

    match error {
        CoreError::InternalState { ref message } => {
            assert_eq!(message, "Unexpected state transition");
        }
        _ => panic!("Expected InternalState error"),
    }

    assert_eq!(
        error.to_string(),
        "Internal state error: Unexpected state transition"
    );
}

#[test]
fn test_invalid_input_error_creation() {
    let error = CoreError::invalid_input("branch_name", "Invalid characters in branch name");

    match error {
        CoreError::InvalidInput {
            ref field,
            ref message,
        } => {
            assert_eq!(field, "branch_name");
            assert_eq!(message, "Invalid characters in branch name");
        }
        _ => panic!("Expected InvalidInput error"),
    }

    assert_eq!(
        error.to_string(),
        "Invalid input: branch_name - Invalid characters in branch name"
    );
}

#[test]
fn test_io_error_conversion() {
    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
    let core_error = CoreError::from(io_error);

    match core_error {
        CoreError::Io { .. } => {
            // Expected
        }
        _ => panic!("Expected Io error from std::io::Error"),
    }
}

#[test]
fn test_not_supported_error_creation() {
    let error = CoreError::not_supported("external_versioning", "No external script configured");

    match error {
        CoreError::NotSupported {
            ref operation,
            ref context,
        } => {
            assert_eq!(operation, "external_versioning");
            assert_eq!(context, "No external script configured");
        }
        _ => panic!("Expected NotSupported error"),
    }

    assert_eq!(
        error.to_string(),
        "Operation not supported: external_versioning - No external script configured"
    );
}

#[test]
fn test_versioning_error_creation() {
    let error = CoreError::versioning("No conventional commits found");

    match error {
        CoreError::Versioning { ref reason } => {
            assert_eq!(reason, "No conventional commits found");
        }
        _ => panic!("Expected Versioning error"),
    }

    assert_eq!(
        error.to_string(),
        "Version calculation failed: No conventional commits found"
    );
}

#[test]
fn test_webhook_error_creation() {
    let error = CoreError::webhook("signature_validation", "Invalid signature");

    match error {
        CoreError::Webhook {
            ref stage,
            ref message,
        } => {
            assert_eq!(stage, "signature_validation");
            assert_eq!(message, "Invalid signature");
        }
        _ => panic!("Expected Webhook error"),
    }

    assert_eq!(
        error.to_string(),
        "Webhook processing failed: signature_validation - Invalid signature"
    );
}

#[test]
fn test_yaml_error_conversion() {
    let yaml_str = "invalid: yaml: content:";
    let yaml_error = serde_yaml::from_str::<serde_yaml::Value>(yaml_str).unwrap_err();
    let core_error = CoreError::from(yaml_error);

    match core_error {
        CoreError::YamlParsing { .. } => {
            // Expected
        }
        _ => panic!("Expected YamlParsing error from serde_yaml::Error"),
    }
}
