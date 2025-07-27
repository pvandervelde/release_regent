//! Spec test runner for executing specification tests

use crate::assertions::{SpecAssertion, SpecTestResult};
use release_regent_core::CoreResult;

/// Spec test runner for executing behavioral tests
#[derive(Debug)]
pub struct SpecRunner {
    /// Test name
    name: String,
    /// Assertions to run
    assertions: Vec<SpecAssertion>,
}

impl SpecRunner {
    /// Create a new spec runner
    ///
    /// # Parameters
    /// - `name`: Test name
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

    /// Run all assertions
    ///
    /// # Returns
    /// Test result summary
    pub fn run(self) -> SpecTestResult {
        let mut result = SpecTestResult::new();

        for mut assertion in self.assertions {
            assertion.evaluate();
            result.add_assertion(assertion);
        }

        result
    }
}
