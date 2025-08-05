//! Mock webhook validator for testing
//!
//! This module provides mock implementations of the WebhookValidator trait
//! for comprehensive testing scenarios.

use async_trait::async_trait;
use release_regent_core::{traits::WebhookValidator, CoreResult};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Mock webhook validator for testing
///
/// This mock implementation allows controlling validation behavior
/// and capturing method calls for test verification.
#[derive(Debug, Clone)]
pub struct MockWebhookValidator {
    /// Shared state for the mock
    state: Arc<Mutex<MockWebhookValidatorState>>,
}

#[derive(Debug, Default)]
struct MockWebhookValidatorState {
    /// Method call tracking
    call_history: Vec<String>,
    /// Expected results for verify_signature calls
    signature_results: HashMap<String, bool>,
    /// Expected results for validate_payload calls
    payload_results: HashMap<String, bool>,
    /// Whether to simulate failures
    should_fail_signature: bool,
    should_fail_payload: bool,
}

impl MockWebhookValidator {
    /// Create a new mock webhook validator
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(MockWebhookValidatorState::default())),
        }
    }

    /// Set expected result for signature verification
    pub fn expect_signature_result(&self, signature: &str, result: bool) {
        let mut state = self.state.lock().unwrap();
        state
            .signature_results
            .insert(signature.to_string(), result);
    }

    /// Set expected result for payload validation
    pub fn expect_payload_result(&self, event_type: &str, result: bool) {
        let mut state = self.state.lock().unwrap();
        state.payload_results.insert(event_type.to_string(), result);
    }

    /// Make signature verification fail with an error
    pub fn fail_signature_verification(&self, should_fail: bool) {
        let mut state = self.state.lock().unwrap();
        state.should_fail_signature = should_fail;
    }

    /// Make payload validation fail with an error
    pub fn fail_payload_validation(&self, should_fail: bool) {
        let mut state = self.state.lock().unwrap();
        state.should_fail_payload = should_fail;
    }

    /// Get call history for verification
    pub fn get_call_history(&self) -> Vec<String> {
        let state = self.state.lock().unwrap();
        state.call_history.clone()
    }

    /// Clear call history
    pub fn clear_call_history(&self) {
        let mut state = self.state.lock().unwrap();
        state.call_history.clear();
    }

    /// Verify that a specific method was called
    pub fn verify_method_called(&self, method: &str) -> bool {
        let state = self.state.lock().unwrap();
        state.call_history.iter().any(|call| call.contains(method))
    }

    /// Get number of times a method was called
    pub fn get_method_call_count(&self, method: &str) -> usize {
        let state = self.state.lock().unwrap();
        state
            .call_history
            .iter()
            .filter(|call| call.contains(method))
            .count()
    }
}

impl Default for MockWebhookValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl WebhookValidator for MockWebhookValidator {
    async fn verify_signature(
        &self,
        payload: &[u8],
        signature: &str,
        secret: &str,
    ) -> CoreResult<bool> {
        let method = "verify_signature";
        {
            let mut state = self.state.lock().unwrap();
            state.call_history.push(format!(
                "{}(payload_len={}, signature={}, secret={})",
                method,
                payload.len(),
                signature,
                secret
            ));

            if state.should_fail_signature {
                return Err(release_regent_core::CoreError::webhook(
                    "mock_error",
                    "Mock signature verification failure",
                ));
            }
        }

        // Check if we have a specific result configured for this signature
        let state = self.state.lock().unwrap();
        if let Some(&result) = state.signature_results.get(signature) {
            Ok(result)
        } else {
            // Default: valid signature
            Ok(true)
        }
    }

    async fn validate_payload(
        &self,
        payload: &serde_json::Value,
        event_type: &str,
    ) -> CoreResult<bool> {
        let method = "validate_payload";
        {
            let mut state = self.state.lock().unwrap();
            state
                .call_history
                .push(format!("{}(event_type={})", method, event_type));

            if state.should_fail_payload {
                return Err(release_regent_core::CoreError::webhook(
                    "mock_error",
                    "Mock payload validation failure",
                ));
            }
        }

        // Check if we have a specific result configured for this event type
        let state = self.state.lock().unwrap();
        if let Some(&result) = state.payload_results.get(event_type) {
            Ok(result)
        } else {
            // Default: valid payload
            Ok(true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_webhook_validator_signature_verification() {
        let validator = MockWebhookValidator::new();

        // Test default behavior (should pass)
        let result = validator
            .verify_signature(b"test payload", "sha256=hash", "secret")
            .await
            .unwrap();
        assert!(result);

        // Test configured result
        validator.expect_signature_result("sha256=invalid", false);
        let result = validator
            .verify_signature(b"test payload", "sha256=invalid", "secret")
            .await
            .unwrap();
        assert!(!result);

        // Verify method was called
        assert!(validator.verify_method_called("verify_signature"));
        assert_eq!(validator.get_method_call_count("verify_signature"), 2);
    }

    #[tokio::test]
    async fn test_mock_webhook_validator_payload_validation() {
        let validator = MockWebhookValidator::new();
        let payload = serde_json::json!({"action": "opened"});

        // Test default behavior (should pass)
        let result = validator
            .validate_payload(&payload, "pull_request")
            .await
            .unwrap();
        assert!(result);

        // Test configured result
        validator.expect_payload_result("invalid_event", false);
        let result = validator
            .validate_payload(&payload, "invalid_event")
            .await
            .unwrap();
        assert!(!result);

        // Verify method was called
        assert!(validator.verify_method_called("validate_payload"));
        assert_eq!(validator.get_method_call_count("validate_payload"), 2);
    }

    #[tokio::test]
    async fn test_mock_webhook_validator_failure_simulation() {
        let validator = MockWebhookValidator::new();

        // Test signature verification failure
        validator.fail_signature_verification(true);
        let result = validator.verify_signature(b"test", "sig", "secret").await;
        assert!(result.is_err());

        // Test payload validation failure
        validator.fail_payload_validation(true);
        let payload = serde_json::json!({});
        let result = validator.validate_payload(&payload, "test").await;
        assert!(result.is_err());
    }
}
