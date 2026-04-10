use super::*;

// ============================================================================
// Constructor and conversion tests
// ============================================================================

#[test]
fn test_config_error_creation() {
    let error = CoreError::config("Invalid YAML format");

    match error {
        CoreError::Config { ref message, .. } => {
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
        CoreError::InternalState { ref message, .. } => {
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
            ..
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
            ..
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
        CoreError::Versioning { ref reason, .. } => {
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
            ..
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

/// Verify `not_found()` creates a `NotFound` variant with the correct resource string and display text.
#[test]
fn test_not_found_error_creation() {
    let error = CoreError::not_found("release for tag 'v1.0.0' not found");

    match error {
        CoreError::NotFound { ref resource, .. } => {
            assert_eq!(resource, "release for tag 'v1.0.0' not found");
        }
        _ => panic!("Expected NotFound error"),
    }

    assert_eq!(
        error.to_string(),
        "Not found: release for tag 'v1.0.0' not found"
    );

    // context() must not silently discard an attached context
    let ctx = ErrorContext::new("get_release_by_tag", "mock");
    let with_ctx = CoreError::NotFound {
        resource: "r".to_string(),
        context: Some(ctx),
    };
    assert!(with_ctx.context().is_some());
    assert!(CoreError::not_found("r").context().is_none());
}

#[test]
fn test_yaml_error_conversion() {
    let yaml_error =
        serde_yaml::from_str::<serde_yaml::Value>("invalid: yaml: content:").unwrap_err();
    let core_error = CoreError::from(yaml_error);

    match core_error {
        CoreError::YamlParsing { .. } => {}
        _ => panic!("Expected YamlParsing error from serde_yaml::Error"),
    }
}

#[test]
fn test_changelog_generation_error_creation() {
    let error = CoreError::changelog_generation("Failed to parse commit message");

    match error {
        CoreError::ChangelogGeneration { ref message, .. } => {
            assert_eq!(message, "Failed to parse commit message");
        }
        _ => panic!("Expected ChangelogGeneration error"),
    }

    assert_eq!(
        error.to_string(),
        "Changelog generation failed: Failed to parse commit message"
    );
}

#[test]
fn test_validation_error_creation() {
    let error = CoreError::validation("email", "Invalid email format");

    match error {
        CoreError::Validation {
            ref field,
            ref message,
            ..
        } => {
            assert_eq!(field, "email");
            assert_eq!(message, "Invalid email format");
        }
        _ => panic!("Expected Validation error"),
    }

    assert_eq!(
        error.to_string(),
        "Validation failed: email - Invalid email format"
    );
}

#[test]
fn test_authentication_error_creation() {
    let error = CoreError::authentication("Invalid token");

    match error {
        CoreError::Authentication { ref message, .. } => {
            assert_eq!(message, "Invalid token");
        }
        _ => panic!("Expected Authentication error"),
    }

    assert_eq!(error.to_string(), "Authentication error: Invalid token");
}

// ============================================================================
// ErrorContext tests
// ============================================================================

#[test]
fn test_error_context_creation() {
    let context = ErrorContext::new("test_operation", "test_component")
        .with_data("key1", "value1")
        .with_correlation_id("test-123");

    assert_eq!(context.operation, "test_operation");
    assert_eq!(context.component, "test_component");
    assert_eq!(
        context.context_data.get("key1"),
        Some(&"value1".to_string())
    );
    assert_eq!(context.correlation_id, Some("test-123".to_string()));
}

#[test]
fn test_error_with_context() {
    let context =
        ErrorContext::new("config_load", "config_provider").with_data("file", "config.yaml");

    let error = CoreError::config_with_context("Failed to load config", context);

    let retrieved_context = error.context().unwrap();
    assert_eq!(retrieved_context.operation, "config_load");
    assert_eq!(retrieved_context.component, "config_provider");
    assert_eq!(
        retrieved_context.context_data.get("file"),
        Some(&"config.yaml".to_string())
    );
}

// ============================================================================
// is_retryable — exhaustive classification tests for all 19 CoreError variants
// ============================================================================

// --- Retryable variants: transient failures where the caller should back off and retry ---

#[test]
fn test_is_retryable_network_is_retryable() {
    assert!(CoreError::network("Connection refused").is_retryable());
}

#[test]
fn test_is_retryable_rate_limit_is_retryable() {
    assert!(CoreError::rate_limit("API quota exceeded").is_retryable());
    assert!(CoreError::rate_limit_with_retry("quota", 60).is_retryable());
}

#[test]
fn test_is_retryable_timeout_is_retryable() {
    assert!(CoreError::timeout("GitHub API request", 30_000).is_retryable());
}

#[test]
fn test_is_retryable_conflict_is_retryable() {
    // Conflict = optimistic-lock collision; caller must re-fetch the resource and retry.
    assert!(CoreError::conflict("branch 'release/v1.0.0' already exists").is_retryable());
}

// --- Permanent variants: errors that will not resolve on retry ---

#[test]
fn test_is_retryable_authentication_not_retryable() {
    assert!(!CoreError::authentication("GitHub token invalid (401)").is_retryable());
}

#[test]
fn test_is_retryable_changelog_generation_not_retryable() {
    assert!(!CoreError::changelog_generation("Failed to parse commit").is_retryable());
}

#[test]
fn test_is_retryable_config_not_retryable() {
    assert!(!CoreError::config("Missing required field: repo").is_retryable());
}

#[test]
fn test_is_retryable_github_not_retryable() {
    let inner = std::io::Error::new(std::io::ErrorKind::Other, "unprocessable entity");
    assert!(!CoreError::github(inner).is_retryable());
}

#[test]
fn test_is_retryable_internal_state_not_retryable() {
    assert!(!CoreError::internal_state("Inconsistent processor state").is_retryable());
}

#[test]
fn test_is_retryable_invalid_input_not_retryable() {
    assert!(!CoreError::invalid_input("version", "not a valid semver string").is_retryable());
}

#[test]
fn test_is_retryable_io_not_retryable() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "config file missing");
    assert!(!CoreError::from(io_err).is_retryable());
}

#[test]
fn test_is_retryable_json_parsing_not_retryable() {
    let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
    assert!(!CoreError::from(json_err).is_retryable());
}

#[test]
fn test_is_retryable_not_found_not_retryable() {
    assert!(!CoreError::not_found("release for tag 'v1.0.0'").is_retryable());
}

#[test]
fn test_is_retryable_not_supported_not_retryable() {
    assert!(!CoreError::not_supported("!release major", "task not yet implemented").is_retryable());
}

#[test]
fn test_is_retryable_toml_parsing_not_retryable() {
    // Unclosed inline table is invalid TOML.
    let toml_err = toml::from_str::<toml::Value>("key = {unclosed").unwrap_err();
    assert!(!CoreError::from(toml_err).is_retryable());
}

#[test]
fn test_is_retryable_validation_not_retryable() {
    assert!(!CoreError::validation("tag_name", "must match vX.Y.Z").is_retryable());
}

#[test]
fn test_is_retryable_versioning_not_retryable() {
    assert!(!CoreError::versioning("no conventional commits found").is_retryable());
}

#[test]
fn test_is_retryable_webhook_not_retryable() {
    assert!(!CoreError::webhook("signature_validation", "HMAC mismatch").is_retryable());
}

#[test]
fn test_is_retryable_yaml_parsing_not_retryable() {
    let yaml_err =
        serde_yaml::from_str::<serde_yaml::Value>("invalid: yaml: content:").unwrap_err();
    assert!(!CoreError::from(yaml_err).is_retryable());
}

// ============================================================================
// retry_delay_seconds tests
// ============================================================================

#[test]
fn test_retry_delay_seconds_retryable_variants() {
    // Network errors: default 1-second delay.
    assert_eq!(CoreError::network("timeout").retry_delay_seconds(), Some(1));
    // Timeout errors: default 2-second delay.
    assert_eq!(
        CoreError::timeout("op", 5_000).retry_delay_seconds(),
        Some(2)
    );
    // Rate limit with explicit hint: honour the hint value.
    assert_eq!(
        CoreError::rate_limit_with_retry("quota", 60).retry_delay_seconds(),
        Some(60)
    );
    // Conflict is retryable but has no prescribed delay; re-fetch-and-retry is immediate.
    assert_eq!(
        CoreError::conflict("branch exists").retry_delay_seconds(),
        None
    );
}

#[test]
fn test_retry_delay_seconds_permanent_variants_return_none() {
    assert_eq!(CoreError::config("bad config").retry_delay_seconds(), None);
    assert_eq!(
        CoreError::authentication("no token").retry_delay_seconds(),
        None
    );
    assert_eq!(
        CoreError::invalid_input("f", "v").retry_delay_seconds(),
        None
    );
    assert_eq!(CoreError::not_found("resource").retry_delay_seconds(), None);
}
