//! Mock implementations of all core traits
//!
//! This module provides comprehensive mock implementations of all traits defined
//! in the core crate. These mocks support deterministic testing, error simulation,
//! and behavioral assertion verification.
//!
//! # Mock Behavior Configuration
//!
//! All mocks support configuration of:
//! - Expected return values and behaviors
//! - Error conditions and failure scenarios
//! - Response timing and latency simulation
//! - Call tracking and verification
//! - State management for complex scenarios
//!
//! # Thread Safety
//!
//! All mock implementations are thread-safe and can be used in:
//! - Concurrent test execution
//! - Multi-threaded application testing
//! - Shared test fixtures
//! - Performance testing scenarios
//!
//! # Error Simulation
//!
//! Mocks can simulate all error conditions that may occur in production:
//! - Network failures and timeouts
//! - Authentication and authorization errors
//! - Rate limiting and quota exceeded
//! - Invalid input and validation errors
//! - Service unavailable and maintenance modes

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod configuration_provider;
pub mod github_operations;
pub mod version_calculator;

pub use configuration_provider::MockConfigurationProvider;
pub use github_operations::MockGitHubOperations;
pub use version_calculator::MockVersionCalculator;

/// Mock behavior configuration for all trait implementations
#[derive(Debug, Clone)]
pub struct MockConfig {
    /// Whether to enable deterministic behavior (same inputs = same outputs)
    pub deterministic: bool,
    /// Default response latency in milliseconds
    pub response_latency_ms: u64,
    /// Whether to track all method calls for verification
    pub track_calls: bool,
    /// Maximum number of calls before simulating quota exceeded
    pub max_calls: Option<u64>,
    /// Whether to simulate intermittent failures
    pub simulate_failures: bool,
    /// Failure rate (0.0 = no failures, 1.0 = always fail)
    pub failure_rate: f64,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            deterministic: true,
            response_latency_ms: 0,
            track_calls: true,
            max_calls: None,
            simulate_failures: false,
            failure_rate: 0.0,
        }
    }
}

/// Call tracking information for mock verification
#[derive(Debug, Clone)]
pub struct CallInfo {
    /// Method name that was called
    pub method: String,
    /// Parameters passed to the method (serialized as JSON)
    pub parameters: String,
    /// Timestamp when the call was made
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Result of the call (success/failure)
    pub result: CallResult,
}

/// Result of a mock method call
#[derive(Debug, Clone)]
pub enum CallResult {
    /// Call completed successfully
    Success,
    /// Call failed with an error
    Error(String),
    /// Call was cancelled or timed out
    Cancelled,
}

/// Shared state for tracking mock behavior and calls
#[derive(Debug, Default)]
pub struct MockState {
    /// Configuration for mock behavior
    config: MockConfig,
    /// History of all method calls
    call_history: Vec<CallInfo>,
    /// Current call count
    call_count: u64,
    /// Custom state data for specific test scenarios
    custom_data: HashMap<String, serde_json::Value>,
}

impl MockState {
    /// Create new mock state with default configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Create new mock state with custom configuration
    pub fn with_config(config: MockConfig) -> Self {
        Self {
            config,
            ..Default::default()
        }
    }

    /// Record a method call for tracking and verification
    pub fn record_call(&mut self, method: &str, parameters: &str, result: CallResult) {
        if self.config.track_calls {
            self.call_history.push(CallInfo {
                method: method.to_string(),
                parameters: parameters.to_string(),
                timestamp: chrono::Utc::now(),
                result,
            });
        }
        self.call_count += 1;
    }

    /// Get the history of all recorded calls
    pub fn call_history(&self) -> &[CallInfo] {
        &self.call_history
    }

    /// Get the total number of calls made
    pub fn call_count(&self) -> u64 {
        self.call_count
    }

    /// Check if quota limit has been exceeded
    pub fn is_quota_exceeded(&self) -> bool {
        if let Some(max_calls) = self.config.max_calls {
            self.call_count >= max_calls
        } else {
            false
        }
    }

    /// Determine if this call should simulate a failure
    pub fn should_simulate_failure(&self) -> bool {
        if !self.config.simulate_failures {
            return false;
        }

        use rand::Rng;
        let mut rng = rand::thread_rng();
        rng.gen::<f64>() < self.config.failure_rate
    }

    /// Add latency simulation if configured
    pub async fn simulate_latency(&self) {
        if self.config.response_latency_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(
                self.config.response_latency_ms,
            ))
            .await;
        }
    }

    /// Set custom data for test scenarios
    pub fn set_custom_data(&mut self, key: &str, value: serde_json::Value) {
        self.custom_data.insert(key.to_string(), value);
    }

    /// Get custom data for test scenarios
    pub fn get_custom_data(&self, key: &str) -> Option<&serde_json::Value> {
        self.custom_data.get(key)
    }
}

/// Thread-safe wrapper for mock state
pub type SharedMockState = Arc<RwLock<MockState>>;
