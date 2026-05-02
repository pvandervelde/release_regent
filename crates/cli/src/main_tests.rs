use super::*;
use release_regent_core::{traits::event_source::ProcessingEvent, CoreResult};
use std::sync::{Arc, Mutex};

/// Minimal handler that records the event types it processes.
#[derive(Clone)]
struct SpyHandler {
    events: Arc<Mutex<Vec<String>>>,
}

impl SpyHandler {
    fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn received_event_types(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }

    fn received_count(&self) -> usize {
        self.events.lock().unwrap().len()
    }
}

/// Separate spy that also records full events for metadata assertions.
#[derive(Clone)]
struct EventSpyHandler {
    events: Arc<Mutex<Vec<ProcessingEvent>>>,
}

impl EventSpyHandler {
    fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait::async_trait]
impl release_regent_core::MergedPullRequestHandler for SpyHandler {
    async fn handle_merged_pull_request(&self, event: &ProcessingEvent) -> CoreResult<()> {
        self.events
            .lock()
            .unwrap()
            .push(event.event_type.to_string());
        Ok(())
    }

    async fn handle_release_pr_merged(&self, event: &ProcessingEvent) -> CoreResult<()> {
        self.events
            .lock()
            .unwrap()
            .push(event.event_type.to_string());
        Ok(())
    }

    async fn handle_pr_comment(&self, event: &ProcessingEvent) -> CoreResult<()> {
        self.events
            .lock()
            .unwrap()
            .push(event.event_type.to_string());
        Ok(())
    }

    async fn handle_pull_request_activity(&self, event: &ProcessingEvent) -> CoreResult<()> {
        self.events
            .lock()
            .unwrap()
            .push(event.event_type.to_string());
        Ok(())
    }
}

#[async_trait::async_trait]
impl release_regent_core::MergedPullRequestHandler for EventSpyHandler {
    async fn handle_merged_pull_request(&self, event: &ProcessingEvent) -> CoreResult<()> {
        self.events.lock().unwrap().push(event.clone());
        Ok(())
    }
}

fn sample_payload(owner: &str, repo: &str) -> serde_json::Value {
    serde_json::json!({
        "repository": {
            "name": repo,
            "owner": { "login": owner },
            "default_branch": "main"
        },
        "installation": { "id": 12_345_678 }
    })
}

#[test]
fn test_cli_parsing() {
    // Basic smoke test — expanded separately
}

#[test]
fn test_sample_webhook_generation() {
    let webhook = generate_sample_webhook();
    assert!(!webhook.is_empty());

    let parsed: serde_json::Value = serde_json::from_str(&webhook).unwrap();
    assert!(parsed.get("action").is_some());
    assert!(parsed.get("pull_request").is_some());
    assert!(parsed.get("repository").is_some());
    // installation object must be present for production authentication
    assert!(
        parsed.get("installation").is_some(),
        "sample webhook must include 'installation' object"
    );
    assert!(
        parsed["installation"]["id"].as_u64().unwrap_or(0) > 0,
        "installation.id must be a non-zero number"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// dispatch_event tests
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_dispatch_event_pull_request_merged() {
    let handler = SpyHandler::new();
    let cloned = handler.clone();
    let payload = sample_payload("owner", "repo");

    dispatch_event(cloned, "pull_request_merged", payload)
        .await
        .expect("dispatch should succeed");

    assert_eq!(
        handler.received_event_types(),
        vec!["pull_request_merged"],
        "handle_merged_pull_request should have been called"
    );
}

#[tokio::test]
async fn test_dispatch_event_release_pr_merged() {
    let handler = SpyHandler::new();
    let cloned = handler.clone();
    let payload = sample_payload("owner", "repo");

    dispatch_event(cloned, "release_pr_merged", payload)
        .await
        .expect("dispatch should succeed");

    assert_eq!(handler.received_event_types(), vec!["release_pr_merged"]);
}

#[tokio::test]
async fn test_dispatch_event_pr_comment() {
    let handler = SpyHandler::new();
    let cloned = handler.clone();
    let payload = sample_payload("owner", "repo");

    dispatch_event(cloned, "pull_request_comment_received", payload)
        .await
        .expect("dispatch should succeed");

    assert_eq!(
        handler.received_event_types(),
        vec!["pull_request_comment_received"]
    );
}

#[tokio::test]
async fn test_dispatch_event_pull_request_opened() {
    let handler = SpyHandler::new();
    let cloned = handler.clone();
    let payload = sample_payload("owner", "repo");

    dispatch_event(cloned, "pull_request_opened", payload)
        .await
        .expect("dispatch should succeed");

    assert_eq!(handler.received_event_types(), vec!["pull_request_opened"]);
}

#[tokio::test]
async fn test_dispatch_event_pull_request_updated() {
    let handler = SpyHandler::new();
    let cloned = handler.clone();
    let payload = sample_payload("owner", "repo");

    dispatch_event(cloned, "pull_request_updated", payload)
        .await
        .expect("dispatch should succeed");

    assert_eq!(handler.received_event_types(), vec!["pull_request_updated"]);
}

/// An unrecognised event type (including GitHub native names like "pull_request")
/// must be silently dropped — the handler should never be invoked.
#[tokio::test]
async fn test_dispatch_event_unknown_is_dropped() {
    let handler = SpyHandler::new();
    let cloned = handler.clone();
    let payload = sample_payload("owner", "repo");

    // "pull_request" is the GitHub X-GitHub-Event header value, not an internal
    // RR event type — it maps to Unknown and should be silently dropped.
    dispatch_event(cloned, "pull_request", payload)
        .await
        .expect("dispatch of unknown event should return Ok");

    assert_eq!(
        handler.received_count(),
        0,
        "unknown event type should be dropped without calling any handler"
    );
}

/// Verify repository metadata is correctly extracted from the payload.
#[tokio::test]
async fn test_dispatch_event_extracts_repository_metadata() {
    let handler = EventSpyHandler::new();
    let cloned = handler.clone();
    let payload = serde_json::json!({
        "repository": {
            "name": "my-repo",
            "owner": { "login": "my-org" },
            "default_branch": "develop"
        },
        "installation": { "id": 99 }
    });

    dispatch_event(cloned, "pull_request_merged", payload)
        .await
        .unwrap();

    let events = handler.events.lock().unwrap();
    assert_eq!(events[0].repository.owner, "my-org");
    assert_eq!(events[0].repository.name, "my-repo");
    assert_eq!(events[0].repository.default_branch, "develop");
    assert_eq!(events[0].installation_id, 99);
}
