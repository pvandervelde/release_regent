use super::*;
use chrono::TimeZone;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_processing_event(source: EventSourceKind) -> ProcessingEvent {
    ProcessingEvent {
        event_id: "evt-001".to_string(),
        correlation_id: "corr-abc".to_string(),
        event_type: EventType::PullRequestMerged,
        repository: RepositoryInfo {
            owner: "octocat".to_string(),
            name: "hello-world".to_string(),
            default_branch: "main".to_string(),
        },
        payload: serde_json::json!({ "action": "closed", "merged": true }),
        received_at: Utc.with_ymd_and_hms(2026, 3, 8, 12, 0, 0).unwrap(),
        source,
    }
}

// ── EventType::from(&str) ─────────────────────────────────────────────────────

#[test]
fn test_event_type_from_str_pull_request_merged_returns_expected() {
    let et: EventType = "pull_request_merged".into();
    assert_eq!(et, EventType::PullRequestMerged);
}

#[test]
fn test_event_type_from_str_release_pr_merged_returns_expected() {
    let et: EventType = "release_pr_merged".into();
    assert_eq!(et, EventType::ReleasePrMerged);
}

#[test]
fn test_event_type_from_str_pull_request_comment_received_returns_expected() {
    let et: EventType = "pull_request_comment_received".into();
    assert_eq!(et, EventType::PullRequestCommentReceived);
}

#[test]
fn test_event_type_from_str_unknown_string_returns_unknown_variant() {
    let et: EventType = "issue_opened".into();
    assert_eq!(et, EventType::Unknown("issue_opened".to_string()));
}

#[test]
fn test_event_type_from_str_empty_string_returns_unknown_variant() {
    let et: EventType = "".into();
    assert_eq!(et, EventType::Unknown(String::new()));
}

// ── EventType::from(String) ───────────────────────────────────────────────────

#[test]
fn test_event_type_from_string_owned_pull_request_merged_returns_expected() {
    let et = EventType::from("pull_request_merged".to_string());
    assert_eq!(et, EventType::PullRequestMerged);
}

#[test]
fn test_event_type_from_string_owned_pull_request_comment_received_returns_expected() {
    let et = EventType::from("pull_request_comment_received".to_string());
    assert_eq!(et, EventType::PullRequestCommentReceived);
}

#[test]
fn test_event_type_from_string_owned_unknown_returns_unknown_variant() {
    let raw = "deployment_created".to_string();
    let et = EventType::from(raw.clone());
    assert_eq!(et, EventType::Unknown(raw));
}

// ── EventSourceKind serde round-trips ─────────────────────────────────────────

#[test]
fn test_event_source_kind_serde_round_trip_webhook() {
    let original = EventSourceKind::Webhook;
    let json = serde_json::to_string(&original).expect("serialise EventSourceKind::Webhook");
    let decoded: EventSourceKind =
        serde_json::from_str(&json).expect("deserialise EventSourceKind::Webhook");
    assert_eq!(original, decoded);
}

#[test]
fn test_event_source_kind_serde_round_trip_queue() {
    let original = EventSourceKind::Queue {
        provider: "azure_service_bus".to_string(),
    };
    let json = serde_json::to_string(&original).expect("serialise EventSourceKind::Queue");
    let decoded: EventSourceKind =
        serde_json::from_str(&json).expect("deserialise EventSourceKind::Queue");
    assert_eq!(original, decoded);
}

// ── EventType serde round-trips ───────────────────────────────────────────────

#[test]
fn test_event_type_serde_round_trip_pull_request_merged() {
    let original = EventType::PullRequestMerged;
    let json = serde_json::to_string(&original).expect("serialise EventType::PullRequestMerged");
    let decoded: EventType =
        serde_json::from_str(&json).expect("deserialise EventType::PullRequestMerged");
    assert_eq!(original, decoded);
}

#[test]
fn test_event_type_serde_round_trip_release_pr_merged() {
    let original = EventType::ReleasePrMerged;
    let json = serde_json::to_string(&original).expect("serialise EventType::ReleasePrMerged");
    let decoded: EventType =
        serde_json::from_str(&json).expect("deserialise EventType::ReleasePrMerged");
    assert_eq!(original, decoded);
}

#[test]
fn test_event_type_serde_round_trip_pull_request_comment_received() {
    let original = EventType::PullRequestCommentReceived;
    let json =
        serde_json::to_string(&original).expect("serialise EventType::PullRequestCommentReceived");
    let decoded: EventType =
        serde_json::from_str(&json).expect("deserialise EventType::PullRequestCommentReceived");
    assert_eq!(original, decoded);
}

#[test]
fn test_event_type_serde_round_trip_unknown() {
    let original = EventType::Unknown("some_novel_event".to_string());
    let json = serde_json::to_string(&original).expect("serialise EventType::Unknown");
    let decoded: EventType = serde_json::from_str(&json).expect("deserialise EventType::Unknown");
    assert_eq!(original, decoded);
}

// ── ProcessingEvent serde round-trips ────────────────────────────────────────

#[test]
fn test_processing_event_serde_round_trip_webhook_source() {
    let original = make_processing_event(EventSourceKind::Webhook);
    let json =
        serde_json::to_string(&original).expect("serialise ProcessingEvent (webhook source)");
    let decoded: ProcessingEvent =
        serde_json::from_str(&json).expect("deserialise ProcessingEvent (webhook source)");
    assert_eq!(original, decoded);
}

#[test]
fn test_processing_event_serde_round_trip_queue_source() {
    let original = make_processing_event(EventSourceKind::Queue {
        provider: "aws_sqs".to_string(),
    });
    let json = serde_json::to_string(&original).expect("serialise ProcessingEvent (queue source)");
    let decoded: ProcessingEvent =
        serde_json::from_str(&json).expect("deserialise ProcessingEvent (queue source)");
    assert_eq!(original, decoded);
}

#[test]
fn test_processing_event_serde_round_trip_comment_event_type() {
    let mut original = make_processing_event(EventSourceKind::Webhook);
    original.event_type = EventType::PullRequestCommentReceived;
    original.payload = serde_json::json!({
        "action": "created",
        "comment": { "body": "!release major" }
    });
    let json =
        serde_json::to_string(&original).expect("serialise ProcessingEvent (comment event type)");
    let decoded: ProcessingEvent =
        serde_json::from_str(&json).expect("deserialise ProcessingEvent (comment event type)");
    assert_eq!(original, decoded);
}

#[test]
fn test_processing_event_serde_preserves_correlation_id() {
    let original = make_processing_event(EventSourceKind::Webhook);
    let json = serde_json::to_string(&original).expect("serialise");
    let decoded: ProcessingEvent = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(decoded.correlation_id, "corr-abc");
}

#[test]
fn test_processing_event_serde_preserves_event_id() {
    let original = make_processing_event(EventSourceKind::Webhook);
    let json = serde_json::to_string(&original).expect("serialise");
    let decoded: ProcessingEvent = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(decoded.event_id, "evt-001");
}

// ── RepositoryInfo serde round-trip ──────────────────────────────────────────

#[test]
fn test_repository_info_serde_round_trip() {
    let original = RepositoryInfo {
        owner: "pvandervelde".to_string(),
        name: "release_regent".to_string(),
        default_branch: "master".to_string(),
    };
    let json = serde_json::to_string(&original).expect("serialise RepositoryInfo");
    let decoded: RepositoryInfo = serde_json::from_str(&json).expect("deserialise RepositoryInfo");
    assert_eq!(original, decoded);
}
