//! Behavior verifier for testing trait implementations

use crate::assertions::{SpecAssertion, SpecTestResult};
use std::collections::HashMap;

/// Behavior verifier for testing trait implementations
#[derive(Debug)]
pub struct BehaviorVerifier {
    /// Current test context
    context: HashMap<String, String>,
}

impl BehaviorVerifier {
    /// Create a new behavior verifier
    pub fn new() -> Self {
        Self {
            context: HashMap::new(),
        }
    }

    /// Add context information
    ///
    /// # Parameters
    /// - `key`: Context key
    /// - `value`: Context value
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_context(mut self, key: &str, value: &str) -> Self {
        self.context.insert(key.to_string(), value.to_string());
        self
    }

    /// Verify behavior against specification
    ///
    /// # Parameters
    /// - `spec_name`: Specification name
    /// - `expected_behavior`: Expected behavior description
    /// - `actual_behavior`: Actual behavior observed
    ///
    /// # Returns
    /// Spec assertion result
    pub fn verify_behavior(
        &self,
        spec_name: &str,
        expected_behavior: &str,
        actual_behavior: &str,
    ) -> SpecAssertion {
        let mut assertion =
            SpecAssertion::new("Behavior Verification", spec_name, expected_behavior)
                .with_actual_behavior(actual_behavior);

        // Add context as metadata
        for (key, value) in &self.context {
            assertion = assertion.with_metadata(key, value);
        }

        assertion
    }

    /// Verify multiple behaviors
    ///
    /// # Parameters
    /// - `behaviors`: Vector of (spec_name, expected, actual) tuples
    ///
    /// # Returns
    /// Test result with all assertions
    pub fn verify_behaviors(&self, behaviors: Vec<(&str, &str, &str)>) -> SpecTestResult {
        let mut result = SpecTestResult::new();

        for (spec_name, expected, actual) in behaviors {
            let mut assertion = self.verify_behavior(spec_name, expected, actual);
            assertion.evaluate();
            result.add_assertion(assertion);
        }

        result
    }

    /// Verify configuration loading behavior
    ///
    /// # Parameters
    /// - `config_type`: Type of configuration being tested
    /// - `expected_keys`: Expected configuration keys
    /// - `actual_keys`: Actual configuration keys found
    ///
    /// # Returns
    /// Spec assertion result
    pub fn verify_configuration_behavior(
        &self,
        config_type: &str,
        expected_keys: &[&str],
        actual_keys: &[&str],
    ) -> SpecAssertion {
        let expected = format!("Configuration should contain keys: {:?}", expected_keys);
        let actual = format!("Configuration contains keys: {:?}", actual_keys);

        let mut assertion = SpecAssertion::new(
            &format!("{} Configuration", config_type),
            "Configuration Key Presence",
            &expected,
        )
        .with_actual_behavior(&actual)
        .with_metadata("config_type", config_type);

        // Custom evaluation for key presence
        assertion.evaluate_with(|expected_desc, actual_desc| {
            if let Some(actual_desc) = actual_desc {
                // Simple check - all expected keys should be present
                expected_keys.iter().all(|key| actual_desc.contains(key))
            } else {
                false
            }
        });

        assertion
    }

    /// Verify error handling behavior
    ///
    /// # Parameters
    /// - `operation`: Operation being tested
    /// - `expected_error_type`: Expected error type
    /// - `actual_error`: Actual error encountered
    ///
    /// # Returns
    /// Spec assertion result
    pub fn verify_error_handling(
        &self,
        operation: &str,
        expected_error_type: &str,
        actual_error: &str,
    ) -> SpecAssertion {
        let expected = format!("Should produce {} error", expected_error_type);
        let actual = format!("Produced error: {}", actual_error);

        let mut assertion = SpecAssertion::new(
            &format!("{} Error Handling", operation),
            "Error Type Verification",
            &expected,
        )
        .with_actual_behavior(&actual)
        .with_metadata("operation", operation)
        .with_metadata("expected_error_type", expected_error_type);

        // Custom evaluation for error type matching
        assertion.evaluate_contains();

        assertion
    }

    /// Verify async operation behavior
    ///
    /// # Parameters
    /// - `operation`: Async operation name
    /// - `expected_result`: Expected result description
    /// - `actual_result`: Actual result description
    ///
    /// # Returns
    /// Spec assertion result
    pub fn verify_async_behavior(
        &self,
        operation: &str,
        expected_result: &str,
        actual_result: &str,
    ) -> SpecAssertion {
        let mut assertion = SpecAssertion::new(
            &format!("Async {}", operation),
            "Async Operation Behavior",
            expected_result,
        )
        .with_actual_behavior(actual_result)
        .with_metadata("operation_type", "async")
        .with_metadata("operation", operation);

        assertion.evaluate();
        assertion
    }

    /// Get current context
    pub fn context(&self) -> &HashMap<String, String> {
        &self.context
    }
}

impl Default for BehaviorVerifier {
    fn default() -> Self {
        Self::new()
    }
}
