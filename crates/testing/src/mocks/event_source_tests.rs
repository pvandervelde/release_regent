//! Self-tests for [`MockEventSource`].
//!
//! Verifies that the mock correctly drains pre-loaded events in FIFO order,
//! records `acknowledge` and `reject` calls for assertion, injects one-shot
//! errors, and tracks the remaining queue depth.

use super::*;
use chrono::Utc;
use release_regent_core::{
    traits::event_source::{EventSourceKind, EventType, RepositoryInfo},
    CoreError,
};

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn make_event(id: &str) -> ProcessingEvent {
    ProcessingEvent {
        event_id: id.to_string(),
        correlation_id: format!("corr-{id}"),
        event_type: EventType::PullRequestMerged,
        repository: RepositoryInfo {
            owner: "acme".to_string(),
            name: "app".to_string(),
            default_branch: "main".to_string(),
        },
        payload: serde_json::json!({}),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// next_event
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_next_event_returns_events_in_fifo_order() {
    let mock = MockEventSource::new(vec![make_event("evt-1"), make_event("evt-2")]);

    let first = mock.next_event().await.unwrap();
    let second = mock.next_event().await.unwrap();

    assert_eq!(first.unwrap().event_id, "evt-1");
    assert_eq!(second.unwrap().event_id, "evt-2");
}

#[tokio::test]
async fn test_next_event_returns_none_when_queue_is_empty() {
    let mock = MockEventSource::empty();

    let result = mock.next_event().await.unwrap();

    assert!(result.is_none());
}

#[tokio::test]
async fn test_next_event_returns_none_after_queue_is_drained() {
    let mock = MockEventSource::new(vec![make_event("evt-1")]);

    let _ = mock.next_event().await;
    let result = mock.next_event().await.unwrap();

    assert!(result.is_none());
}

#[tokio::test]
async fn test_empty_constructor_produces_no_events() {
    let mock = MockEventSource::empty();

    assert_eq!(mock.remaining_event_count().await, 0);
    assert!(mock.next_event().await.unwrap().is_none());
}

// ─────────────────────────────────────────────────────────────────────────────
// inject_next_error
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_inject_next_error_returns_error_on_next_call() {
    let mock = MockEventSource::empty();
    mock.inject_next_error(CoreError::network("injected transient failure"));

    let result = mock.next_event().await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_inject_next_error_clears_after_single_use() {
    let mock = MockEventSource::empty();
    mock.inject_next_error(CoreError::network("injected transient failure"));

    let _ = mock.next_event().await; // consumes injected error
    let second = mock.next_event().await;

    assert!(second.is_ok());
    assert!(second.unwrap().is_none());
}

#[tokio::test]
async fn test_inject_error_does_not_consume_queued_event() {
    let mock = MockEventSource::new(vec![make_event("evt-1")]);
    mock.inject_next_error(CoreError::network("injected transient failure"));

    let error_result = mock.next_event().await;
    assert!(error_result.is_err());

    // The queued event must still be available after the injected error fires.
    let event_result = mock.next_event().await.unwrap();
    assert_eq!(event_result.unwrap().event_id, "evt-1");
}

// ─────────────────────────────────────────────────────────────────────────────
// acknowledge
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_acknowledge_records_event_id() {
    let mock = MockEventSource::empty();

    mock.acknowledge("evt-42").await.unwrap();

    assert_eq!(mock.acknowledged_ids().await, vec!["evt-42".to_string()]);
}

#[tokio::test]
async fn test_acknowledged_ids_returns_multiple_in_call_order() {
    let mock = MockEventSource::empty();

    mock.acknowledge("evt-1").await.unwrap();
    mock.acknowledge("evt-2").await.unwrap();
    mock.acknowledge("evt-3").await.unwrap();

    assert_eq!(
        mock.acknowledged_ids().await,
        vec![
            "evt-1".to_string(),
            "evt-2".to_string(),
            "evt-3".to_string()
        ]
    );
}

#[tokio::test]
async fn test_acknowledge_same_event_id_twice_records_both_calls() {
    let mock = MockEventSource::empty();

    mock.acknowledge("evt-1").await.unwrap();
    mock.acknowledge("evt-1").await.unwrap();

    // The mock records every call; idempotency in production is the caller's
    // concern, but the test double must not silently deduplicate.
    assert_eq!(mock.acknowledged_ids().await.len(), 2);
}

// ─────────────────────────────────────────────────────────────────────────────
// reject
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_reject_with_permanent_true_is_recorded() {
    let mock = MockEventSource::empty();

    mock.reject("evt-99", true).await.unwrap();

    assert_eq!(
        mock.rejected_ids().await,
        vec![("evt-99".to_string(), true)]
    );
}

#[tokio::test]
async fn test_reject_with_permanent_false_is_recorded() {
    let mock = MockEventSource::empty();

    mock.reject("evt-100", false).await.unwrap();

    assert_eq!(
        mock.rejected_ids().await,
        vec![("evt-100".to_string(), false)]
    );
}

#[tokio::test]
async fn test_rejected_ids_returns_multiple_in_call_order() {
    let mock = MockEventSource::empty();

    mock.reject("evt-1", true).await.unwrap();
    mock.reject("evt-2", false).await.unwrap();
    mock.reject("evt-3", true).await.unwrap();

    assert_eq!(
        mock.rejected_ids().await,
        vec![
            ("evt-1".to_string(), true),
            ("evt-2".to_string(), false),
            ("evt-3".to_string(), true),
        ]
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// remaining_event_count
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_remaining_event_count_decrements_on_each_next_event() {
    let mock = MockEventSource::new(vec![make_event("evt-1"), make_event("evt-2")]);

    assert_eq!(mock.remaining_event_count().await, 2);
    let _ = mock.next_event().await;
    assert_eq!(mock.remaining_event_count().await, 1);
    let _ = mock.next_event().await;
    assert_eq!(mock.remaining_event_count().await, 0);
}

// ─────────────────────────────────────────────────────────────────────────────
// acknowledge and reject are independent
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_acknowledge_and_reject_do_not_interfere_with_each_other() {
    let mock = MockEventSource::empty();

    mock.acknowledge("evt-a").await.unwrap();
    mock.reject("evt-b", false).await.unwrap();

    assert_eq!(mock.acknowledged_ids().await, vec!["evt-a".to_string()]);
    assert_eq!(
        mock.rejected_ids().await,
        vec![("evt-b".to_string(), false)]
    );
}
