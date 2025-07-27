//! Test fixtures for webhooks and API responses
//!
//! This module provides pre-built test data for common scenarios
//! including webhook payloads and GitHub API responses.

use serde_json::Value;
use std::collections::HashMap;

pub mod github_api_fixtures;
pub mod webhook_fixtures;

pub use github_api_fixtures::*;
pub use webhook_fixtures::*;

/// Common fixture data provider
#[derive(Debug, Clone)]
pub struct FixtureProvider {
    /// Pre-loaded fixture data
    fixtures: HashMap<String, Value>,
}

impl FixtureProvider {
    /// Create a new fixture provider
    ///
    /// # Returns
    /// Fixture provider with pre-loaded common fixtures
    pub fn new() -> Self {
        let mut provider = Self {
            fixtures: HashMap::new(),
        };
        provider.load_common_fixtures();
        provider
    }

    /// Load all common fixtures into memory
    fn load_common_fixtures(&mut self) {
        // TODO: implement - placeholder for compilation
        // This will load common webhook payloads and API responses
    }

    /// Get a fixture by name
    ///
    /// # Parameters
    /// - `name`: Fixture name
    ///
    /// # Returns
    /// Fixture data if found
    pub fn get_fixture(&self, name: &str) -> Option<&Value> {
        self.fixtures.get(name)
    }

    /// Add a custom fixture
    ///
    /// # Parameters
    /// - `name`: Fixture name
    /// - `data`: Fixture data
    pub fn add_fixture(&mut self, name: &str, data: Value) {
        self.fixtures.insert(name.to_string(), data);
    }
}

impl Default for FixtureProvider {
    fn default() -> Self {
        Self::new()
    }
}
