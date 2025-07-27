//! Webhook builder for creating test webhook payloads

use crate::builders::TestDataBuilder;
use serde_json::Value;

/// Builder for creating test webhook payloads
#[derive(Debug, Clone)]
pub struct WebhookBuilder {
    event_type: String,
    action: Option<String>,
    repository_name: String,
    repository_owner: String,
}

impl WebhookBuilder {
    /// Create a new webhook builder with defaults
    pub fn new() -> Self {
        Self {
            event_type: "push".to_string(),
            action: None,
            repository_name: "test-repo".to_string(),
            repository_owner: "test-owner".to_string(),
        }
    }

    /// Set event type
    pub fn with_event_type(mut self, event_type: &str) -> Self {
        self.event_type = event_type.to_string();
        self
    }

    /// Set action (for some event types)
    pub fn with_action(mut self, action: &str) -> Self {
        self.action = Some(action.to_string());
        self
    }

    /// Set repository
    pub fn with_repository(mut self, owner: &str, name: &str) -> Self {
        self.repository_owner = owner.to_string();
        self.repository_name = name.to_string();
        self
    }
}

impl Default for WebhookBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TestDataBuilder<Value> for WebhookBuilder {
    fn build(self) -> Value {
        // TODO: implement - placeholder for compilation
        // This should create a proper webhook payload
        serde_json::json!({
            "event": self.event_type,
            "action": self.action,
            "repository": {
                "name": self.repository_name,
                "owner": {
                    "login": self.repository_owner
                }
            }
        })
    }

    fn reset(self) -> Self {
        Self::new()
    }
}
