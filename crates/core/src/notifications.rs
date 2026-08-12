//! Notification dispatch for processing errors and release events.
//!
//! [`NotificationDispatcher`] is the single entry point event-processing code
//! consults to decide whether, and how, to notify humans about a processing
//! outcome. It reads [`crate::config::NotificationConfig`] to select one of
//! three delivery strategies — GitHub Issue, Webhook, or Slack — and routes
//! the [`NotificationEvent`] accordingly. When `notifications.enabled` is
//! `false`, or the configured strategy is
//! [`crate::config::NotificationStrategy::None`], dispatch is a no-op: no
//! port method is called.
//!
//! # Idempotency (GitHub Issue strategy)
//!
//! The GitHub Issue strategy embeds a deterministic hidden marker,
//! `<!-- release-regent:notification:{sha256_hex} -->`, in the issue body.
//! The hash is computed over `(owner, repo, kind, summary)` so that the same
//! recurring error produces the same marker. Before creating a new issue, the
//! dispatcher lists open issues carrying the configured labels and searches
//! for one whose body already contains this marker; if found, a comment is
//! added to that existing issue instead of creating a duplicate.
//!
//! # Architecture
//!
//! Per ADR-001 (hexagonal architecture), this module depends only on the
//! [`crate::traits::github_operations::GitHubOperations`] and
//! [`crate::traits::notification_sender::NotificationSender`] ports; it never
//! imports an HTTP client directly. `core` remains free of any concrete
//! delivery mechanism.

use crate::config::NotificationConfig;
use crate::traits::{github_operations::GitHubOperations, notification_sender::NotificationSender};
use crate::CoreResult;

/// The kind of event being reported to [`NotificationDispatcher::dispatch`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationEventKind {
    /// An unrecoverable error occurred while processing a webhook event.
    ProcessingError,
    /// A release was successfully created.
    ReleaseCreated,
}

/// A single notification-worthy occurrence to be routed by
/// [`NotificationDispatcher::dispatch`].
#[derive(Debug, Clone)]
pub struct NotificationEvent {
    /// The kind of event being reported.
    pub kind: NotificationEventKind,
    /// Repository owner.
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Correlation ID used to trace this event across logs and notifications.
    pub correlation_id: String,
    /// Short, one-line human-readable summary (e.g. issue title, Slack header line).
    pub summary: String,
    /// Full human-readable details (e.g. issue body text, Slack message body).
    pub details: String,
}

/// Routes [`NotificationEvent`]s to the delivery strategy selected by
/// [`NotificationConfig::strategy`].
///
/// Borrows both ports for the duration of a single [`Self::dispatch`] call
/// rather than owning them, so a dispatcher can be constructed cheaply from
/// whichever installation-scoped `GitHubOperations`/`NotificationSender`
/// instance is already in hand at the call site.
pub struct NotificationDispatcher<'a, G: GitHubOperations, N: NotificationSender> {
    github_operations: &'a G,
    notification_sender: &'a N,
}

impl<'a, G: GitHubOperations, N: NotificationSender> NotificationDispatcher<'a, G, N> {
    /// Construct a new dispatcher borrowing the given ports.
    pub fn new(github_operations: &'a G, notification_sender: &'a N) -> Self {
        Self {
            github_operations,
            notification_sender,
        }
    }

    /// Route `event` to the delivery strategy selected by `config.strategy`.
    ///
    /// Returns `Ok(())` immediately without contacting any port when
    /// `config.enabled` is `false`, or when `config.strategy` is
    /// [`crate::config::NotificationStrategy::None`].
    ///
    /// # Errors
    /// - `CoreError::Config` — the strategy-specific configuration section
    ///   (`config.webhook`, `config.slack`) required by `config.strategy` is
    ///   absent.
    /// - Any error propagated unchanged from the underlying
    ///   [`GitHubOperations`] or [`NotificationSender`] port call.
    ///
    /// # Panics
    /// This is a RED-phase skeleton: the body is intentionally unimplemented
    /// and always panics. A Coder implements the real dispatch logic in the
    /// GREEN phase.
    pub async fn dispatch(
        &self,
        config: &NotificationConfig,
        event: &NotificationEvent,
    ) -> CoreResult<()> {
        // Reference both ports so the fields are not flagged as dead code
        // while this method is unimplemented.
        let _ = self.github_operations;
        let _ = self.notification_sender;
        let _ = (config, event);
        todo!("RED phase: NotificationDispatcher::dispatch is not yet implemented")
    }
}

#[cfg(test)]
#[path = "notifications_tests.rs"]
mod tests;
