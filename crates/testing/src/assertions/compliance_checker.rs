//! Compliance checker for specification compliance

use crate::assertions::{SpecAssertion, SpecTestResult};
use std::collections::HashMap;

/// Compliance requirement definition
#[derive(Debug, Clone)]
pub struct ComplianceRequirement {
    /// Requirement identifier
    pub id: String,
    /// Requirement description
    pub description: String,
    /// Whether this requirement is mandatory
    pub mandatory: bool,
    /// Category of the requirement
    pub category: String,
}

impl ComplianceRequirement {
    /// Create a new compliance requirement
    pub fn new(id: &str, description: &str) -> Self {
        Self {
            id: id.to_string(),
            description: description.to_string(),
            mandatory: true,
            category: "General".to_string(),
        }
    }

    /// Set as optional requirement
    pub fn as_optional(mut self) -> Self {
        self.mandatory = false;
        self
    }

    /// Set requirement category
    pub fn with_category(mut self, category: &str) -> Self {
        self.category = category.to_string();
        self
    }
}

/// Compliance checker for verifying specification compliance
#[derive(Debug)]
pub struct ComplianceChecker {
    /// Specification name
    specification: String,
    /// Compliance requirements
    requirements: Vec<ComplianceRequirement>,
    /// Check results
    results: HashMap<String, bool>,
}

impl ComplianceChecker {
    /// Create a new compliance checker
    ///
    /// # Parameters
    /// - `specification`: Specification name
    ///
    /// # Returns
    /// New compliance checker instance
    pub fn new(specification: &str) -> Self {
        Self {
            specification: specification.to_string(),
            requirements: Vec::new(),
            results: HashMap::new(),
        }
    }

    /// Add a compliance requirement
    ///
    /// # Parameters
    /// - `requirement`: Requirement to add
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_requirement(mut self, requirement: ComplianceRequirement) -> Self {
        self.requirements.push(requirement);
        self
    }

    /// Add a simple requirement
    ///
    /// # Parameters
    /// - `id`: Requirement identifier
    /// - `description`: Requirement description
    ///
    /// # Returns
    /// Self for method chaining
    pub fn with_simple_requirement(mut self, id: &str, description: &str) -> Self {
        self.requirements
            .push(ComplianceRequirement::new(id, description));
        self
    }

    /// Check a specific requirement
    ///
    /// # Parameters
    /// - `requirement_id`: ID of requirement to check
    /// - `checker`: Function to evaluate compliance
    ///
    /// # Returns
    /// Self for method chaining
    pub fn check_requirement<F>(mut self, requirement_id: &str, checker: F) -> Self
    where
        F: FnOnce() -> bool,
    {
        let result = checker();
        self.results.insert(requirement_id.to_string(), result);
        self
    }

    /// Check compliance for all requirements
    ///
    /// # Returns
    /// Compliance test result
    pub fn check_compliance(self) -> SpecTestResult {
        let mut test_result = SpecTestResult::new();

        for requirement in &self.requirements {
            let compliance_result = self.results.get(&requirement.id).unwrap_or(&false);

            let mut assertion = SpecAssertion::new(
                &self.specification,
                &requirement.id,
                &requirement.description,
            )
            .with_actual_behavior(&format!("Compliance: {}", compliance_result))
            .with_metadata("requirement_id", &requirement.id)
            .with_metadata("category", &requirement.category)
            .with_metadata("mandatory", &requirement.mandatory.to_string());

            // Set the result directly based on our check
            assertion.passed = *compliance_result;

            test_result.add_assertion(assertion);
        }

        test_result
    }

    /// Get requirements by category
    ///
    /// # Parameters
    /// - `category`: Category to filter by
    ///
    /// # Returns
    /// Requirements in the specified category
    pub fn requirements_by_category(&self, category: &str) -> Vec<&ComplianceRequirement> {
        self.requirements
            .iter()
            .filter(|req| req.category == category)
            .collect()
    }

    /// Get mandatory requirements
    ///
    /// # Returns
    /// All mandatory requirements
    pub fn mandatory_requirements(&self) -> Vec<&ComplianceRequirement> {
        self.requirements
            .iter()
            .filter(|req| req.mandatory)
            .collect()
    }

    /// Check only mandatory requirements
    ///
    /// # Returns
    /// Whether all mandatory requirements are met
    pub fn check_mandatory_compliance(&self) -> bool {
        self.mandatory_requirements()
            .iter()
            .all(|req| *self.results.get(&req.id).unwrap_or(&false))
    }

    /// Get specification name
    pub fn specification(&self) -> &str {
        &self.specification
    }

    /// Get requirement count
    pub fn requirement_count(&self) -> usize {
        self.requirements.len()
    }

    /// Get compliance rate
    pub fn compliance_rate(&self) -> f64 {
        if self.requirements.is_empty() {
            100.0
        } else {
            let passing = self.results.values().filter(|&&result| result).count();
            (passing as f64 / self.requirements.len() as f64) * 100.0
        }
    }
}
