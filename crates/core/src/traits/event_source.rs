//! Event source trait and associated types
//!
//! This module defines the fundamental abstraction for event delivery into the
//! Release Regent processing pipeline. The [`EventSource`] trait decouples where
//! events come from (HTTP webhook, message queue, test fixture) from how they are
//! processed, enabling clean hexagonal architecture and comprehensive testing.
//!
//! # Architecture
//!
//! ```text
//! [GitHub Webhook]  ──►  WebhookEventSource ──►  EventSource ──►  run_event_loop
//! [Message Queue]   ──►  QueueEventSource   ──►  EventSource ──►  run_event_loop
//! [Test Fixture]    ──►  MockEventSource    ──►  EventSource ──►  run_event_loop
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use release_regent_core::traits::event_source::{EventSource, ProcessingEvent};
//!
//! async fn run<S: EventSource>(source: &S) {
//!     loop {
//!         match source.next_event().await {
//!             Ok(Some(event)) => {
//!                 // process event
//!                 source.acknowledge(&event.event_id).await.ok();
//!             }
//!             Ok(None) => tokio::time::sleep(std::time::Duration::from_millis(100)).await,
//!             Err(e) => tracing::error!(error = %e, "event source error"),
//!         }
//!     }
//! }
//! ```

use crate::CoreResult;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// RepositoryInfo
// ─────────────────────────────────────────────────────────────────────────────

/// Identity information for a GitHub repository carried inside a [`ProcessingEvent`].
///
/// This type is the canonical repository representation within the event pipeline.
/// It is intentionally minimal — it carries only what is needed for routing and
/// logging, not the full GitHub API `Repository` model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryInfo {
    /// GitHub organisation or user that owns the repository (e.g. `"octocat"`).
    pub owner: String,
    /// Repository name without the owner prefix (e.g. `"hello-world"`).
    pub name: String,
    /// The repository's configured default branch (e.g. `"main"`).
    pub default_branch: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// EventSourceKind
// ─────────────────────────────────────────────────────────────────────────────

/// Identifies the delivery mechanism that produced a [`ProcessingEvent`].
///
/// Used for observability (tracing fields) and for routing decisions where the
/// source kind is relevant (e.g. queue-based events may support dead-lettering
/// while webhook events silently discard rejects).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventSourceKind {
    /// Event was received directly from a GitHub webhook HTTP call.
    Webhook,
    /// Event was received from a message queue.
    Queue {
        /// Human-readable provider name, e.g. `"azure_service_bus"` or `"aws_sqs"`.
        provider: String,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// EventType
// ─────────────────────────────────────────────────────────────────────────────

/// Semantic classification of a GitHub event as it enters the processing pipeline.
///
/// The pipeline dispatcher uses this to route events to the correct handler:
/// - [`EventType::PullRequestMerged`]        → Release Orchestrator
/// - [`EventType::ReleasePrMerged`]          → Release Automator
/// - [`EventType::PullRequestCommentReceived`] → Comment command processor
/// - [`EventType::Unknown`]                  → logged and dropped
///
/// # Parsing from strings
///
/// Use [`EventType::from`] (or `.into()`) to parse the raw event type string
/// forwarded by the webhook or queue message:
///
/// ```
/// use release_regent_core::traits::event_source::EventType;
///
/// let et: EventType = "pull_request_merged".into();
/// assert_eq!(et, EventType::PullRequestMerged);
///
/// let unknown: EventType = "some_other_event".into();
/// assert!(matches!(unknown, EventType::Unknown(_)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// A regular (non-release) pull request was merged.
    ///
    /// Triggers the Release Orchestrator to calculate the next version and
    /// create or update a release PR.
    PullRequestMerged,

    /// A release pull request (branch `release/vX.Y.Z`) was merged.
    ///
    /// Triggers the Release Automator to create a Git tag and a GitHub release.
    ReleasePrMerged,

    /// A comment was posted on a pull request (regular or release PR).
    ///
    /// The comment processor inspects the payload body for recognised command
    /// patterns before acting. Supported commands include:
    ///
    /// - **Version bump override** — a reviewer may request a larger bump than
    ///   the conventional-commit analysis calculated, e.g. when a breaking
    ///   change was omitted from commit messages:
    ///   `!release major` / `!release minor` / `!release patch`
    ///
    /// - **Explicit version pin** — a reviewer may pin the exact next version
    ///   (must be strictly greater than the current released version):
    ///   `!set-version 2.4.0`
    ///
    /// - **Informational feedback** — Release Regent posts a comment on regular
    ///   PRs that will not trigger a version bump (e.g. `chore:` only commits)
    ///   so that developers are aware their change will not produce a release.
    ///
    /// Comments that do not match any known command are silently ignored.
    PullRequestCommentReceived,

    /// A GitHub event that this version of Release Regent does not recognise.
    ///
    /// The inner `String` preserves the raw event type for diagnostic logging.
    ///
    /// # Serialisation note
    ///
    /// With `#[serde(rename_all = "snake_case")]` the unit variants serialise
    /// as bare JSON strings (e.g. `"pull_request_merged"`), but this newtype
    /// variant serialises as a JSON object (e.g. `{"unknown":"some_novel_event"}`).
    /// Downstream consumers that compare raw JSON text against a serialised
    /// `EventType` must account for this difference.
    Unknown(String),
}

impl From<&str> for EventType {
    /// Parse a raw event-type string into an [`EventType`] variant.
    ///
    /// Unrecognised strings map to [`EventType::Unknown`] rather than returning
    /// an error, because an unknown event should be logged and dropped, not
    /// cause the processing loop to fail.
    fn from(s: &str) -> Self {
        match s {
            "pull_request_merged" => Self::PullRequestMerged,
            "release_pr_merged" => Self::ReleasePrMerged,
            "pull_request_comment_received" => Self::PullRequestCommentReceived,
            other => Self::Unknown(other.to_string()),
        }
    }
}

impl From<String> for EventType {
    fn from(s: String) -> Self {
        Self::from(s.as_str())
    }
}

impl std::fmt::Display for EventType {
    /// Formats the event type as its wire-format string.
    ///
    /// Unit variants produce the same string as their serde serialised form
    /// (e.g. `pull_request_merged`). The `Unknown` variant forwards the inner
    /// raw string, so `format!("{}", EventType::Unknown("foo".into()))` gives
    /// `"foo"` rather than `"unknown"`.
    ///
    /// This is useful for structured log fields:
    /// ```rust
    /// use release_regent_core::traits::event_source::EventType;
    /// let et = EventType::PullRequestMerged;
    /// assert_eq!(et.to_string(), "pull_request_merged");
    /// ```
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PullRequestMerged => write!(f, "pull_request_merged"),
            Self::ReleasePrMerged => write!(f, "release_pr_merged"),
            Self::PullRequestCommentReceived => write!(f, "pull_request_comment_received"),
            Self::Unknown(s) => write!(f, "{s}"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ProcessingEvent
// ─────────────────────────────────────────────────────────────────────────────

/// A normalised, source-agnostic event flowing through the processing pipeline.
///
/// `ProcessingEvent` is the single authoritative event type inside the core
/// domain. All event sources — webhooks, queues, or test fixtures — must convert
/// their native event representation into a `ProcessingEvent` before handing it
/// to the event loop.
///
/// # Correlation tracking
///
/// Each event carries both an [`event_id`](ProcessingEvent::event_id) (assigned by
/// the source) and a [`correlation_id`](ProcessingEvent::correlation_id) (propagated
/// across the entire workflow). Both must be recorded in every tracing span.
///
/// # Serialisation
///
/// `ProcessingEvent` implements `Serialize`/`Deserialize` so it can be written to
/// and read from a message queue body as JSON without a separate DTO.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessingEvent {
    /// Opaque identifier assigned by the event source.
    ///
    /// Passed back to [`EventSource::acknowledge`] / [`EventSource::reject`] so
    /// the source can settle (complete, abandon, or dead-letter) the underlying
    /// message or request. When calling `reject`, also supply `permanent: bool`
    /// to indicate whether the failure is retryable.
    pub event_id: String,

    /// Cross-system correlation identifier for distributed tracing.
    ///
    /// Either forwarded from the upstream request (e.g. `X-Correlation-Id`
    /// HTTP header) or generated fresh by the event source when not present.
    pub correlation_id: String,

    /// Semantic classification used to route the event to the correct handler.
    pub event_type: EventType,

    /// Repository that this event originated from.
    pub repository: RepositoryInfo,

    /// Raw event payload as received from GitHub.
    ///
    /// Kept as [`serde_json::Value`] so that handlers can extract the fields
    /// they need without forcing a single up-front deserialisation schema on
    /// the entire pipeline.
    pub payload: serde_json::Value,

    /// UTC timestamp at which this event was first received by the system.
    pub received_at: DateTime<Utc>,

    /// Delivery mechanism that produced this event.
    pub source: EventSourceKind,
}

// ─────────────────────────────────────────────────────────────────────────────
// EventSource trait
// ─────────────────────────────────────────────────────────────────────────────

/// Abstraction over any mechanism that delivers [`ProcessingEvent`]s to the
/// processing loop.
///
/// The trait is intentionally non-blocking for `next_event`: it returns
/// `Ok(None)` when no event is currently available rather than blocking the
/// caller. The event loop is responsible for sleeping between empty polls.
///
/// # Idempotency contract
///
/// `acknowledge` and `reject` are idempotent. Calling them more than once for
/// the same `event_id` (or for an unknown `event_id`) must succeed without
/// error.
///
/// # Source-specific semantics
///
/// | Source | `acknowledge` | `reject(_, permanent=false)` | `reject(_, permanent=true)` |
/// |--------|--------------|------------------------------|-----------------------------|
/// | Webhook | no-op (fire-and-forget) | no-op | no-op |
/// | Queue   | complete/delete message | re-queue for retry | dead-letter |
/// | Mock    | recorded for assertions | recorded for assertions | recorded for assertions |
#[async_trait]
pub trait EventSource: Send + Sync {
    /// Poll for the next available event.
    ///
    /// Returns `Ok(None)` when no event is currently ready. The caller should
    /// sleep briefly before polling again to avoid a busy-loop.
    ///
    /// # Errors
    ///
    /// - [`CoreError::InvalidInput`] — the source detected a malformed message
    ///   body that cannot be deserialised into a [`ProcessingEvent`].
    /// - Any infrastructure-level error from the underlying delivery mechanism.
    async fn next_event(&self) -> CoreResult<Option<ProcessingEvent>>;

    /// Acknowledge successful processing of the event identified by `event_id`.
    ///
    /// For queue-based sources this permanently removes the message from the
    /// queue. For webhook sources this is a no-op.
    ///
    /// # Parameters
    ///
    /// - `event_id`: The [`ProcessingEvent::event_id`] returned by `next_event`.
    ///
    /// # Errors
    ///
    /// Returns `Err` only for unrecoverable infrastructure failures. An unknown
    /// `event_id` must **not** be treated as an error.
    async fn acknowledge(&self, event_id: &str) -> CoreResult<()>;

    /// Signal that processing of the event identified by `event_id` failed.
    ///
    /// The `permanent` flag distinguishes transient from permanent failures:
    ///
    /// - `permanent = false` — transient failure; the event may be retried.
    ///   Queue-based sources re-queue the message (abandon / nack).
    /// - `permanent = true` — permanent failure; the event must not be retried.
    ///   Queue-based sources route the message to a dead-letter queue (or
    ///   equivalent). Webhook sources treat both values as a no-op.
    ///
    /// # Parameters
    ///
    /// - `event_id`: The [`ProcessingEvent::event_id`] returned by `next_event`.
    /// - `permanent`: Whether this is a permanent (non-retryable) failure.
    ///
    /// # Errors
    ///
    /// Returns `Err` only for unrecoverable infrastructure failures. An unknown
    /// `event_id` must **not** be treated as an error.
    async fn reject(&self, event_id: &str, permanent: bool) -> CoreResult<()>;
}

#[cfg(test)]
#[path = "event_source_tests.rs"]
mod tests;
