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
        // Load common webhook payloads
        self.fixtures
            .insert("webhook.push.simple".to_string(), push_event_simple());
        self.fixtures.insert(
            "webhook.push.with_commits".to_string(),
            push_event_with_commits(),
        );
        self.fixtures.insert(
            "webhook.pull_request.opened".to_string(),
            pull_request_opened(),
        );
        self.fixtures.insert(
            "webhook.pull_request.merged".to_string(),
            pull_request_merged(),
        );
        self.fixtures
            .insert("webhook.release.published".to_string(), release_published());
        self.fixtures
            .insert("webhook.release.draft".to_string(), release_draft());

        // Load common API responses
        self.fixtures.insert(
            "api.repository.sample".to_string(),
            sample_repository_json(),
        );
        self.fixtures.insert(
            "api.pull_request.sample".to_string(),
            sample_pull_request_json(),
        );
        self.fixtures
            .insert("api.release.sample".to_string(), sample_release_json());
        self.fixtures
            .insert("api.commits.list".to_string(), sample_commits_list());
        self.fixtures.insert(
            "api.error.rate_limit".to_string(),
            rate_limit_exceeded_response(),
        );
        self.fixtures.insert(
            "api.error.not_found".to_string(),
            error_response(404, "Not Found"),
        );
        self.fixtures.insert(
            "api.error.unauthorized".to_string(),
            error_response(401, "Bad credentials"),
        );
    }

    /// Get a webhook fixture by event type and action
    ///
    /// # Parameters
    /// - `event_type`: GitHub event type (e.g., "push", "pull_request", "release")
    /// - `action`: Event action (e.g., "opened", "closed", "published")
    ///
    /// # Returns
    /// Webhook payload data if found
    pub fn get_webhook_fixture(&self, event_type: &str, action: &str) -> Option<&Value> {
        let key = format!("webhook.{}.{}", event_type, action);
        self.fixtures.get(&key)
    }

    /// Get an API response fixture by resource type
    ///
    /// # Parameters
    /// - `resource`: API resource type (e.g., "repository", "pull_request", "release")
    /// - `variant`: Response variant (e.g., "sample", "error.not_found")
    ///
    /// # Returns
    /// API response data if found
    pub fn get_api_fixture(&self, resource: &str, variant: &str) -> Option<&Value> {
        let key = format!("api.{}.{}", resource, variant);
        self.fixtures.get(&key)
    }

    /// Get all fixtures matching a pattern
    ///
    /// # Parameters
    /// - `pattern`: Pattern to match (e.g., "webhook.*", "api.error.*")
    ///
    /// # Returns
    /// Map of matching fixtures
    pub fn get_fixtures_matching(&self, pattern: &str) -> HashMap<String, &Value> {
        let mut matches = HashMap::new();
        let pattern_prefix = pattern.trim_end_matches('*');

        for (key, value) in &self.fixtures {
            if key.starts_with(pattern_prefix) {
                matches.insert(key.clone(), value);
            }
        }

        matches
    }

    /// List all available fixture names
    ///
    /// # Returns
    /// Vector of all fixture names
    pub fn list_fixtures(&self) -> Vec<String> {
        self.fixtures.keys().cloned().collect()
    }

    /// Clear all fixtures
    pub fn clear(&mut self) {
        self.fixtures.clear();
    }

    /// Reload common fixtures
    pub fn reload(&mut self) {
        self.clear();
        self.load_common_fixtures();
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
