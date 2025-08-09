//! Unit tests for configuration validation.

use super::*;
use release_regent_core::config::*;

#[test]
fn test_validation_result_creation() {
    let valid = ValidationResult::valid();
    assert!(valid.is_valid);
    assert!(valid.errors.is_empty());
    assert!(valid.warnings.is_empty());

    let invalid = ValidationResult::invalid(vec!["error1".to_string()]);
    assert!(!invalid.is_valid);
    assert_eq!(invalid.errors.len(), 1);
}

#[test]
fn test_validation_result_with_warnings() {
    let result = ValidationResult::valid()
        .with_warning("warning1".to_string())
        .with_warnings(vec!["warning2".to_string(), "warning3".to_string()]);

    assert!(result.is_valid);
    assert_eq!(result.warnings.len(), 3);
    assert!(result.has_issues());
}

#[test]
fn test_config_validator_creation() {
    let validator = ConfigValidator::new();
    assert!(!validator.strict_mode);

    let strict_validator = ConfigValidator::strict();
    assert!(strict_validator.strict_mode);
}

#[test]
fn test_basic_config_validation() {
    let validator = ConfigValidator::new();

    // Create a minimal valid config using the actual current structure
    let config = ReleaseRegentConfig::default();

    let result = validator.validate(&config).unwrap();
    assert!(result.is_valid);
}

#[test]
fn test_invalid_config_validation() {
    let validator = ConfigValidator::new();

    // Create an invalid config with empty main branch
    let mut config = ReleaseRegentConfig::default();
    config.core.branches.main = "".to_string(); // Empty main branch

    let result = validator.validate(&config).unwrap();
    assert!(!result.is_valid);
    assert!(!result.errors.is_empty());
}
