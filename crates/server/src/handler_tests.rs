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
use tracing_test::traced_test;

// ─────────────────────────────────────────────────────────────────────────────
// Test helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Compute the HMAC-SHA256 of `payload` with `secret`, formatted as
/// `sha256=<hex>` to match the `X-Hub-Signature-256` header.
fn compute_signature(payload: &[u8], secret: &str) -> String {
    use hmac::{Hmac, KeyInit, Mac};
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

/// Build an [`EventEnvelope`] for a specific repository `full_name` (no HTTP
/// round-trip). Unlike [`make_envelope`] (which hardcodes `"owner/test-repo"`),
/// this lets allow-list/exclude-list tests control the repository under test.
fn make_envelope_for_repo(
    event_type: &str,
    full_name: &str,
    payload: serde_json::Value,
) -> EventEnvelope {
    EventEnvelope::new(
        event_type.to_string(),
        make_sdk_repository(full_name),
        EventPayload::new(payload),
    )
}

/// Compile glob pattern strings directly via `glob::Pattern::new`, bypassing
/// `compile_repo_patterns` (which is stubbed with `todo!()` during the RED
/// phase of TDD). Used by tests that exercise `is_allowed` / `handle_event`
/// filtering logic in isolation from pattern *compilation* concerns, so that
/// an `is_allowed`-focused test fails for the `is_allowed` stub reason, not
/// because `compile_repo_patterns` is also unimplemented.
///
/// Patterns are used as-is (NOT lowercased) — use [`lower_pats`] when a test
/// needs to simulate what `compile_repo_patterns` does at startup.
fn pats(strs: &[&str]) -> Vec<glob::Pattern> {
    strs.iter()
        .map(|s| glob::Pattern::new(s).expect("test pattern must be valid glob syntax"))
        .collect()
}

/// Same as [`pats`], but lowercases each pattern first — simulating the
/// lowercase-normalization `compile_repo_patterns` is expected to perform at
/// startup (BA-70, BA-73). Use this whenever a test asserts case-insensitive
/// matching behavior of `is_allowed`.
fn lower_pats(strs: &[&str]) -> Vec<glob::Pattern> {
    strs.iter()
        .map(|s| {
            glob::Pattern::new(&s.to_lowercase()).expect("test pattern must be valid glob syntax")
        })
        .collect()
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
    let result = classify_event("pull_request", &payload, "release", "v");
    assert_eq!(result, EventType::PullRequestMerged);
}

#[test]
fn test_classify_event_pull_request_closed_merged_release_branch_returns_release_pr_merged() {
    let payload = merged_release_pr_payload();
    let result = classify_event("pull_request", &payload, "release", "v");
    assert_eq!(result, EventType::ReleasePrMerged);
}

#[test]
fn test_classify_event_pull_request_not_merged_returns_unknown_with_action() {
    let payload = json!({
        "action": "closed",
        "pull_request": { "merged": false, "head": { "ref": "feature/x" } }
    });
    let result = classify_event("pull_request", &payload, "release", "v");
    assert!(
        matches!(result, EventType::Unknown(ref s) if s == "pull_request:closed"),
        "non-merged closed PR must return Unknown with action suffix"
    );
}

#[test]
fn test_classify_event_pull_request_opened_returns_pull_request_opened() {
    let payload = json!({
        "action": "opened",
        "pull_request": { "merged": false, "head": { "ref": "feature/x" } }
    });
    let result = classify_event("pull_request", &payload, "release", "v");
    assert_eq!(
        result,
        EventType::PullRequestOpened,
        "opened PR must map to PullRequestOpened"
    );
}

#[test]
fn test_classify_event_pull_request_synchronize_returns_pull_request_updated() {
    let payload = json!({
        "action": "synchronize",
        "pull_request": { "merged": false, "head": { "ref": "feature/x" } }
    });
    let result = classify_event("pull_request", &payload, "release", "v");
    assert_eq!(result, EventType::PullRequestUpdated);
}

#[test]
fn test_classify_event_pull_request_ready_for_review_returns_pull_request_updated() {
    let payload = json!({
        "action": "ready_for_review",
        "pull_request": { "merged": false, "head": { "ref": "feature/x" } }
    });
    let result = classify_event("pull_request", &payload, "release", "v");
    assert_eq!(result, EventType::PullRequestUpdated);
}

#[test]
fn test_classify_event_pull_request_edited_returns_pull_request_updated() {
    let payload = json!({
        "action": "edited",
        "pull_request": { "merged": false, "head": { "ref": "feature/x" } }
    });
    let result = classify_event("pull_request", &payload, "release", "v");
    assert_eq!(result, EventType::PullRequestUpdated);
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
    let result = classify_event("issue_comment", &payload, "release", "v");
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
    let result = classify_event("issue_comment", &payload, "release", "v");
    assert!(
        matches!(result, EventType::Unknown(ref s) if s == "issue_comment:issue"),
        "issue_comment on a plain issue must not be classified as PullRequestCommentReceived"
    );
}

#[test]
fn test_classify_event_pull_request_review_comment_returns_pr_comment_received() {
    let result = classify_event("pull_request_review_comment", &json!({}), "release", "v");
    assert_eq!(result, EventType::PullRequestCommentReceived);
}

#[test]
fn test_classify_event_push_returns_unknown() {
    let result = classify_event("push", &json!({}), "release", "v");
    assert!(matches!(result, EventType::Unknown(s) if s == "push"));
}

#[test]
fn test_classify_event_empty_string_returns_unknown() {
    let result = classify_event("", &json!({}), "release", "v");
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
    let result = classify_event("pull_request", &payload, "custom", "v");
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
    let result = classify_event("pull_request", &payload, "custom", "v");
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
    let result = classify_event("pull_request", &payload, "release", "v");
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
    let result = classify_event("pull_request", &payload, "", "v");
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
    let event = convert_envelope(&envelope, "release", "v").expect("conversion must succeed");
    assert_eq!(event.repository.owner, "owner");
    assert_eq!(event.repository.name, "test-repo");
}

#[test]
fn test_convert_envelope_valid_maps_default_branch() {
    let envelope = make_envelope("pull_request", merged_pr_payload());
    let event = convert_envelope(&envelope, "release", "v").expect("conversion must succeed");
    assert_eq!(event.repository.default_branch, "main");
}

#[test]
fn test_convert_envelope_valid_sets_webhook_source_kind() {
    let envelope = make_envelope("pull_request", merged_pr_payload());
    let event = convert_envelope(&envelope, "release", "v").expect("conversion must succeed");
    assert_eq!(event.source, EventSourceKind::Webhook);
}

#[test]
fn test_convert_envelope_valid_classifies_event_type() {
    let envelope = make_envelope("pull_request", merged_pr_payload());
    let event = convert_envelope(&envelope, "release", "v").expect("conversion must succeed");
    assert_eq!(event.event_type, EventType::PullRequestMerged);
}

#[test]
fn test_convert_envelope_invalid_full_name_returns_error() {
    // Make an envelope whose full_name has no slash
    use github_bot_sdk::events::EventPayload;

    let mut repo = make_sdk_repository("owner/repo");
    repo.full_name = "no-slash-here".to_string();

    let envelope = EventEnvelope::new("push".to_string(), repo, EventPayload::new(json!({})));
    let result = convert_envelope(&envelope, "release", "v");
    assert!(result.is_err());
}

#[test]
fn test_convert_envelope_payload_is_preserved() {
    let payload = merged_pr_payload();
    let envelope = make_envelope("pull_request", payload.clone());
    let event = convert_envelope(&envelope, "release", "v").expect("conversion must succeed");
    assert_eq!(event.payload, payload);
}

// ─────────────────────────────────────────────────────────────────────────────
// compile_repo_patterns tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_compile_repo_patterns_valid_globs_returns_compiled_patterns() {
    let raw = vec!["myorg/service-*".to_string(), "otherorg/*".to_string()];
    let result = compile_repo_patterns("allowed_repos", &raw);

    let compiled = result.expect("valid glob patterns must compile");
    assert_eq!(compiled.len(), 2);
    assert!(compiled[0].matches("myorg/service-api"));
    assert!(compiled[1].matches("otherorg/anything"));
}

#[test]
fn test_compile_repo_patterns_empty_input_returns_empty_vec() {
    let result = compile_repo_patterns("allowed_repos", &[]);
    let compiled = result.expect("empty input must compile to an empty (not erroring) Vec");
    assert!(
        compiled.is_empty(),
        "expected an empty Vec of patterns, got: {compiled:?}"
    );
}

#[test]
fn test_compile_repo_patterns_lowercases_pattern_before_compiling() {
    // BA-70/BA-73 support: compile_repo_patterns must lowercase each raw
    // pattern before compiling it, so that later matching against a
    // lowercased subject is case-insensitive.
    let raw = vec!["MyOrg/Service-*".to_string()];
    let compiled =
        compile_repo_patterns("allowed_repos", &raw).expect("valid pattern must compile");

    assert_eq!(compiled.len(), 1);
    assert_eq!(
        compiled[0].as_str(),
        "myorg/service-*",
        "pattern must be lowercased before compilation"
    );
}

#[test]
fn test_compile_repo_patterns_malformed_pattern_returns_invalid_repo_pattern_error() {
    // BA-71: a malformed glob pattern must fail, not be silently treated as a
    // non-matching literal string.
    let raw = vec!["myorg/[unterminated".to_string()];
    let result = compile_repo_patterns("allowed_repos", &raw);

    assert!(
        result.is_err(),
        "malformed glob pattern must return an error"
    );
    assert!(
        matches!(result.unwrap_err(), Error::InvalidRepoPattern { .. }),
        "error must be the InvalidRepoPattern variant"
    );
}

#[test]
fn test_compile_repo_patterns_malformed_pattern_error_identifies_offending_pattern() {
    let raw = vec!["myorg/service-*".to_string(), "myorg/[bad".to_string()];
    let result = compile_repo_patterns("allowed_repos", &raw);

    match result {
        Err(Error::InvalidRepoPattern { pattern, .. }) => {
            assert_eq!(
                pattern, "myorg/[bad",
                "error must identify the specific offending pattern, not the whole list"
            );
        }
        other => panic!("expected InvalidRepoPattern error, got: {other:?}"),
    }
}

#[test]
fn test_compile_repo_patterns_malformed_pattern_error_identifies_excluded_list_name() {
    // BA-75: same as BA-71, but the error must identify which list ("excluded_repos")
    // the offending pattern came from — distinct from the "allowed_repos" case above.
    let raw = vec!["myorg/[bad".to_string()];
    let result = compile_repo_patterns("excluded_repos", &raw);

    match result {
        Err(Error::InvalidRepoPattern { list_name, .. }) => {
            assert_eq!(list_name, "excluded_repos");
        }
        other => panic!("expected InvalidRepoPattern error, got: {other:?}"),
    }
}

#[test]
fn test_compile_repo_patterns_wildcard_matches_any_repo() {
    let raw = vec!["*".to_string()];
    let compiled = compile_repo_patterns("allowed_repos", &raw).expect("'*' must compile");
    assert!(compiled[0].matches("any/repo"));
    assert!(compiled[0].matches("another/project"));
}

// ─────────────────────────────────────────────────────────────────────────────
// create_webhook_components tests
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_create_webhook_components_allowed_repo_event_reaches_source() {
    let (handler, source) = create_webhook_components(
        pats(&["myorg/*"]),
        vec![],
        4,
        "release".to_string(),
        "v".to_string(),
    );

    let envelope = make_envelope_for_repo("pull_request", "myorg/service-a", merged_pr_payload());
    handler
        .handle_event(&envelope)
        .await
        .expect("handle_event must succeed");

    let event = source
        .next_event()
        .await
        .expect("next_event must not error")
        .expect("expected an event forwarded through the shared channel");
    assert_eq!(event.repository.owner, "myorg");
}

#[tokio::test]
async fn test_create_webhook_components_excluded_repo_event_never_reaches_source() {
    let (handler, source) = create_webhook_components(
        pats(&["myorg/*"]),
        pats(&["myorg/legacy-secrets"]),
        4,
        "release".to_string(),
        "v".to_string(),
    );

    let envelope =
        make_envelope_for_repo("pull_request", "myorg/legacy-secrets", merged_pr_payload());
    handler
        .handle_event(&envelope)
        .await
        .expect("handle_event must succeed even when dropping");

    let event = source
        .next_event()
        .await
        .expect("next_event must not error");
    assert!(
        event.is_none(),
        "excluded repository's event must never reach the shared channel"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// ReleaseRegentWebhookHandler::is_allowed tests
// ─────────────────────────────────────────────────────────────────────────────

/// Convenience constructor for a handler used only in `is_allowed` unit tests
/// (the `tx`/channel side is irrelevant to these tests).
fn handler_with_scope(
    allowed: Vec<glob::Pattern>,
    excluded: Vec<glob::Pattern>,
) -> ReleaseRegentWebhookHandler {
    let (tx, _rx) = mpsc::channel(1);
    ReleaseRegentWebhookHandler::new(
        tx,
        allowed,
        excluded,
        "release".to_string(),
        "v".to_string(),
    )
}

#[test]
fn test_is_allowed_empty_allowed_list_denies_all() {
    // BA-69: an explicitly empty allow-list must deny every repository.
    let handler = handler_with_scope(vec![], vec![]);
    assert!(!handler.is_allowed("owner/repo"));
}

#[test]
fn test_is_allowed_wildcard_allows_any_repo() {
    // BA-68: the default `["*"]` allow-list must act on every repository.
    let handler = handler_with_scope(pats(&["*"]), vec![]);
    assert!(handler.is_allowed("any/repo"));
    assert!(handler.is_allowed("another/project"));
}

#[test]
fn test_is_allowed_repo_matching_allow_glob_is_forwarded() {
    // BA-66: `myorg/service-api` matches the `myorg/service-*` allow pattern.
    let handler = handler_with_scope(pats(&["myorg/service-*"]), vec![]);
    assert!(handler.is_allowed("myorg/service-api"));
}

#[test]
fn test_is_allowed_repo_not_matching_any_allow_glob_is_denied() {
    // BA-67: `myorg/unrelated-repo` does not match `myorg/service-*`.
    let handler = handler_with_scope(pats(&["myorg/service-*"]), vec![]);
    assert!(!handler.is_allowed("myorg/unrelated-repo"));
}

#[test]
fn test_is_allowed_explicit_match_allows_listed_repo() {
    let handler = handler_with_scope(pats(&["owner/allowed-repo"]), vec![]);
    assert!(handler.is_allowed("owner/allowed-repo"));
}

#[test]
fn test_is_allowed_explicit_match_denies_unlisted_repo() {
    let handler = handler_with_scope(pats(&["owner/allowed-repo"]), vec![]);
    assert!(!handler.is_allowed("owner/other-repo"));
}

#[test]
fn test_is_allowed_multiple_allow_patterns_any_match_allows() {
    // Boundary: `is_allowed` uses "any pattern matches", not "first pattern matches".
    let handler = handler_with_scope(pats(&["teamA/*", "teamB/*"]), vec![]);
    assert!(handler.is_allowed("teamB/some-service"));
}

#[test]
fn test_is_allowed_case_insensitive_pattern_matches_mixed_case_repo() {
    // BA-70: pattern "MyOrg/*" (lowercased to "myorg/*" by compile_repo_patterns)
    // must match "myorg/repo-a" when the incoming full_name is lowercased too.
    let handler = handler_with_scope(lower_pats(&["MyOrg/*"]), vec![]);
    assert!(handler.is_allowed("myorg/repo-a"));
}

#[test]
fn test_is_allowed_case_insensitive_matches_regardless_of_subject_case() {
    // BA-70: the subject ("owner/repo" from the webhook) must ALSO be lowercased
    // before matching — a mixed-case incoming repo name must still match a
    // lowercase-normalized pattern.
    let handler = handler_with_scope(lower_pats(&["MyOrg/*"]), vec![]);
    assert!(handler.is_allowed("MyOrg/Repo-A"));
    assert!(handler.is_allowed("MYORG/REPO-A"));
}

#[test]
fn test_is_allowed_exclude_overrides_allow_when_exclude_is_exact_and_allow_is_broad() {
    // BA-72, direction 1: exact-name exclude overrides a broad allow glob.
    let handler = handler_with_scope(pats(&["myorg/*"]), pats(&["myorg/legacy-secrets"]));
    assert!(!handler.is_allowed("myorg/legacy-secrets"));
    // Sibling repos under the same broad allow glob remain allowed.
    assert!(handler.is_allowed("myorg/other-repo"));
}

#[test]
fn test_is_allowed_exclude_overrides_allow_when_exclude_is_broad_and_allow_is_exact() {
    // BA-72, direction 2: a broad exclude glob overrides an exact-name allow entry.
    let handler = handler_with_scope(pats(&["myorg/legacy-secrets"]), pats(&["myorg/*"]));
    assert!(!handler.is_allowed("myorg/legacy-secrets"));
}

#[test]
fn test_is_allowed_exclude_case_insensitive_denies_mixed_case_repo() {
    // BA-73: exclude-list matching is case-insensitive, same rule as BA-70.
    let handler = handler_with_scope(pats(&["myorg/*"]), lower_pats(&["MyOrg/Legacy-Secrets"]));
    assert!(!handler.is_allowed("myorg/legacy-secrets"));
    assert!(!handler.is_allowed("MyOrg/Legacy-Secrets"));
}

#[test]
fn test_is_allowed_empty_exclude_list_excludes_nothing() {
    // BA-74: an empty exclude-list is NOT a kill switch — every allow-matching
    // repository remains allowed. This is the asymmetric counterpart to
    // BA-69 (empty ALLOW list denies everything).
    let handler = handler_with_scope(pats(&["myorg/*"]), vec![]);
    assert!(handler.is_allowed("myorg/anything"));
    assert!(handler.is_allowed("myorg/legacy-secrets"));
}

#[test]
fn test_is_allowed_multiple_exclude_patterns_any_match_denies() {
    let handler = handler_with_scope(
        pats(&["myorg/*"]),
        pats(&["other/*", "myorg/legacy-secrets"]),
    );
    assert!(!handler.is_allowed("myorg/legacy-secrets"));
}

#[test]
fn test_is_allowed_non_matching_allow_stays_denied_even_with_unrelated_broad_exclude() {
    // Exclude-list must only ever *subtract* from the allow-list, never *add*
    // to it. A repo that fails the allow check stays denied regardless of
    // exclude-list contents.
    let handler = handler_with_scope(pats(&["myorg/*"]), pats(&["otherorg/*"]));
    assert!(!handler.is_allowed("otherorg/repo"));
}

// ─────────────────────────────────────────────────────────────────────────────
// ReleaseRegentWebhookHandler::handle_event tests
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_handle_event_allowed_repo_sends_processing_event() {
    let (tx, mut rx) = mpsc::channel(4);
    let handler = ReleaseRegentWebhookHandler::new(
        tx,
        pats(&["*"]),
        vec![],
        "release".to_string(),
        "v".to_string(),
    );

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
    // Empty allow-list denies all (BA-69).
    let handler = ReleaseRegentWebhookHandler::new(
        tx,
        vec![],
        vec![],
        "release".to_string(),
        "v".to_string(),
    );

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
    let handler = ReleaseRegentWebhookHandler::new(
        tx,
        pats(&["*"]),
        vec![],
        "release".to_string(),
        "v".to_string(),
    );

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
        installation_id: 0,
    };
    tx.try_send(filler).expect("pre-fill must succeed");

    let handler = ReleaseRegentWebhookHandler::new(
        tx,
        pats(&["*"]),
        vec![],
        "release".to_string(),
        "v".to_string(),
    );
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
// handle_event — repository allow-list / exclude-list end-to-end (BA-66, 67, 72, 73)
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
#[traced_test]
async fn test_handle_event_repo_matching_allow_glob_is_forwarded_without_drop_warning() {
    // BA-66: an event for a repo matching the allow-list glob must be forwarded,
    // with no warning logged about the allow-list.
    let (tx, mut rx) = mpsc::channel(4);
    let handler = ReleaseRegentWebhookHandler::new(
        tx,
        pats(&["myorg/service-*"]),
        vec![],
        "release".to_string(),
        "v".to_string(),
    );

    let envelope = make_envelope_for_repo("pull_request", "myorg/service-api", merged_pr_payload());
    handler
        .handle_event(&envelope)
        .await
        .expect("handle_event must succeed");

    let event = rx.try_recv().expect("event must be forwarded to channel");
    assert_eq!(event.repository.owner, "myorg");
    assert_eq!(event.repository.name, "service-api");
    assert!(
        !logs_contain("dropping event"),
        "no drop warning should be logged for an allow-matching repository"
    );
}

#[tokio::test]
#[traced_test]
async fn test_handle_event_repo_not_matching_allow_glob_drops_and_logs_repo_and_event_id() {
    // BA-67: an event for a repo NOT matching any allow-list pattern must be
    // dropped (never reaches the channel), handle_event must still return
    // Ok(()), and the warning log must identify both the repository and the
    // event id.
    let (tx, mut rx) = mpsc::channel(4);
    let handler = ReleaseRegentWebhookHandler::new(
        tx,
        pats(&["myorg/service-*"]),
        vec![],
        "release".to_string(),
        "v".to_string(),
    );

    let envelope =
        make_envelope_for_repo("pull_request", "myorg/unrelated-repo", merged_pr_payload());
    let event_id = envelope.event_id.to_string();

    let result = handler.handle_event(&envelope).await;

    assert!(
        result.is_ok(),
        "handle_event must return Ok(()) even when dropping a non-allow-listed repo"
    );
    assert!(
        rx.try_recv().is_err(),
        "event must never be forwarded to the processing channel"
    );
    assert!(
        logs_contain("myorg/unrelated-repo"),
        "warning log must identify the dropped repository"
    );
    assert!(
        logs_contain(&event_id),
        "warning log must identify the dropped event's id"
    );
}

#[tokio::test]
async fn test_handle_event_repo_matching_exclude_glob_is_dropped_even_though_allow_matches() {
    // BA-72: exclude unconditionally overrides allow, even for an exact-name
    // exclude entry against a broad allow glob.
    let (tx, mut rx) = mpsc::channel(4);
    let handler = ReleaseRegentWebhookHandler::new(
        tx,
        pats(&["myorg/*"]),
        pats(&["myorg/legacy-secrets"]),
        "release".to_string(),
        "v".to_string(),
    );

    let envelope =
        make_envelope_for_repo("pull_request", "myorg/legacy-secrets", merged_pr_payload());
    let result = handler.handle_event(&envelope).await;

    assert!(
        result.is_ok(),
        "handle_event must return Ok(()) when dropping"
    );
    assert!(
        rx.try_recv().is_err(),
        "excluded repository's event must never reach the processing channel"
    );
}

#[tokio::test]
async fn test_handle_event_repo_matching_exclude_glob_case_insensitive_is_dropped() {
    // BA-73: exclude-list matching is case-insensitive.
    let (tx, mut rx) = mpsc::channel(4);
    let handler = ReleaseRegentWebhookHandler::new(
        tx,
        pats(&["myorg/*"]),
        lower_pats(&["MyOrg/Legacy-Secrets"]),
        "release".to_string(),
        "v".to_string(),
    );

    let envelope =
        make_envelope_for_repo("pull_request", "myorg/legacy-secrets", merged_pr_payload());
    handler
        .handle_event(&envelope)
        .await
        .expect("handle_event must succeed");

    assert!(
        rx.try_recv().is_err(),
        "case-insensitively excluded repository's event must never reach the channel"
    );
}

#[tokio::test]
async fn test_handle_event_empty_exclude_list_still_forwards_allow_matching_repo() {
    // BA-74: an unset/empty exclude-list must not behave as a kill switch.
    let (tx, mut rx) = mpsc::channel(4);
    let handler = ReleaseRegentWebhookHandler::new(
        tx,
        pats(&["myorg/*"]),
        vec![],
        "release".to_string(),
        "v".to_string(),
    );

    let envelope = make_envelope_for_repo("pull_request", "myorg/anything", merged_pr_payload());
    handler
        .handle_event(&envelope)
        .await
        .expect("handle_event must succeed");

    let event = rx
        .try_recv()
        .expect("event must be forwarded — exclude-list is empty");
    assert_eq!(event.repository.owner, "myorg");
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
        installation_id: 0,
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
        installation_id: 0,
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
        pats(&["*"]),
        vec![],
        "release".to_string(),
        "v".to_string(),
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

// ─────────────────────────────────────────────────────────────────────────────
// Property-based tests (Tier 3)
//
// These verify invariants of `is_allowed` and `compile_repo_patterns` across
// many generated inputs, rather than a handful of hand-picked examples. They
// are the primary defense against subtle case-folding bugs (e.g. an
// implementation that only lowercases the pattern OR the subject, but not
// both) and against exclude-list / allow-list precedence bugs that a small
// fixed set of example tests might miss.
// ─────────────────────────────────────────────────────────────────────────────

mod property_tests {
    use super::*;
    use proptest::prelude::*;

    /// Restrict generated repo-name segments to the character classes GitHub
    /// actually allows in `owner`/`repo` names (alphanumeric, `-`, `_`, `.`),
    /// with mixed case so tests exercise BA-70/BA-73 case-insensitivity.
    fn repo_segment() -> impl Strategy<Value = String> {
        "[A-Za-z][A-Za-z0-9_.-]{0,15}"
    }

    fn full_name_strategy() -> impl Strategy<Value = (String, String)> {
        (repo_segment(), repo_segment())
    }

    proptest! {
        /// A wildcard `"*"` allow pattern must match every generated `owner/repo`
        /// combination, in any case combination, with an empty exclude-list
        /// (BA-68's "acts on every repository" default semantics).
        #[test]
        fn prop_is_allowed_wildcard_matches_any_repo_any_case((owner, repo) in full_name_strategy()) {
            let handler = handler_with_scope(lower_pats(&["*"]), vec![]);
            let full_name = format!("{owner}/{repo}");
            prop_assert!(handler.is_allowed(&full_name));
        }

        /// `is_allowed` must never panic for arbitrary (including adversarial,
        /// non-UTF8-adjacent-boundary, empty, or glob-metacharacter-laden)
        /// input strings — it must always resolve to `true` or `false`.
        #[test]
        fn prop_is_allowed_never_panics_on_arbitrary_input(
            full_name in proptest::arbitrary::any::<String>()
        ) {
            let handler = handler_with_scope(lower_pats(&["*"]), vec![]);
            let _ = handler.is_allowed(&full_name);
        }

        /// For any repo name, an allow pattern built from that repo name's exact
        /// lowercased form must match the repo name regardless of the case used
        /// in the incoming (webhook-supplied) full_name — i.e. matching is
        /// case-insensitive on BOTH sides (BA-70).
        #[test]
        fn prop_is_allowed_case_insensitive_for_exact_pattern((owner, repo) in full_name_strategy()) {
            let full_name = format!("{owner}/{repo}");
            let handler = handler_with_scope(lower_pats(&[&full_name]), vec![]);

            // Exercise several case permutations of the same logical repo name.
            prop_assert!(handler.is_allowed(&full_name.to_lowercase()));
            prop_assert!(handler.is_allowed(&full_name.to_uppercase()));
        }

        /// BA-72 invariant: whenever a repo matches BOTH an allow pattern and an
        /// exclude pattern built from its own exact (lowercased) name, the result
        /// must always be `false` — exclude always wins, for any generated repo
        /// name and any broad co-existing allow pattern.
        #[test]
        fn prop_is_allowed_exact_exclude_always_overrides_broad_allow((owner, repo) in full_name_strategy()) {
            let full_name = format!("{owner}/{repo}");
            let broad_allow = format!("{owner}/*");
            let handler = handler_with_scope(
                lower_pats(&[&broad_allow]),
                lower_pats(&[&full_name]),
            );
            prop_assert!(!handler.is_allowed(&full_name.to_lowercase()));
        }

        /// `compile_repo_patterns` must never panic for arbitrary raw pattern
        /// lists — it must always resolve to `Ok` or a well-formed `Err`.
        #[test]
        fn prop_compile_repo_patterns_never_panics_on_arbitrary_input(
            raw in proptest::collection::vec(proptest::arbitrary::any::<String>(), 0..8)
        ) {
            let _ = compile_repo_patterns("allowed_repos", &raw);
        }

        /// For any raw pattern string built only from characters that are valid,
        /// non-metacharacter glob literals (so compilation cannot fail),
        /// `compile_repo_patterns` must produce a pattern whose `as_str()` is
        /// exactly the lowercased input — the lowercase-normalization invariant
        /// underlying BA-70/BA-73 must hold for arbitrary literal segments, not
        /// just the hand-picked "MyOrg" example.
        #[test]
        fn prop_compile_repo_patterns_lowercases_arbitrary_literal_pattern(
            raw in "[A-Za-z][A-Za-z0-9_/-]{0,20}"
        ) {
            let compiled = compile_repo_patterns("allowed_repos", &[raw.clone()])
                .expect("literal alnum/dash/slash pattern must always compile");
            prop_assert_eq!(compiled[0].as_str(), raw.to_lowercase());
        }
    }
}
