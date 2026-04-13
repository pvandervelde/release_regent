use super::*;

use chrono::Utc;
use github_bot_sdk::{
    client::{OwnerType, Repository, RepositoryOwner},
    events::{EventPayload, EventProcessor, ProcessorConfig},
    webhook::{WebhookReceiver, WebhookRequest},
};
use serde_json::json;
use std::collections::HashMap;
use tokio::sync::mpsc;

// ─────────────────────────────────────────────────────────────────────────────
// Test helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Compute the HMAC-SHA256 of `payload` with `secret`, formatted as
/// `sha256=<hex>` to match the `X-Hub-Signature-256` header.
fn compute_signature(payload: &[u8], secret: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can use any key length");
    mac.update(payload);
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}

/// Build a [`WebhookRequest`] with a valid HMAC-SHA256 signature.
fn signed_webhook_request(event_type: &str, payload: &str, secret: &str) -> WebhookRequest {
    let payload_bytes = payload.as_bytes();
    let signature = compute_signature(payload_bytes, secret);

    let headers = HashMap::from([
        ("x-github-event".to_string(), event_type.to_string()),
        (
            "x-github-delivery".to_string(),
            "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string(),
        ),
        ("x-hub-signature-256".to_string(), signature),
        ("content-type".to_string(), "application/json".to_string()),
    ]);

    WebhookRequest::new(headers, bytes::Bytes::copy_from_slice(payload_bytes))
}

/// Build a [`WebhookRequest`] with a deliberately wrong signature.
fn tampered_webhook_request(event_type: &str, original_payload: &str) -> WebhookRequest {
    let headers = HashMap::from([
        ("x-github-event".to_string(), event_type.to_string()),
        (
            "x-github-delivery".to_string(),
            "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string(),
        ),
        (
            "x-hub-signature-256".to_string(),
            "sha256=0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        ),
    ]);

    // tampered body: append extra bytes so HMAC will not match
    let tampered = format!("{original_payload}TAMPERED");
    WebhookRequest::new(headers, bytes::Bytes::copy_from_slice(tampered.as_bytes()))
}

/// Construct a minimal [`Repository`] for use in SDK tests.
fn make_sdk_repository(full_name: &str) -> Repository {
    let (owner_login, repo_name) = full_name.split_once('/').unwrap_or(("owner", full_name));

    Repository {
        id: 1,
        name: repo_name.to_string(),
        full_name: full_name.to_string(),
        owner: RepositoryOwner {
            login: owner_login.to_string(),
            id: 1,
            avatar_url: String::new(),
            owner_type: OwnerType::Organization,
        },
        private: false,
        description: None,
        default_branch: "main".to_string(),
        html_url: String::new(),
        clone_url: String::new(),
        ssh_url: String::new(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

/// Build an [`EventEnvelope`] directly from constituent parts (no HTTP round-trip).
fn make_envelope(event_type: &str, payload: serde_json::Value) -> EventEnvelope {
    EventEnvelope::new(
        event_type.to_string(),
        make_sdk_repository("owner/test-repo"),
        EventPayload::new(payload),
    )
}

/// Minimal GitHub `pull_request` payload for a merged non-release PR.
fn merged_pr_payload() -> serde_json::Value {
    json!({
        "action": "closed",
        "pull_request": {
            "merged": true,
            "head": { "ref": "feature/my-feature" }
        },
        "repository": {
            "id": 1,
            "name": "test-repo",
            "full_name": "owner/test-repo",
            "owner": { "login": "owner", "id": 1, "avatar_url": "",
                       "type": "Organization" },
            "private": false,
            "default_branch": "main",
            "html_url": "", "clone_url": "", "ssh_url": "",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }
    })
}

/// Minimal GitHub `pull_request` payload for a merged release PR.
fn merged_release_pr_payload() -> serde_json::Value {
    json!({
        "action": "closed",
        "pull_request": {
            "merged": true,
            "head": { "ref": "release/v1.2.3" }
        },
        "repository": {
            "id": 1,
            "name": "test-repo",
            "full_name": "owner/test-repo",
            "owner": { "login": "owner", "id": 1, "avatar_url": "",
                       "type": "Organization" },
            "private": false,
            "default_branch": "main",
            "html_url": "", "clone_url": "", "ssh_url": "",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }
    })
}

/// A minimal full webhook JSON payload suitable for `receive_webhook` integration tests.
fn minimal_webhook_payload(action: &str) -> String {
    json!({
        "action": action,
        "repository": {
            "id": 123,
            "name": "test-repo",
            "full_name": "owner/test-repo",
            "owner": {
                "login": "owner",
                "id": 1,
                "avatar_url": "https://github.com/avatars/u/1",
                "type": "Organization"
            },
            "private": false,
            "default_branch": "main",
            "html_url": "https://github.com/owner/test-repo",
            "clone_url": "https://github.com/owner/test-repo.git",
            "ssh_url": "git@github.com:owner/test-repo.git",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }
    })
    .to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// WebhookSecretProvider tests
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_webhook_secret_provider_get_webhook_secret_returns_stored_secret() {
    let provider = WebhookSecretProvider::new("my-secret");
    let result = provider.get_webhook_secret().await;
    assert_eq!(result.unwrap(), "my-secret");
}

#[tokio::test]
async fn test_webhook_secret_provider_get_private_key_returns_not_found() {
    let provider = WebhookSecretProvider::new("any-secret");
    let result = provider.get_private_key().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_webhook_secret_provider_get_app_id_returns_not_found() {
    let provider = WebhookSecretProvider::new("any-secret");
    let result = provider.get_app_id().await;
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// classify_event tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_classify_event_pull_request_closed_merged_regular_returns_pr_merged() {
    let payload = merged_pr_payload();
    let result = classify_event("pull_request", &payload, "release");
    assert_eq!(result, EventType::PullRequestMerged);
}

#[test]
fn test_classify_event_pull_request_closed_merged_release_branch_returns_release_pr_merged() {
    let payload = merged_release_pr_payload();
    let result = classify_event("pull_request", &payload, "release");
    assert_eq!(result, EventType::ReleasePrMerged);
}

#[test]
fn test_classify_event_pull_request_not_merged_returns_unknown_with_action() {
    let payload = json!({
        "action": "closed",
        "pull_request": { "merged": false, "head": { "ref": "feature/x" } }
    });
    let result = classify_event("pull_request", &payload, "release");
    assert!(
        matches!(result, EventType::Unknown(ref s) if s == "pull_request:closed"),
        "non-merged closed PR must return Unknown with action suffix"
    );
}

#[test]
fn test_classify_event_pull_request_opened_returns_unknown_with_action() {
    let payload = json!({
        "action": "opened",
        "pull_request": { "merged": false, "head": { "ref": "feature/x" } }
    });
    let result = classify_event("pull_request", &payload, "release");
    assert!(
        matches!(result, EventType::Unknown(ref s) if s == "pull_request:opened"),
        "opened PR must return Unknown with action suffix"
    );
}

#[test]
fn test_classify_event_issue_comment_on_pr_returns_pr_comment_received() {
    // Payload with "issue.pull_request" present — this is a PR comment.
    let payload = json!({
        "action": "created",
        "issue": {
            "number": 7,
            "pull_request": {
                "url": "https://api.github.com/repos/owner/repo/pulls/7"
            }
        }
    });
    let result = classify_event("issue_comment", &payload, "release");
    assert_eq!(result, EventType::PullRequestCommentReceived);
}

#[test]
fn test_classify_event_issue_comment_on_regular_issue_returns_unknown() {
    // Payload without "issue.pull_request" — this is a plain issue comment.
    let payload = json!({
        "action": "created",
        "issue": {
            "number": 42,
            "title": "Bug report"
            // no "pull_request" key
        }
    });
    let result = classify_event("issue_comment", &payload, "release");
    assert!(
        matches!(result, EventType::Unknown(ref s) if s == "issue_comment:issue"),
        "issue_comment on a plain issue must not be classified as PullRequestCommentReceived"
    );
}

#[test]
fn test_classify_event_pull_request_review_comment_returns_pr_comment_received() {
    let result = classify_event("pull_request_review_comment", &json!({}), "release");
    assert_eq!(result, EventType::PullRequestCommentReceived);
}

#[test]
fn test_classify_event_push_returns_unknown() {
    let result = classify_event("push", &json!({}), "release");
    assert!(matches!(result, EventType::Unknown(s) if s == "push"));
}

#[test]
fn test_classify_event_empty_string_returns_unknown() {
    let result = classify_event("", &json!({}), "release");
    assert!(matches!(result, EventType::Unknown(_)));
}

#[test]
fn test_classify_event_custom_prefix_matching_branch_returns_release_pr_merged() {
    // A deployment that uses "custom" as the branch prefix should have
    // "custom/v1.2.3" branches classified as ReleasePrMerged.
    let payload = json!({
        "action": "closed",
        "pull_request": {
            "merged": true,
            "head": { "ref": "custom/v1.2.3" }
        }
    });
    let result = classify_event("pull_request", &payload, "custom");
    assert_eq!(
        result,
        EventType::ReleasePrMerged,
        "merged PR on custom/v1.2.3 with prefix='custom' must be ReleasePrMerged"
    );
}

#[test]
fn test_classify_event_custom_prefix_non_matching_branch_returns_pr_merged() {
    // With prefix "custom", the standard "release/v*" branch is NOT a release PR.
    let payload = json!({
        "action": "closed",
        "pull_request": {
            "merged": true,
            "head": { "ref": "release/v1.2.3" }
        }
    });
    let result = classify_event("pull_request", &payload, "custom");
    assert_eq!(
        result,
        EventType::PullRequestMerged,
        "merged PR on release/v1.2.3 with prefix='custom' must NOT be ReleasePrMerged"
    );
}

#[test]
fn test_classify_event_default_prefix_unchanged_behavior_for_release_branch() {
    // Regression guard: default "release" prefix keeps existing behavior.
    let payload = json!({
        "action": "closed",
        "pull_request": {
            "merged": true,
            "head": { "ref": "release/v2.0.0" }
        }
    });
    let result = classify_event("pull_request", &payload, "release");
    assert_eq!(
        result,
        EventType::ReleasePrMerged,
        "default prefix='release' must still classify release/v2.0.0 as ReleasePrMerged"
    );
}

#[test]
fn test_classify_event_empty_prefix_merged_pr_returns_pr_merged() {
    // An empty prefix is a programming error. Rather than matching any "/v*" branch,
    // the classifier must fall back to PullRequestMerged and emit a warning.
    let payload = json!({
        "action": "closed",
        "pull_request": {
            "merged": true,
            "head": { "ref": "/v1.0.0" }
        }
    });
    let result = classify_event("pull_request", &payload, "");
    assert_eq!(
        result,
        EventType::PullRequestMerged,
        "empty prefix must not silently match /v* branches as ReleasePrMerged"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// convert_envelope tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_convert_envelope_valid_maps_repository_owner_and_name() {
    let envelope = make_envelope("pull_request", merged_pr_payload());
    let event = convert_envelope(&envelope, "release").expect("conversion must succeed");
    assert_eq!(event.repository.owner, "owner");
    assert_eq!(event.repository.name, "test-repo");
}

#[test]
fn test_convert_envelope_valid_maps_default_branch() {
    let envelope = make_envelope("pull_request", merged_pr_payload());
    let event = convert_envelope(&envelope, "release").expect("conversion must succeed");
    assert_eq!(event.repository.default_branch, "main");
}

#[test]
fn test_convert_envelope_valid_sets_webhook_source_kind() {
    let envelope = make_envelope("pull_request", merged_pr_payload());
    let event = convert_envelope(&envelope, "release").expect("conversion must succeed");
    assert_eq!(event.source, EventSourceKind::Webhook);
}

#[test]
fn test_convert_envelope_valid_classifies_event_type() {
    let envelope = make_envelope("pull_request", merged_pr_payload());
    let event = convert_envelope(&envelope, "release").expect("conversion must succeed");
    assert_eq!(event.event_type, EventType::PullRequestMerged);
}

#[test]
fn test_convert_envelope_invalid_full_name_returns_error() {
    // Make an envelope whose full_name has no slash
    use github_bot_sdk::events::EventPayload;

    let mut repo = make_sdk_repository("owner/repo");
    repo.full_name = "no-slash-here".to_string();

    let envelope = EventEnvelope::new("push".to_string(), repo, EventPayload::new(json!({})));
    let result = convert_envelope(&envelope, "release");
    assert!(result.is_err());
}

#[test]
fn test_convert_envelope_payload_is_preserved() {
    let payload = merged_pr_payload();
    let envelope = make_envelope("pull_request", payload.clone());
    let event = convert_envelope(&envelope, "release").expect("conversion must succeed");
    assert_eq!(event.payload, payload);
}

// ─────────────────────────────────────────────────────────────────────────────
// ReleaseRegentWebhookHandler::is_allowed tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_is_allowed_empty_list_denies_all() {
    let (tx, _rx) = mpsc::channel(1);
    let handler = ReleaseRegentWebhookHandler::new(tx, vec![], "release".to_string());
    assert!(!handler.is_allowed("owner/repo"));
}

#[test]
fn test_is_allowed_wildcard_allows_any_repo() {
    let (tx, _rx) = mpsc::channel(1);
    let handler =
        ReleaseRegentWebhookHandler::new(tx, vec!["*".to_string()], "release".to_string());
    assert!(handler.is_allowed("any/repo"));
    assert!(handler.is_allowed("another/project"));
}

#[test]
fn test_is_allowed_explicit_match_allows_listed_repo() {
    let (tx, _rx) = mpsc::channel(1);
    let handler = ReleaseRegentWebhookHandler::new(
        tx,
        vec!["owner/allowed-repo".to_string()],
        "release".to_string(),
    );
    assert!(handler.is_allowed("owner/allowed-repo"));
}

#[test]
fn test_is_allowed_explicit_match_denies_unlisted_repo() {
    let (tx, _rx) = mpsc::channel(1);
    let handler = ReleaseRegentWebhookHandler::new(
        tx,
        vec!["owner/allowed-repo".to_string()],
        "release".to_string(),
    );
    assert!(!handler.is_allowed("owner/other-repo"));
}

// ─────────────────────────────────────────────────────────────────────────────
// ReleaseRegentWebhookHandler::handle_event tests
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_handle_event_allowed_repo_sends_processing_event() {
    let (tx, mut rx) = mpsc::channel(4);
    let handler =
        ReleaseRegentWebhookHandler::new(tx, vec!["*".to_string()], "release".to_string());

    let envelope = make_envelope("pull_request", merged_pr_payload());
    handler
        .handle_event(&envelope)
        .await
        .expect("handle_event must succeed");

    let event = rx
        .try_recv()
        .expect("expected exactly one event on channel");
    assert_eq!(event.event_type, EventType::PullRequestMerged);
    assert_eq!(event.repository.owner, "owner");
    assert_eq!(event.repository.name, "test-repo");
}

#[tokio::test]
async fn test_handle_event_denied_repo_sends_nothing_to_channel() {
    let (tx, mut rx) = mpsc::channel(4);
    let handler = ReleaseRegentWebhookHandler::new(tx, vec![], "release".to_string()); // deny all

    let envelope = make_envelope("pull_request", merged_pr_payload());
    handler
        .handle_event(&envelope)
        .await
        .expect("handle_event must succeed (even when dropping)");

    assert!(
        rx.try_recv().is_err(),
        "channel must be empty — event should have been dropped"
    );
}

#[tokio::test]
async fn test_handle_event_release_pr_sends_release_pr_merged_event() {
    let (tx, mut rx) = mpsc::channel(4);
    let handler =
        ReleaseRegentWebhookHandler::new(tx, vec!["*".to_string()], "release".to_string());

    let envelope = make_envelope("pull_request", merged_release_pr_payload());
    handler
        .handle_event(&envelope)
        .await
        .expect("handle_event must succeed");

    let event = rx.try_recv().expect("expected event on channel");
    assert_eq!(event.event_type, EventType::ReleasePrMerged);
}

#[tokio::test]
async fn test_handle_event_full_channel_drops_event_without_error() {
    // Channel with capacity 0 is impossible; use capacity 1 and fill it first.
    let (tx, mut rx) = mpsc::channel(1);

    // Pre-fill the channel so the next try_send overflows it.
    let filler = ProcessingEvent {
        event_id: "filler".to_string(),
        correlation_id: "filler".to_string(),
        event_type: EventType::Unknown("filler".to_string()),
        repository: RepositoryInfo {
            owner: "o".to_string(),
            name: "r".to_string(),
            default_branch: "main".to_string(),
        },
        payload: json!({}),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
    };
    tx.try_send(filler).expect("pre-fill must succeed");

    let handler =
        ReleaseRegentWebhookHandler::new(tx, vec!["*".to_string()], "release".to_string());
    let envelope = make_envelope("pull_request", merged_pr_payload());

    // Must not error even though the channel is full.
    handler
        .handle_event(&envelope)
        .await
        .expect("handle_event must return Ok even when channel is full");

    // The channel still holds only the filler event.
    let filler_event = rx.try_recv().expect("filler must still be in channel");
    assert_eq!(filler_event.event_id, "filler");
    assert!(rx.try_recv().is_err(), "no second event should be present");
}

// ─────────────────────────────────────────────────────────────────────────────
// WebhookEventSource tests
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_next_event_empty_channel_returns_none() {
    let (_tx, rx) = mpsc::channel::<ProcessingEvent>(4);
    let source = WebhookEventSource::new(rx);
    let result = source
        .next_event()
        .await
        .expect("next_event must not error");
    assert!(result.is_none());
}

#[tokio::test]
async fn test_next_event_with_event_available_returns_some() {
    let (tx, rx) = mpsc::channel(4);
    let event = ProcessingEvent {
        event_id: "evt-1".to_string(),
        correlation_id: "corr-1".to_string(),
        event_type: EventType::PullRequestMerged,
        repository: RepositoryInfo {
            owner: "owner".to_string(),
            name: "repo".to_string(),
            default_branch: "main".to_string(),
        },
        payload: json!({}),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
    };
    tx.try_send(event.clone()).expect("send must succeed");

    let source = WebhookEventSource::new(rx);
    let received = source
        .next_event()
        .await
        .expect("next_event must not error")
        .expect("expected Some(event)");

    assert_eq!(received.event_id, "evt-1");
    assert_eq!(received.event_type, EventType::PullRequestMerged);
}

#[tokio::test]
async fn test_next_event_returns_none_after_channel_is_drained() {
    let (tx, rx) = mpsc::channel(4);
    let event = ProcessingEvent {
        event_id: "evt-2".to_string(),
        correlation_id: "corr-2".to_string(),
        event_type: EventType::PullRequestMerged,
        repository: RepositoryInfo {
            owner: "o".to_string(),
            name: "r".to_string(),
            default_branch: "main".to_string(),
        },
        payload: json!({}),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
    };
    tx.try_send(event).expect("send must succeed");
    let source = WebhookEventSource::new(rx);

    // First call returns the event.
    let first = source.next_event().await.expect("first call must succeed");
    assert!(first.is_some());

    // Second call returns None — channel is empty.
    let second = source.next_event().await.expect("second call must succeed");
    assert!(second.is_none());
}

#[tokio::test]
async fn test_acknowledge_is_noop_returns_ok() {
    let (_tx, rx) = mpsc::channel::<ProcessingEvent>(4);
    let source = WebhookEventSource::new(rx);
    let result = source.acknowledge("any-event-id").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_reject_is_noop_returns_ok() {
    let (_tx, rx) = mpsc::channel::<ProcessingEvent>(4);
    let source = WebhookEventSource::new(rx);
    assert!(source.reject("any-id", false).await.is_ok());
    assert!(source.reject("any-id", true).await.is_ok());
}

#[tokio::test]
async fn test_next_event_returns_none_when_sender_dropped() {
    let (tx, rx) = mpsc::channel::<ProcessingEvent>(4);
    drop(tx); // disconnect the sender
    let source = WebhookEventSource::new(rx);
    let result = source.next_event().await.expect("must not error");
    assert!(result.is_none(), "disconnected channel must return None");
}

// ─────────────────────────────────────────────────────────────────────────────
// WebhookReceiver integration tests (signature validation paths)
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_receive_webhook_valid_signature_returns_ok() {
    const SECRET: &str = "test-webhook-secret";
    let secret_provider = Arc::new(WebhookSecretProvider::new(SECRET));
    let processor = EventProcessor::new(ProcessorConfig::default());
    let receiver = WebhookReceiver::new(secret_provider, processor);

    let payload = minimal_webhook_payload("opened");
    let request = signed_webhook_request("pull_request", &payload, SECRET);
    let response = receiver.receive_webhook(request).await;

    assert_eq!(
        response.status_code(),
        200,
        "valid signature must yield 200"
    );
}

#[tokio::test]
async fn test_receive_webhook_tampered_body_returns_unauthorized() {
    const SECRET: &str = "test-webhook-secret";
    let secret_provider = Arc::new(WebhookSecretProvider::new(SECRET));
    let processor = EventProcessor::new(ProcessorConfig::default());
    let receiver = WebhookReceiver::new(secret_provider, processor);

    let payload = minimal_webhook_payload("opened");
    let request = tampered_webhook_request("pull_request", &payload);
    let response = receiver.receive_webhook(request).await;

    assert_eq!(response.status_code(), 401, "tampered body must yield 401");
}

#[tokio::test]
async fn test_receive_webhook_missing_signature_header_returns_unauthorized() {
    const SECRET: &str = "test-webhook-secret";
    let secret_provider = Arc::new(WebhookSecretProvider::new(SECRET));
    let processor = EventProcessor::new(ProcessorConfig::default());
    let receiver = WebhookReceiver::new(secret_provider, processor);

    let payload = minimal_webhook_payload("opened");
    let headers = HashMap::from([
        ("x-github-event".to_string(), "pull_request".to_string()),
        (
            "x-github-delivery".to_string(),
            "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string(),
        ),
        // deliberately no x-hub-signature-256
    ]);
    let request = WebhookRequest::new(headers, bytes::Bytes::copy_from_slice(payload.as_bytes()));
    let response = receiver.receive_webhook(request).await;

    assert_eq!(
        response.status_code(),
        401,
        "missing signature must yield 401"
    );
}

#[tokio::test]
async fn test_receive_webhook_missing_event_type_header_returns_bad_request() {
    const SECRET: &str = "test-webhook-secret";
    let secret_provider = Arc::new(WebhookSecretProvider::new(SECRET));
    let processor = EventProcessor::new(ProcessorConfig::default());
    let receiver = WebhookReceiver::new(secret_provider, processor);

    let payload = minimal_webhook_payload("opened");
    let payload_bytes = payload.as_bytes();
    let signature = compute_signature(payload_bytes, SECRET);

    let headers = HashMap::from([
        // deliberately no x-github-event
        (
            "x-github-delivery".to_string(),
            "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string(),
        ),
        ("x-hub-signature-256".to_string(), signature),
    ]);

    let request = WebhookRequest::new(headers, bytes::Bytes::copy_from_slice(payload_bytes));
    let response = receiver.receive_webhook(request).await;

    assert_eq!(
        response.status_code(),
        400,
        "missing event-type header must yield 400"
    );
}

#[tokio::test]
async fn test_receive_webhook_valid_request_invokes_handler_and_sends_event() {
    const SECRET: &str = "handler-integration-secret";

    let (tx, mut rx) = mpsc::channel(4);
    let handler = Arc::new(ReleaseRegentWebhookHandler::new(
        tx,
        vec!["*".to_string()],
        "release".to_string(),
    ));

    let secret_provider = Arc::new(WebhookSecretProvider::new(SECRET));
    let processor = EventProcessor::new(ProcessorConfig::default());
    let mut receiver = WebhookReceiver::new(secret_provider, processor);
    receiver.add_handler(handler).await;

    // Use a standard closed+merged PR payload so event type is PullRequestMerged.
    let payload = json!({
        "action": "closed",
        "pull_request": {
            "merged": true,
            "head": { "ref": "feature/my-feature" }
        },
        "repository": {
            "id": 1,
            "name": "test-repo",
            "full_name": "owner/test-repo",
            "owner": {
                "login": "owner",
                "id": 1,
                "avatar_url": "https://github.com/avatars/u/1",
                "type": "Organization"
            },
            "private": false,
            "default_branch": "main",
            "html_url": "https://github.com/owner/test-repo",
            "clone_url": "https://github.com/owner/test-repo.git",
            "ssh_url": "git@github.com:owner/test-repo.git",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }
    })
    .to_string();

    let request = signed_webhook_request("pull_request", &payload, SECRET);
    let response = receiver.receive_webhook(request).await;
    assert_eq!(response.status_code(), 200);

    // The handler runs fire-and-forget inside the SDK. Wait up to 1 second for
    // the spawned task to deliver the event rather than relying on an arbitrary
    // sleep, which is fragile on slow CI machines.
    let event = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .expect("timed out waiting for fire-and-forget handler to deliver event")
        .expect("channel must not be closed before the event arrives");
    assert_eq!(event.event_type, EventType::PullRequestMerged);
    assert_eq!(event.repository.owner, "owner");
    assert_eq!(event.repository.name, "test-repo");
}
