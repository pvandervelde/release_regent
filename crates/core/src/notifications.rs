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

use crate::config::{NotificationConfig, NotificationStrategy};
use crate::traits::{
    github_operations::{CreateIssueParams, GitHubOperations, IssueState, ListIssuesOptions},
    notification_sender::NotificationSender,
};
use crate::{CoreError, CoreResult};
use sha2::{Digest, Sha256};


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
    pub async fn dispatch(
        &self,
        config: &NotificationConfig,
        event: &NotificationEvent,
    ) -> CoreResult<()> {
        if !config.enabled {
            return Ok(());
        }

        match config.strategy {
            NotificationStrategy::None => Ok(()),
            NotificationStrategy::GitHubIssue => self.dispatch_github_issue(config, event).await,
            NotificationStrategy::Webhook => self.dispatch_webhook(config, event).await,
            NotificationStrategy::Slack => self.dispatch_slack(config, event).await,
        }
    }

    /// Handle [`NotificationStrategy::GitHubIssue`] dispatch.
    ///
    /// Computes the idempotency marker, lists open issues carrying the
    /// configured labels, and either comments on a matching existing issue or
    /// creates a new one.
    async fn dispatch_github_issue(
        &self,
        config: &NotificationConfig,
        event: &NotificationEvent,
    ) -> CoreResult<()> {
        let github_issue_config = config.github_issue.clone().unwrap_or_default();
        let marker = notification_marker(&event.owner, &event.repo, event.kind, &event.summary);

        let existing = self
            .github_operations
            .list_issues(
                &event.owner,
                &event.repo,
                ListIssuesOptions {
                    state: IssueState::Open,
                    labels: github_issue_config.labels.clone(),
                },
            )
            .await?;

        let matching_issue = existing
            .into_iter()
            .find(|issue| issue.body.as_deref().is_some_and(|body| body.contains(&marker)));

        if let Some(issue) = matching_issue {
            self.github_operations
                .create_issue_comment(&event.owner, &event.repo, issue.number, &event.details)
                .await?;
        } else {
            self.github_operations
                .create_issue(
                    &event.owner,
                    &event.repo,
                    CreateIssueParams {
                        title: event.summary.clone(),
                        body: Some(format!("{marker}\n\n{}", event.details)),
                        labels: github_issue_config.labels,
                        assignees: github_issue_config.assignees,
                    },
                )
                .await?;
        }

        Ok(())
    }

    /// Handle [`NotificationStrategy::Webhook`] dispatch.
    async fn dispatch_webhook(
        &self,
        config: &NotificationConfig,
        event: &NotificationEvent,
    ) -> CoreResult<()> {
        let webhook = config
            .webhook
            .as_ref()
            .ok_or_else(|| CoreError::config(
                "notifications.webhook is required when notifications.strategy is \"webhook\"",
            ))?;

        let payload = serde_json::json!({
            "event": event_kind_as_str(event.kind),
            "owner": event.owner,
            "repo": event.repo,
            "correlation_id": event.correlation_id,
            "summary": event.summary,
            "details": event.details,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        self.notification_sender
            .send_webhook(&webhook.url, &webhook.headers, &payload)
            .await
    }

    /// Handle [`NotificationStrategy::Slack`] dispatch.
    async fn dispatch_slack(
        &self,
        config: &NotificationConfig,
        event: &NotificationEvent,
    ) -> CoreResult<()> {
        let slack = config
            .slack
            .as_ref()
            .ok_or_else(|| CoreError::config(
                "notifications.slack is required when notifications.strategy is \"slack\"",
            ))?;

        let text = format!("*{}*\n{}", event.summary, event.details);

        self.notification_sender
            .send_slack(&slack.webhook_url, slack.channel.as_deref(), &text)
            .await
    }
}

/// Compute the deterministic idempotency marker embedded in GitHub issue
/// bodies created by the [`NotificationStrategy::GitHubIssue`] strategy.
///
/// The hash is computed over `(owner, repo, kind, summary)` so that the same
/// recurring error produces the same marker across dispatcher invocations.
fn notification_marker(
    owner: &str,
    repo: &str,
    kind: NotificationEventKind,
    summary: &str,
) -> String {
    let input = format!("{owner}/{repo}/{kind:?}/{summary}");
    let digest = Sha256::digest(input.as_bytes());
    let hex = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    format!("<!-- release-regent:notification:{hex} -->")
}

/// Render a [`NotificationEventKind`] as the lowercase snake_case string used
/// in the `event` field of webhook payloads.
fn event_kind_as_str(kind: NotificationEventKind) -> &'static str {
    match kind {
        NotificationEventKind::ProcessingError => "processing_error",
        NotificationEventKind::ReleaseCreated => "release_created",
    }
}

#[cfg(test)]
#[path = "notifications_tests.rs"]
mod tests;
