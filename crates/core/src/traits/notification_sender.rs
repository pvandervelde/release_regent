//! Notification delivery port
//!
//! Defines the [`NotificationSender`] trait used by the notification
//! dispatcher (`crate::notifications`) to deliver webhook- and Slack-based
//! notifications through an externally injected adapter, keeping `core` free
//! of direct HTTP client dependencies per ADR-001 (hexagonal architecture).
//!
//! Adapters implementing this trait live outside `core` (e.g. in a future
//! `notification_client` crate or in `server`) and perform the actual HTTP
//! delivery. `core` depends only on this trait.

use crate::CoreResult;
use async_trait::async_trait;
use std::collections::HashMap;

/// Delivery port for webhook- and Slack-based notifications.
///
/// Implementations perform the actual HTTP delivery; `core` never imports an
/// HTTP client directly (ADR-001).
///
/// # Error Handling
///
/// Implementations must map:
/// - Connection/timeout failures to [`crate::CoreError::Network`]
/// - Non-2xx HTTP responses to [`crate::CoreError::Webhook`], with `stage`
///   set to `"webhook"` or `"slack"` respectively, and the response status
///   code folded into the `message` field.
#[async_trait]
pub trait NotificationSender: Send + Sync {
    /// Deliver a generic JSON payload to a webhook endpoint.
    ///
    /// # Parameters
    /// - `url`: Destination webhook URL
    /// - `headers`: Additional HTTP headers to include on the request
    /// - `payload`: JSON body to POST
    ///
    /// # Errors
    /// - `CoreError::Network` - connection or timeout failure
    /// - `CoreError::Webhook` - non-2xx response, `stage == "webhook"`
    async fn send_webhook(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        payload: &serde_json::Value,
    ) -> CoreResult<()>;

    /// Deliver a message to a Slack incoming webhook.
    ///
    /// # Parameters
    /// - `webhook_url`: Slack incoming webhook URL
    /// - `channel`: Optional channel override (e.g. `"#releases"`)
    /// - `text`: Pre-formatted Slack message text
    ///
    /// # Errors
    /// - `CoreError::Network` - connection or timeout failure
    /// - `CoreError::Webhook` - non-2xx response, `stage == "slack"`
    async fn send_slack(&self, webhook_url: &str, channel: Option<&str>, text: &str)
        -> CoreResult<()>;
}
