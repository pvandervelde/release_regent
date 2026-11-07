//! Production HTTP-based [`NotificationSender`] adapter.
//!
//! Implements [`NotificationSender`] using a single, reused `reqwest::Client`
//! with a bounded request timeout, delivering webhook and Slack
//! incoming-webhook notifications on behalf of the notification dispatcher
//! (`release_regent_core::notifications`). Keeping this adapter outside
//! `crates/core` preserves ADR-001's hexagonal architecture boundary: `core`
//! depends only on the [`NotificationSender`] trait, never on `reqwest`
//! directly.

use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use release_regent_core::{traits::notification_sender::NotificationSender, CoreError, CoreResult};
use tracing::instrument;

/// Timeout applied to every webhook/Slack HTTP request.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Production [`NotificationSender`] implementation that delivers webhook and
/// Slack notifications over HTTP using `reqwest`.
///
/// The underlying [`reqwest::Client`] is constructed once (bound to a
/// 10-second [`REQUEST_TIMEOUT`]) and reused for every request.
#[derive(Debug, Clone)]
pub struct HttpNotificationSender {
    client: reqwest::Client,
}

impl HttpNotificationSender {
    /// Construct a new sender with a fresh `reqwest::Client` bound to a
    /// 10-second request timeout.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Network`] if the underlying HTTP client cannot be
    /// built (e.g. TLS backend initialisation failure).
    #[allow(clippy::result_large_err)]
    pub fn new() -> CoreResult<Self> {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|e| CoreError::network(format!("failed to build HTTP client: {e}")))?;
        Ok(Self { client })
    }

    /// Deliver `payload` to `url` via POST, applying `headers` and mapping the
    /// response/transport outcome to a `CoreResult`.
    ///
    /// Shared by [`send_webhook`](NotificationSender::send_webhook) and
    /// [`send_slack`](NotificationSender::send_slack); `stage` selects which
    /// `CoreError::Webhook` stage label (`"webhook"` or `"slack"`) is used on
    /// a non-2xx response.
    async fn post_json(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        payload: &serde_json::Value,
        stage: &str,
    ) -> CoreResult<()> {
        let mut request = self
            .client
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, "application/json");
        for (name, value) in headers {
            request = request.header(name, value);
        }

        let response = request
            .json(payload)
            .send()
            .await
            .map_err(|e| CoreError::network(format!("{stage} request failed: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            return Err(CoreError::webhook(
                stage,
                format!("{stage} responded with status {status}"),
            ));
        }

        Ok(())
    }
}

#[async_trait]
impl NotificationSender for HttpNotificationSender {
    #[instrument(skip(self, headers, payload))]
    async fn send_webhook(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        payload: &serde_json::Value,
    ) -> CoreResult<()> {
        self.post_json(url, headers, payload, "webhook").await
    }

    #[instrument(skip(self, text))]
    async fn send_slack(
        &self,
        webhook_url: &str,
        channel: Option<&str>,
        text: &str,
    ) -> CoreResult<()> {
        let mut payload = serde_json::json!({ "text": text });
        if let Some(channel) = channel {
            payload["channel"] = serde_json::Value::String(channel.to_string());
        }

        self.post_json(webhook_url, &HashMap::new(), &payload, "slack")
            .await
    }
}

#[cfg(test)]
#[path = "notification_sender_tests.rs"]
mod notification_sender_tests;

