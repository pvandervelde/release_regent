//! Spec testing framework examples
//!
//! This module provides practical examples of how to use the spec testing framework
//! for behavioral assertions and compliance checking.

use crate::assertions::spec_runner::SpecRunner;
use crate::assertions::{
    BehaviorVerifier, ComplianceChecker, ComplianceRequirement, SpecAssertion,
};

/// Example: Basic spec assertion
///
/// # Returns
/// Example spec assertion result
pub fn example_basic_assertion() -> SpecAssertion {
    let mut assertion = SpecAssertion::new(
        "Version Calculator",
        "Semantic Versioning Compliance",
        "Incremented major version from 1.0.0 to 2.0.0",
    )
    .with_actual_behavior("Incremented major version from 1.0.0 to 2.0.0")
    .with_metadata("test_case", "breaking_change_detection");

    // Use exact evaluation since the strings match exactly
    assertion.evaluate();
    assertion
}

/// Example: Behavior verification
///
/// # Returns
/// Example behavior verification result
pub fn example_behavior_verification() -> crate::assertions::SpecTestResult {
    let verifier = BehaviorVerifier::new()
        .with_context("environment", "test")
        .with_context("version", "1.0.0");

    verifier.verify_behaviors(vec![
        (
            "GitHub API Interaction",
            "Authentication successful with token",
            "Authentication successful with token",
        ),
        (
            "Error Handling",
            "Received 403 rate limit error, waiting 60 seconds",
            "Received 403 rate limit error, waiting 60 seconds",
        ),
        (
            "Webhook Processing",
            "Successfully parsed GitHub webhook JSON payload",
            "Successfully parsed GitHub webhook JSON payload",
        ),
    ])
}

/// Example: Configuration compliance checking
///
/// # Returns
/// Example compliance check result
pub fn example_configuration_compliance() -> crate::assertions::SpecTestResult {
    ComplianceChecker::new("GitHub Configuration Specification")
        .with_requirement(
            ComplianceRequirement::new("github_token", "Must provide valid GitHub token")
                .with_category("Authentication"),
        )
        .with_requirement(
            ComplianceRequirement::new(
                "webhook_secret",
                "Must provide webhook secret for security",
            )
            .with_category("Security"),
        )
        .with_requirement(
            ComplianceRequirement::new("repository_config", "Must specify target repository")
                .with_category("Configuration"),
        )
        .with_requirement(
            ComplianceRequirement::new("debug_mode", "May enable debug mode for development")
                .as_optional()
                .with_category("Development"),
        )
        .check_requirement("github_token", || {
            // Simulate checking for GitHub token
            std::env::var("GITHUB_TOKEN").is_ok()
        })
        .check_requirement("webhook_secret", || {
            // Simulate checking for webhook secret
            true // Assume secret is configured
        })
        .check_requirement("repository_config", || {
            // Simulate checking repository configuration
            true // Assume repository is configured
        })
        .check_requirement("debug_mode", || {
            // Optional debug mode
            false // Not enabled
        })
        .check_compliance()
}

/// Example: Complete spec test suite
///
/// # Returns
/// Example complete test result
pub fn example_complete_spec_test() -> crate::assertions::SpecTestResult {
    let assertion1 = SpecAssertion::new(
        "Release Automation",
        "Conventional Commits",
        "Detected 'feat:' commit, incremented minor version",
    )
    .with_actual_behavior("Detected 'feat:' commit, incremented minor version")
    .with_metadata("commit_type", "feat")
    .with_metadata("version_change", "1.0.0 -> 1.1.0");

    let assertion2 = SpecAssertion::new(
        "Release Automation",
        "Breaking Changes",
        "Detected BREAKING CHANGE footer, incremented major version",
    )
    .with_actual_behavior("Detected BREAKING CHANGE footer, incremented major version")
    .with_metadata("commit_type", "feat")
    .with_metadata("breaking_change", "true")
    .with_metadata("version_change", "1.1.0 -> 2.0.0");

    let assertion3 = SpecAssertion::new(
        "GitHub Integration",
        "Release Creation",
        "Created release v2.0.0 with generated changelog",
    )
    .with_actual_behavior("Created release v2.0.0 with generated changelog")
    .with_metadata("release_tag", "v2.0.0")
    .with_metadata("changelog_generated", "true");

    SpecRunner::new("Release Regent Specification Compliance")
        .with_assertions(vec![assertion1, assertion2, assertion3])
        .run()
}

/// Example: Error handling specification
///
/// # Returns
/// Example error handling verification
pub fn example_error_handling_spec() -> crate::assertions::SpecTestResult {
    let verifier = BehaviorVerifier::new()
        .with_context("test_scenario", "error_conditions")
        .with_context("environment", "integration_test");

    let mut result = verifier.verify_behaviors(vec![
        (
            "GitHub API Errors",
            "Should handle network timeouts gracefully",
            "Caught timeout error, retried 3 times",
        ),
        (
            "Invalid Configuration",
            "Should provide clear error messages",
            "Error: Missing required field 'github_token'",
        ),
        (
            "Webhook Validation",
            "Should reject malformed payloads",
            "Rejected payload: invalid JSON structure",
        ),
    ]);

    // Add error-specific verification
    let error_assertion = verifier.verify_error_handling(
        "Token Validation",
        "Authentication",
        "401 Unauthorized: Bad credentials",
    );

    result.add_assertion(error_assertion);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_assertion_example() {
        let assertion = example_basic_assertion();
        assert!(assertion.passed());
        assert_eq!(assertion.subject, "Version Calculator");
    }

    #[test]
    fn test_behavior_verification_example() {
        let result = example_behavior_verification();
        assert!(result.success);
        assert_eq!(result.total_assertions, 3);
    }

    #[test]
    fn test_configuration_compliance_example() {
        let result = example_configuration_compliance();
        // Result depends on environment variables, but should complete without errors
        assert!(result.total_assertions > 0);
    }

    #[test]
    fn test_complete_spec_test_example() {
        let result = example_complete_spec_test();
        assert_eq!(result.total_assertions, 3);
        assert!(result.pass_rate() > 0.0);
    }

    #[test]
    fn test_error_handling_spec_example() {
        let result = example_error_handling_spec();
        assert!(result.total_assertions >= 4); // 3 behaviors + 1 error handling
    }
}
