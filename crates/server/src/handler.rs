//! Webhook event handling for the Release Regent HTTP server.
//!
//! This module bridges the [`github_bot_sdk`] [`WebhookReceiver`] with the
//! Release Regent core event pipeline. It provides:
//!
//! - [`WebhookSecretProvider`] — thin `SecretProvider` wrapper over a pre-loaded
//!   webhook secret string.
//! - [`classify_event`] — classifies a raw GitHub event-type string + JSON payload
//!   into a domain [`EventType`].
//! - [`convert_envelope`] — converts an SDK [`EventEnvelope`] into a domain
//!   [`ProcessingEvent`].
//! - [`ReleaseRegentWebhookHandler`] — implements the SDK's [`WebhookHandler`]
//!   trait; performs allow-list filtering and forwards events on an `mpsc` channel.
//! - [`WebhookEventSource`] — implements the core [`EventSource`] trait by reading
//!   from the same `mpsc` channel; consumed by `run_event_loop` (task 4.0).
//! - [`create_webhook_components`] — convenience factory that creates a matched
//!   handler/source pair sharing a channel.
//!
//! # Architecture
//!
//! ```text
//!  GitHub HTTPS ───► Axum /webhook handler
//!                         └─ WebhookReceiver (SDK)
//!                               ├─ SignatureValidator        (HMAC-SHA256)
//!                               └─ ReleaseRegentWebhookHandler
//!                                       └─ mpsc::Sender<ProcessingEvent>
//!                                                    │
//!                                                    ▼
//!                                       WebhookEventSource
//!                                         └─ mpsc::Receiver<ProcessingEvent>
//!                                                    │
//!                                                    ▼
//!                                           run_event_loop  (task 4.0)
//! ```

use crate::errors::Error;
use async_trait::async_trait;
use chrono::Utc;
use github_bot_sdk::{
    events::EventEnvelope, webhook::WebhookHandler, GitHubAppId, PrivateKey, SecretError,
    SecretProvider,
};
use release_regent_core::{
    traits::event_source::{
        EventSource, EventSourceKind, EventType, ProcessingEvent, RepositoryInfo,
    },
    CoreResult,
};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, warn};

// ─────────────────────────────────────────────────────────────────────────────
// WebhookSecretProvider
// ─────────────────────────────────────────────────────────────────────────────

/// Thin [`SecretProvider`] that wraps a pre-loaded webhook secret string.
///
/// Production deployments load the secret through a secret-management service
/// (e.g., Azure Key Vault) before constructing this struct. The SDK's
/// [`SignatureValidator`](github_bot_sdk::webhook::SignatureValidator) calls
/// [`get_webhook_secret`](SecretProvider::get_webhook_secret) during every
/// request, so the value must already have been retrieved at startup.
///
/// `get_private_key` and `get_app_id` are not required for webhook validation
/// and always return [`SecretError::NotFound`].
pub struct WebhookSecretProvider {
    secret: String,
}

impl WebhookSecretProvider {
    /// Create a new provider wrapping `secret`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let provider = WebhookSecretProvider::new("my_webhook_secret");
    /// ```
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
        }
    }
}

#[async_trait]
impl SecretProvider for WebhookSecretProvider {
    async fn get_webhook_secret(&self) -> Result<String, SecretError> {
        Ok(self.secret.clone())
    }

    async fn get_private_key(&self) -> Result<PrivateKey, SecretError> {
        Err(SecretError::NotFound {
            key: "private_key".to_string(),
        })
    }

    async fn get_app_id(&self) -> Result<GitHubAppId, SecretError> {
        Err(SecretError::NotFound {
            key: "app_id".to_string(),
        })
    }

    fn cache_duration(&self) -> chrono::Duration {
        // The SDK requires a non-zero TTL. In this implementation the secret is
        // pre-loaded at startup — this value does not trigger any actual re-fetch;
        // it only satisfies the contract. Five minutes is the shortest reasonable
        // value for a cached credential.
        chrono::Duration::minutes(5)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Event classification helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Build the full branch head prefix that identifies a release PR.
///
/// The convention is `"{branch_prefix}/v"`, e.g. `"release/v"` for the
/// default configuration. This is the single place in the server crate that
/// encodes the `/v` suffix; `ReleaseOrchestrator` has an equivalent private
/// method in the core crate.
fn release_v_prefix(branch_prefix: &str) -> String {
    format!("{branch_prefix}/v")
}

/// Classify a raw GitHub webhook event into a domain [`EventType`].
///
/// ## Routing table
///
/// | `X-GitHub-Event`              | Conditions                                                      | Result                             |
/// |-------------------------------|----------------------------------------------------------------|------------------------------------|
/// | `pull_request`                | `action=closed`, `merged=true`, non-release branch             | `PullRequestMerged`                |
/// | `pull_request`                | `action=closed`, `merged=true`, `{release_branch_prefix}/v*`  | `ReleasePrMerged`                  |
/// | `pull_request`                | any other action or not merged                                  | `Unknown("pull_request:<action>")` |
/// | `issue_comment`               | `issue.pull_request` field present in payload                   | `PullRequestCommentReceived`       |
/// | `issue_comment`               | no `issue.pull_request` field (plain issue)                     | `Unknown("issue_comment:issue")`   |
/// | `pull_request_review_comment` | always                                                          | `PullRequestCommentReceived`       |
/// | everything else               | always                                                          | `Unknown("<event_type>")`          |
///
/// # Parameters
///
/// - `event_type` — The raw `X-GitHub-Event` string (e.g. `"pull_request"`).
/// - `payload` — The parsed JSON body of the webhook.
/// - `release_branch_prefix` — The configured release branch prefix (e.g. `"release"`);
///   combined with `/v` to form the expected branch head prefix (e.g. `"release/v"`).
pub fn classify_event(
    event_type: &str,
    payload: &serde_json::Value,
    release_branch_prefix: &str,
) -> EventType {
    match event_type {
        "pull_request" => classify_pull_request_event(payload, release_branch_prefix),
        "issue_comment" => classify_issue_comment_event(payload),
        "pull_request_review_comment" => EventType::PullRequestCommentReceived,
        other => EventType::Unknown(other.to_string()),
    }
}

/// Classify an `issue_comment` payload.
///
/// GitHub fires `issue_comment` events for comments on both plain Issues and
/// Pull Requests. Only comments where the `issue.pull_request` field is present
/// are classified as [`EventType::PullRequestCommentReceived`]. Comments on
/// plain issues are classified as `Unknown("issue_comment:issue")` and will be
/// logged and dropped by the event loop.
fn classify_issue_comment_event(payload: &serde_json::Value) -> EventType {
    if payload
        .get("issue")
        .and_then(|i| i.get("pull_request"))
        .is_some()
    {
        EventType::PullRequestCommentReceived
    } else {
        EventType::Unknown("issue_comment:issue".to_string())
    }
}

/// Classify a `pull_request` payload into a specific [`EventType`].
///
/// Non-closed and non-merged events return `Unknown("pull_request:<action>")`
/// so that the action is visible in logs when diagnosing which events are being
/// discarded.
///
/// A merged PR whose head branch starts with `{release_branch_prefix}/v` is
/// classified as [`EventType::ReleasePrMerged`]; all others map to
/// [`EventType::PullRequestMerged`].
///
/// # Panics
///
/// Does not panic. An empty `release_branch_prefix` is treated as a
/// programming error: a `WARN` log is emitted and the event is classified as
/// [`EventType::PullRequestMerged`] rather than silently matching any branch
/// that starts with `"/v"`.
fn classify_pull_request_event(
    payload: &serde_json::Value,
    release_branch_prefix: &str,
) -> EventType {
    let action = payload
        .get("action")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");

    let is_merged = payload
        .pointer("/pull_request/merged")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    if !(action == "closed" && is_merged) {
        return EventType::Unknown(format!("pull_request:{action}"));
    }

    if release_branch_prefix.is_empty() {
        warn!("release_branch_prefix is empty; classifying merged PR as PullRequestMerged to avoid matching any /v* branch");
        return EventType::PullRequestMerged;
    }

    let head_ref = payload
        .pointer("/pull_request/head/ref")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");

    if head_ref.starts_with(release_v_prefix(release_branch_prefix).as_str()) {
        EventType::ReleasePrMerged
    } else {
        EventType::PullRequestMerged
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Envelope → ProcessingEvent conversion
// ─────────────────────────────────────────────────────────────────────────────

/// Convert an SDK [`EventEnvelope`] into a domain [`ProcessingEvent`].
///
/// The `repository.full_name` field (e.g. `"owner/repo"`) is split on `/` to
/// populate [`RepositoryInfo::owner`] and [`RepositoryInfo::name`].
///
/// # Errors
///
/// Returns [`Error::Internal`] when `repository.full_name` does not contain
/// a `/` separator.
// `CoreError` is a large enum; boxing it here would complicate callers.
// This is the established pattern across the codebase.
#[allow(clippy::result_large_err)]
pub fn convert_envelope(
    envelope: &EventEnvelope,
    release_branch_prefix: &str,
) -> Result<ProcessingEvent, Error> {
    let full_name = &envelope.repository.full_name;

    let (owner, name) = full_name.split_once('/').ok_or_else(|| Error::Internal {
        message: format!("invalid repository full_name: {full_name}"),
    })?;

    let repository = RepositoryInfo {
        owner: owner.to_string(),
        name: name.to_string(),
        default_branch: envelope.repository.default_branch.clone(),
    };

    let event_type = classify_event(
        envelope.event_type.as_str(),
        envelope.payload.raw(),
        release_branch_prefix,
    );

    let installation_id = envelope
        .payload
        .raw()
        .get("installation")
        .and_then(|i| i.get("id"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);

    Ok(ProcessingEvent {
        event_id: envelope.event_id.to_string(),
        correlation_id: envelope.correlation_id().to_string(),
        event_type,
        repository,
        payload: envelope.payload.raw().clone(),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// ReleaseRegentWebhookHandler
// ─────────────────────────────────────────────────────────────────────────────

/// SDK [`WebhookHandler`] that filters events by repository allow-list and
/// forwards them as [`ProcessingEvent`]s on an `mpsc` channel.
///
/// The handler is registered with [`WebhookReceiver`](github_bot_sdk::webhook::WebhookReceiver)
/// and invoked after signature validation succeeds. The HTTP response is
/// already sent to GitHub before this method is called (fire-and-forget), so
/// dropping an event here does not cause a GitHub delivery error.
pub struct ReleaseRegentWebhookHandler {
    tx: mpsc::Sender<ProcessingEvent>,
    allowed_repos: Vec<String>,
    release_branch_prefix: String,
}

impl ReleaseRegentWebhookHandler {
    /// Create a new handler.
    ///
    /// # Parameters
    ///
    /// - `tx` — Sender side of the processing channel.
    /// - `allowed_repos` — Repository allow-list.
    ///   - Empty `Vec` → deny all repositories.
    ///   - `vec!["*"]` → allow all repositories.
    ///   - Otherwise → exact `"owner/repo"` match.
    /// - `release_branch_prefix` — The configured release branch prefix (e.g. `"release"`);
    ///   forwarded to [`classify_event`] during envelope conversion.
    pub fn new(
        tx: mpsc::Sender<ProcessingEvent>,
        allowed_repos: Vec<String>,
        release_branch_prefix: String,
    ) -> Self {
        Self {
            tx,
            allowed_repos,
            release_branch_prefix,
        }
    }

    /// Return `true` if `full_name` matches the allow-list policy.
    ///
    /// See [`new`](Self::new) for documentation on the allow-list semantics.
    pub fn is_allowed(&self, full_name: &str) -> bool {
        if self.allowed_repos.is_empty() {
            return false;
        }
        if self.allowed_repos.iter().any(|r| r == "*") {
            return true;
        }
        self.allowed_repos.iter().any(|r| r == full_name)
    }
}

#[async_trait]
impl WebhookHandler for ReleaseRegentWebhookHandler {
    async fn handle_event(
        &self,
        envelope: &EventEnvelope,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let full_name = &envelope.repository.full_name;

        if !self.is_allowed(full_name) {
            warn!(
                repository = %full_name,
                "Repository not in allow-list; dropping event"
            );
            return Ok(());
        }

        let processing_event = match convert_envelope(envelope, &self.release_branch_prefix) {
            Ok(e) => e,
            Err(e) => {
                warn!(
                    error = %e,
                    event_id = %envelope.event_id,
                    "Failed to convert envelope; dropping event"
                );
                return Ok(());
            }
        };

        let event_id = processing_event.event_id.clone();
        let event_type = processing_event.event_type.to_string();

        match self.tx.try_send(processing_event) {
            Ok(()) => {
                debug!(
                    event_id = %event_id,
                    event_type = %event_type,
                    "Forwarded processing event to channel"
                );
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                warn!(event_id = %event_id, "Event channel full; dropping event");
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                warn!(event_id = %event_id, "Event channel closed; dropping event");
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// WebhookEventSource
// ─────────────────────────────────────────────────────────────────────────────

/// [`EventSource`] that reads [`ProcessingEvent`]s from the `mpsc` channel
/// populated by [`ReleaseRegentWebhookHandler`].
///
/// `acknowledge` and `reject` are deliberate no-ops: webhooks are
/// fire-and-forget and GitHub does not support per-event back-pressure.
///
/// # Implementation notes
///
/// The receiver is wrapped in `Arc<Mutex<…>>` solely because the
/// [`EventSource`] trait requires `&self` on `next_event`; mutably borrowing
/// the channel requires interior mutability. In a healthy deployment only one
/// task ever calls `next_event`, so lock contention is zero.
pub struct WebhookEventSource {
    rx: Arc<Mutex<mpsc::Receiver<ProcessingEvent>>>,
}

impl WebhookEventSource {
    /// Wrap `rx` in the event source.
    pub fn new(rx: mpsc::Receiver<ProcessingEvent>) -> Self {
        Self {
            rx: Arc::new(Mutex::new(rx)),
        }
    }
}

#[async_trait]
impl EventSource for WebhookEventSource {
    /// Poll for the next available event.
    ///
    /// Uses [`mpsc::Receiver::try_recv`] (non-blocking) so that this call
    /// returns immediately when the channel is empty, consistent with the
    /// [`EventSource`] trait contract. The event loop consuming this source
    /// **must** yield between empty polls (e.g. via `tokio::time::sleep`) to
    /// avoid busy-spinning.
    async fn next_event(&self) -> CoreResult<Option<ProcessingEvent>> {
        let mut rx = self.rx.lock().await;
        match rx.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => {
                tracing::warn!(
                    "WebhookEventSource channel disconnected; all senders have been dropped"
                );
                Ok(None)
            }
        }
    }

    async fn acknowledge(&self, _event_id: &str) -> CoreResult<()> {
        Ok(())
    }

    async fn reject(&self, _event_id: &str, _permanent: bool) -> CoreResult<()> {
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Factory
// ─────────────────────────────────────────────────────────────────────────────

/// Create a matched [`ReleaseRegentWebhookHandler`] / [`WebhookEventSource`] pair.
///
/// Both share a bounded `mpsc` channel. Events are dropped (with a `WARN`
/// tracing event) when the channel reaches `channel_capacity`.
///
/// # Parameters
///
/// - `allowed_repos` — Repository allow-list; see [`ReleaseRegentWebhookHandler::new`].
/// - `channel_capacity` — Bounded channel depth.
/// - `release_branch_prefix` — The configured release branch prefix (e.g. `"release"`);
///   forwarded to [`classify_event`] to distinguish release PRs from regular PRs.
pub fn create_webhook_components(
    allowed_repos: Vec<String>,
    channel_capacity: usize,
    release_branch_prefix: String,
) -> (ReleaseRegentWebhookHandler, WebhookEventSource) {
    let (tx, rx) = mpsc::channel(channel_capacity);
    (
        ReleaseRegentWebhookHandler::new(tx, allowed_repos, release_branch_prefix),
        WebhookEventSource::new(rx),
    )
}

#[cfg(test)]
#[path = "handler_tests.rs"]
mod tests;
