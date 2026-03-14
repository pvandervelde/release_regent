//! Mock implementation of the [`EventSource`] trait.
//!
//! Provides a deterministic, in-process event source for unit and integration tests.
//! Events are pre-loaded at construction time and drained in FIFO order by
//! [`next_event`](MockEventSource::next_event). Calls to `acknowledge` and `reject` are
//! recorded and exposed through assertion helpers so tests can verify the event loop's
//! settlement behaviour.
//!
//! # Usage
//!
//! ```rust
//! use release_regent_testing::mocks::event_source::MockEventSource;
//! use release_regent_core::traits::event_source::{EventType, ProcessingEvent,
//!     EventSourceKind, RepositoryInfo};
//! use chrono::Utc;
//!
//! let event = ProcessingEvent {
//!     event_id: "evt-1".to_string(),
//!     correlation_id: "corr-1".to_string(),
//!     event_type: EventType::PullRequestMerged,
//!     repository: RepositoryInfo {
//!         owner: "acme".to_string(),
//!         name: "app".to_string(),
//!         default_branch: "main".to_string(),
//!     },
//!     payload: serde_json::json!({}),
//!     received_at: Utc::now(),
//!     source: EventSourceKind::Webhook,
//! };
//!
//! let mock = MockEventSource::new(vec![event]);
//! ```

use async_trait::async_trait;
use release_regent_core::{
    traits::event_source::{EventSource, ProcessingEvent},
    CoreError, CoreResult,
};
use std::sync::Arc;
use tokio::sync::Mutex;

// ─────────────────────────────────────────────────────────────────────────────
// Shared state
// ─────────────────────────────────────────────────────────────────────────────

/// Internal mutable state protected by an async `Mutex`.
#[derive(Debug, Default)]
struct MockEventSourceState {
    /// Queue of events to return from `next_event`, drained FIFO.
    events: std::collections::VecDeque<ProcessingEvent>,
    /// `event_id` values passed to `acknowledge`.
    acknowledged: Vec<String>,
    /// `(event_id, permanent)` values passed to `reject`.
    rejected: Vec<(String, bool)>,
    /// Optional error to inject from `next_event` (injected once, then cleared).
    next_error: Option<CoreError>,
}

// ─────────────────────────────────────────────────────────────────────────────
// MockEventSource
// ─────────────────────────────────────────────────────────────────────────────

/// Deterministic, in-process [`EventSource`] for tests.
///
/// Events are pre-loaded via [`new`](MockEventSource::new) (or
/// [`with_error`](MockEventSource::with_error)) and drained in FIFO order.
/// `next_event` returns `Ok(None)` when the queue is empty.
///
/// All `acknowledge` and `reject` calls are recorded; use the assertion helpers
/// to verify the event loop settled events correctly.
///
/// # Thread Safety
///
/// `MockEventSource` is `Send + Sync` and safe for use in multi-threaded test
/// harnesses.
#[derive(Debug, Clone)]
pub struct MockEventSource {
    state: Arc<Mutex<MockEventSourceState>>,
}

impl MockEventSource {
    /// Create a mock pre-loaded with `events`.
    ///
    /// Events are consumed from the front of the `Vec` on successive
    /// `next_event` calls.
    pub fn new(events: Vec<ProcessingEvent>) -> Self {
        Self {
            state: Arc::new(Mutex::new(MockEventSourceState {
                events: events.into(),
                ..Default::default()
            })),
        }
    }

    /// Create an empty mock with no pre-loaded events.
    pub fn empty() -> Self {
        Self::new(vec![])
    }

    /// Inject an error that will be returned by the **next** `next_event` call.
    ///
    /// After the error is returned once the queue reverts to normal operation.
    pub fn inject_next_error(&self, error: CoreError) {
        // Block-on to keep API sync; tests run inside a Tokio runtime so this is safe.
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            let mut state = self.state.lock().await;
            state.next_error = Some(error);
        });
    }

    // ─── Assertion helpers ────────────────────────────────────────────────

    /// Returns the `event_id` values that have been passed to `acknowledge`,
    /// in call order.
    pub async fn acknowledged_ids(&self) -> Vec<String> {
        self.state.lock().await.acknowledged.clone()
    }

    /// Returns `(event_id, permanent)` pairs passed to `reject`, in call order.
    pub async fn rejected_ids(&self) -> Vec<(String, bool)> {
        self.state.lock().await.rejected.clone()
    }

    /// Returns the number of events still in the queue waiting to be delivered.
    pub async fn remaining_event_count(&self) -> usize {
        self.state.lock().await.events.len()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// EventSource impl
// ─────────────────────────────────────────────────────────────────────────────

#[async_trait]
impl EventSource for MockEventSource {
    /// Dequeue and return the next pre-loaded event, or `Ok(None)` when empty.
    ///
    /// If an error was injected via [`inject_next_error`](MockEventSource::inject_next_error),
    /// that error is returned and cleared before any queued event is checked.
    async fn next_event(&self) -> CoreResult<Option<ProcessingEvent>> {
        let mut state = self.state.lock().await;

        if let Some(err) = state.next_error.take() {
            return Err(err);
        }

        Ok(state.events.pop_front())
    }

    /// Record the acknowledgement.
    ///
    /// This is always `Ok(())` — the mock never fails on `acknowledge`.
    async fn acknowledge(&self, event_id: &str) -> CoreResult<()> {
        self.state
            .lock()
            .await
            .acknowledged
            .push(event_id.to_string());
        Ok(())
    }

    /// Record the rejection.
    ///
    /// This is always `Ok(())` — the mock never fails on `reject`.
    async fn reject(&self, event_id: &str, permanent: bool) -> CoreResult<()> {
        self.state
            .lock()
            .await
            .rejected
            .push((event_id.to_string(), permanent));
        Ok(())
    }
}
