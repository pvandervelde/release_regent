//! Mock implementation of `NotificationSender` trait
//!
//! Provides a stateful mock implementation of
//! [`release_regent_core::traits::notification_sender::NotificationSender`]
//! that records every `send_webhook` / `send_slack` call and supports
//! configurable failure simulation, without performing any real HTTP
//! delivery.
//!
//! This mock is intended for integration tests (e.g. under
//! `crates/core/tests/`) and consumers outside the `release-regent-core`
//! crate. It cannot be used directly from `#[path]`-included unit-test files
//! within `release-regent-core` itself due to cross-crate type identity
//! constraints between the unittest binary and the library artifact — see
//! `crates/core/src/notifications_tests.rs` for the local inline test double
//! used there instead.

use async_trait::async_trait;
use release_regent_core::traits::notification_sender::NotificationSender;
use release_regent_core::{CoreError, CoreResult};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

type SendWebhookCalls = Arc<RwLock<Vec<(String, HashMap<String, String>, serde_json::Value)>>>;
type SendSlackCalls = Arc<RwLock<Vec<(String, Option<String>, String)>>>;

/// Mock implementation of `NotificationSender`.
///
/// # Example Usage
///
/// ```rust
/// use release_regent_testing::mocks::notification_sender::MockNotificationSender;
///
/// let mock = MockNotificationSender::new().with_send_webhook_error("simulated failure");
/// ```
#[derive(Debug)]
pub struct MockNotificationSender {
    /// Recorded `send_webhook` calls: `(url, headers, payload)`.
    send_webhook_calls: SendWebhookCalls,
    /// Recorded `send_slack` calls: `(webhook_url, channel, text)`.
    send_slack_calls: SendSlackCalls,
    /// When `Some`, `send_webhook` returns this error message instead of succeeding.
    webhook_error: Option<String>,
    /// When `Some`, `send_slack` returns this error message instead of succeeding.
    slack_error: Option<String>,
}

impl MockNotificationSender {
    /// Create a new mock with no recorded calls and no failure simulation.
    #[must_use]
    pub fn new() -> Self {
        Self {
            send_webhook_calls: Arc::new(RwLock::new(Vec::new())),
            send_slack_calls: Arc::new(RwLock::new(Vec::new())),
            webhook_error: None,
            slack_error: None,
        }
    }

    /// Configure `send_webhook` to return a `CoreError::Webhook` with the
    /// given message instead of succeeding.
    #[must_use]
    pub fn with_send_webhook_error(mut self, message: impl Into<String>) -> Self {
        self.webhook_error = Some(message.into());
        self
    }

    /// Configure `send_slack` to return a `CoreError::Webhook` with the
    /// given message instead of succeeding.
    #[must_use]
    pub fn with_send_slack_error(mut self, message: impl Into<String>) -> Self {
        self.slack_error = Some(message.into());
        self
    }

    /// Return all recorded `send_webhook` calls: `(url, headers, payload)`.
    pub async fn send_webhook_calls(
        &self,
    ) -> Vec<(String, HashMap<String, String>, serde_json::Value)> {
        self.send_webhook_calls.read().await.clone()
    }

    /// Return all recorded `send_slack` calls: `(webhook_url, channel, text)`.
    pub async fn send_slack_calls(&self) -> Vec<(String, Option<String>, String)> {
        self.send_slack_calls.read().await.clone()
    }
}

impl Default for MockNotificationSender {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for MockNotificationSender {
    /// Clone by sharing all `Arc`-wrapped call recorders so that the clone
    /// and the original observe the same calls, matching
    /// `MockGitHubOperations`'s clone semantics.
    fn clone(&self) -> Self {
        Self {
            send_webhook_calls: Arc::clone(&self.send_webhook_calls),
            send_slack_calls: Arc::clone(&self.send_slack_calls),
            webhook_error: self.webhook_error.clone(),
            slack_error: self.slack_error.clone(),
        }
    }
}

#[async_trait]
impl NotificationSender for MockNotificationSender {
    async fn send_webhook(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        payload: &serde_json::Value,
    ) -> CoreResult<()> {
        if let Some(message) = &self.webhook_error {
            return Err(CoreError::webhook("webhook", message.clone()));
        }

        self.send_webhook_calls
            .write()
            .await
            .push((url.to_string(), headers.clone(), payload.clone()));
        Ok(())
    }

    async fn send_slack(
        &self,
        webhook_url: &str,
        channel: Option<&str>,
        text: &str,
    ) -> CoreResult<()> {
        if let Some(message) = &self.slack_error {
            return Err(CoreError::webhook("slack", message.clone()));
        }

        self.send_slack_calls.write().await.push((
            webhook_url.to_string(),
            channel.map(str::to_string),
            text.to_string(),
        ));
        Ok(())
    }
}
