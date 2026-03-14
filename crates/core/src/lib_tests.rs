use super::*;
use async_trait::async_trait;
use chrono::Utc;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex as StdMutex},
    time::Duration,
};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use traits::event_source::{
    EventSource, EventSourceKind, EventType, ProcessingEvent, RepositoryInfo,
};

// ─────────────────────────────────────────────────────────────────────────────
// Inline test double (avoids cross-crate type identity issues)
// ─────────────────────────────────────────────────────────────────────────────

/// Minimal in-process `EventSource` for unit tests in this crate.
///
/// Using `release_regent_testing::MockEventSource` directly in `lib_tests.rs`
/// causes a type-identity mismatch: the testing crate is compiled against the
/// *library* artifact of `release_regent_core`, while test code here is
/// compiled as part of that same crate. Defining the mock locally ensures all
/// types come from a single compilation unit.
#[derive(Clone)]
struct TestEventSource {
    events: Arc<Mutex<VecDeque<ProcessingEvent>>>,
    acked: Arc<Mutex<Vec<String>>>,
    rejected: Arc<Mutex<Vec<(String, bool)>>>,
    /// `std::sync::Mutex` so `inject_error` can be called from sync test setup.
    next_error: Arc<StdMutex<Option<CoreError>>>,
}

impl TestEventSource {
    fn new(events: Vec<ProcessingEvent>) -> Self {
        Self {
            events: Arc::new(Mutex::new(events.into())),
            acked: Arc::new(Mutex::new(vec![])),
            rejected: Arc::new(Mutex::new(vec![])),
            next_error: Arc::new(StdMutex::new(None)),
        }
    }

    fn empty() -> Self {
        Self::new(vec![])
    }

    /// Inject a one-shot error to be returned by the next `next_event` call.
    ///
    /// Callable from synchronous test setup (before the async loop is spawned).
    fn inject_error(&self, error: CoreError) {
        *self.next_error.lock().unwrap() = Some(error);
    }

    async fn acknowledged_ids(&self) -> Vec<String> {
        self.acked.lock().await.clone()
    }

    async fn rejected_ids(&self) -> Vec<(String, bool)> {
        self.rejected.lock().await.clone()
    }

    async fn remaining_count(&self) -> usize {
        self.events.lock().await.len()
    }
}

#[async_trait]
impl EventSource for TestEventSource {
    async fn next_event(&self) -> CoreResult<Option<ProcessingEvent>> {
        // Check injected error first (sync lock, no await required).
        let maybe_err = self.next_error.lock().unwrap().take();
        if let Some(e) = maybe_err {
            return Err(e);
        }
        Ok(self.events.lock().await.pop_front())
    }

    async fn acknowledge(&self, event_id: &str) -> CoreResult<()> {
        self.acked.lock().await.push(event_id.to_string());
        Ok(())
    }

    async fn reject(&self, event_id: &str, permanent: bool) -> CoreResult<()> {
        self.rejected
            .lock()
            .await
            .push((event_id.to_string(), permanent));
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn test_repo() -> RepositoryInfo {
    RepositoryInfo {
        owner: "acme".to_string(),
        name: "app".to_string(),
        default_branch: "main".to_string(),
    }
}

fn make_test_event(id: &str, event_type: EventType) -> ProcessingEvent {
    ProcessingEvent {
        event_id: id.to_string(),
        correlation_id: format!("corr-{id}"),
        event_type,
        repository: test_repo(),
        payload: serde_json::json!({}),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ReleaseRegent smoke tests (unchanged)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_release_regent_creation() {
    let config = config::ReleaseRegentConfig::default();
    let regent = ReleaseRegent::new(config);

    assert_eq!(regent.config().core.version_prefix, "v");
    assert_eq!(regent.config().core.branches.main, "main");
}

#[tokio::test]
async fn test_webhook_processing_placeholder() {
    let config = config::ReleaseRegentConfig::default();
    let regent = ReleaseRegent::new(config);

    let event = webhook::WebhookEvent::new(
        "pull_request".to_string(),
        "closed".to_string(),
        serde_json::json!({}),
        std::collections::HashMap::new(),
    );

    // This should succeed with the placeholder implementation
    let result = regent.process_webhook(event).await;
    assert!(result.is_ok());
}

// ─────────────────────────────────────────────────────────────────────────────
// run_event_loop tests
// ─────────────────────────────────────────────────────────────────────────────

/// A pre-cancelled token causes the loop to return immediately without
/// consuming any events.
#[tokio::test]
async fn test_run_event_loop_exits_immediately_when_token_precancelled() {
    let token = CancellationToken::new();
    token.cancel();

    let source = TestEventSource::new(vec![make_test_event(
        "evt-never",
        EventType::PullRequestMerged,
    )]);

    let result = run_event_loop(&source, token).await;
    assert!(result.is_ok());
    // Event was never consumed because the token was already cancelled.
    assert_eq!(source.remaining_count().await, 1);
    assert!(source.acknowledged_ids().await.is_empty());
}

/// A single `PullRequestMerged` event is processed and acknowledged.
#[tokio::test]
async fn test_run_event_loop_acknowledges_pull_request_merged_event() {
    let source = TestEventSource::new(vec![make_test_event(
        "evt-pr-1",
        EventType::PullRequestMerged,
    )]);
    let source_for_loop = source.clone();

    let token = CancellationToken::new();
    let token_loop = token.clone();
    let token_cancel = token.clone();

    let loop_handle =
        tokio::spawn(async move { run_event_loop(&source_for_loop, token_loop).await });

    tokio::time::sleep(Duration::from_millis(300)).await;
    token_cancel.cancel();

    loop_handle.await.unwrap().unwrap();
    assert_eq!(source.acknowledged_ids().await, vec!["evt-pr-1"]);
    assert!(source.rejected_ids().await.is_empty());
}

/// A single `ReleasePrMerged` event is processed and acknowledged.
#[tokio::test]
async fn test_run_event_loop_acknowledges_release_pr_merged_event() {
    let source = TestEventSource::new(vec![make_test_event(
        "evt-rel-1",
        EventType::ReleasePrMerged,
    )]);
    let source_for_loop = source.clone();

    let token = CancellationToken::new();
    let token_loop = token.clone();
    let token_cancel = token.clone();

    let loop_handle =
        tokio::spawn(async move { run_event_loop(&source_for_loop, token_loop).await });

    tokio::time::sleep(Duration::from_millis(300)).await;
    token_cancel.cancel();

    loop_handle.await.unwrap().unwrap();
    assert_eq!(source.acknowledged_ids().await, vec!["evt-rel-1"]);
    assert!(source.rejected_ids().await.is_empty());
}

/// `PullRequestCommentReceived` events are acknowledged.
#[tokio::test]
async fn test_run_event_loop_acknowledges_pr_comment_event() {
    let source = TestEventSource::new(vec![make_test_event(
        "evt-comment-1",
        EventType::PullRequestCommentReceived,
    )]);
    let source_for_loop = source.clone();

    let token = CancellationToken::new();
    let token_loop = token.clone();
    let token_cancel = token.clone();

    let loop_handle =
        tokio::spawn(async move { run_event_loop(&source_for_loop, token_loop).await });

    tokio::time::sleep(Duration::from_millis(300)).await;
    token_cancel.cancel();

    loop_handle.await.unwrap().unwrap();
    assert_eq!(source.acknowledged_ids().await, vec!["evt-comment-1"]);
    assert!(source.rejected_ids().await.is_empty());
}

/// Unknown event types are acknowledged (logged-and-dropped, not errors).
#[tokio::test]
async fn test_run_event_loop_acknowledges_unknown_event_type() {
    let source = TestEventSource::new(vec![make_test_event(
        "evt-unknown-1",
        EventType::Unknown("novel_event".to_string()),
    )]);
    let source_for_loop = source.clone();

    let token = CancellationToken::new();
    let token_loop = token.clone();
    let token_cancel = token.clone();

    let loop_handle =
        tokio::spawn(async move { run_event_loop(&source_for_loop, token_loop).await });

    tokio::time::sleep(Duration::from_millis(300)).await;
    token_cancel.cancel();

    loop_handle.await.unwrap().unwrap();
    assert_eq!(source.acknowledged_ids().await, vec!["evt-unknown-1"]);
    assert!(source.rejected_ids().await.is_empty());
}

/// Multiple events are processed in FIFO order and all acknowledged.
#[tokio::test]
async fn test_run_event_loop_processes_multiple_events_in_order() {
    let source = TestEventSource::new(vec![
        make_test_event("evt-a", EventType::PullRequestMerged),
        make_test_event("evt-b", EventType::ReleasePrMerged),
        make_test_event("evt-c", EventType::PullRequestMerged),
    ]);
    let source_for_loop = source.clone();

    let token = CancellationToken::new();
    let token_loop = token.clone();
    let token_cancel = token.clone();

    let loop_handle =
        tokio::spawn(async move { run_event_loop(&source_for_loop, token_loop).await });

    tokio::time::sleep(Duration::from_millis(500)).await;
    token_cancel.cancel();

    loop_handle.await.unwrap().unwrap();
    assert_eq!(
        source.acknowledged_ids().await,
        vec!["evt-a", "evt-b", "evt-c"]
    );
    assert!(source.rejected_ids().await.is_empty());
}

/// A transient source error is logged and the loop continues; the event
/// that follows the error is still processed and acknowledged.
#[tokio::test]
async fn test_run_event_loop_continues_after_source_error() {
    let source = TestEventSource::new(vec![make_test_event(
        "evt-after-err",
        EventType::PullRequestMerged,
    )]);
    source.inject_error(CoreError::network("transient source failure"));
    let source_for_loop = source.clone();

    let token = CancellationToken::new();
    let token_loop = token.clone();
    let token_cancel = token.clone();

    let loop_handle =
        tokio::spawn(async move { run_event_loop(&source_for_loop, token_loop).await });

    // Allow: error iteration + 100 ms sleep + event processing + 100 ms sleep + margin
    tokio::time::sleep(Duration::from_millis(500)).await;
    token_cancel.cancel();

    loop_handle.await.unwrap().unwrap();
    // The event after the injected error was still processed.
    assert_eq!(source.acknowledged_ids().await, vec!["evt-after-err"]);
    assert!(source.rejected_ids().await.is_empty());
}

/// An empty source with a cancellation token exits cleanly.
#[tokio::test]
async fn test_run_event_loop_empty_source_exits_cleanly() {
    let source = TestEventSource::empty();
    let source_for_loop = source.clone();

    let token = CancellationToken::new();
    let token_loop = token.clone();
    let token_cancel = token.clone();

    let loop_handle =
        tokio::spawn(async move { run_event_loop(&source_for_loop, token_loop).await });

    tokio::time::sleep(Duration::from_millis(150)).await;
    token_cancel.cancel();

    loop_handle.await.unwrap().unwrap();
    assert!(source.acknowledged_ids().await.is_empty());
    assert!(source.rejected_ids().await.is_empty());
}
