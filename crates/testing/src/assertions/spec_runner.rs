//! Spec test runner for executing behavioral assertions
//!
//! This module provides utilities for running specification tests
//! and collecting results.

use crate::assertions::{SpecAssertion, SpecTestResult};

/// Test runner for executing specification tests
#[derive(Debug)]
pub struct SpecRunner {
    /// Name of the test suite
    pub name: String,
    /// Collection of assertions to run
    pub assertions: Vec<SpecAssertion>,
}

impl SpecRunner {
    /// Create a new spec runner
    ///
    /// # Parameters
    /// - `name`: Name of the test suite
    ///
    /// # Returns
    /// New spec runner instance
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            assertions: Vec::new(),
        }
    }

    /// Add an assertion to the runner
    ///
    /// # Parameters
    /// - `assertion`: Assertion to add
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_assertion(mut self, assertion: SpecAssertion) -> Self {
        self.assertions.push(assertion);
        self
    }

    /// Add multiple assertions to the runner
    ///
    /// # Parameters
    /// - `assertions`: Assertions to add
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_assertions(mut self, assertions: Vec<SpecAssertion>) -> Self {
        self.assertions.extend(assertions);
        self
    }

    /// Run all assertions
    ///
    /// # Returns
    /// Test result with all assertion outcomes
    pub fn run(mut self) -> SpecTestResult {
        let mut result = SpecTestResult::new();

        for mut assertion in self.assertions {
            assertion.evaluate();
            result.add_assertion(assertion);
        }

        result
    }

    /// Run all assertions with custom evaluator
    ///
    /// # Parameters
    /// - `evaluator`: Custom evaluation function
    ///
    /// # Returns
    /// Test result with all assertion outcomes
    pub fn run_with_evaluator<F>(mut self, evaluator: F) -> SpecTestResult
    where
        F: Fn(&str, &Option<String>) -> bool,
    {
        let mut result = SpecTestResult::new();

        for mut assertion in self.assertions {
            assertion.evaluate_with(&evaluator);
            result.add_assertion(assertion);
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spec_runner_creation() {
        let runner = SpecRunner::new("Test Suite");
        assert_eq!(runner.name, "Test Suite");
        assert_eq!(runner.assertions.len(), 0);
    }

    #[test]
    fn test_spec_runner_with_assertion() {
        let assertion = SpecAssertion::new("Subject", "Spec", "Expected");
        let runner = SpecRunner::new("Test Suite").with_assertion(assertion);
        assert_eq!(runner.assertions.len(), 1);
    }

    #[test]
    fn test_spec_runner_run() {
        let assertion =
            SpecAssertion::new("Subject", "Spec", "Expected").with_actual_behavior("Expected");

        let result = SpecRunner::new("Test Suite")
            .with_assertion(assertion)
            .run();

        assert_eq!(result.total_assertions, 1);
        assert_eq!(result.passed_assertions, 1);
        assert!(result.success);
    }

    #[test]
    fn test_spec_runner_with_custom_evaluator() {
        let assertion =
            SpecAssertion::new("Subject", "Spec", "Expected").with_actual_behavior("Different");

        let result = SpecRunner::new("Test Suite")
            .with_assertion(assertion)
            .run_with_evaluator(|_expected, actual| {
                actual.as_ref().map_or(false, |a| a.contains("Different"))
            });

        assert_eq!(result.total_assertions, 1);
        assert_eq!(result.passed_assertions, 1);
        assert!(result.success);
    }
}
