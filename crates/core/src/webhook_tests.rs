use super::*;
use std::collections::HashMap;

#[test]
fn test_processing_result_match() {
    let repo_info = RepositoryInfo {
        owner: "owner".to_string(),
        name: "repo".to_string(),
        default_branch: "main".to_string(),
    };

    let pr_info = PullRequestInfo {
        number: 1,
        title: "Test".to_string(),
        body: "Body".to_string(),
        base: "main".to_string(),
        head: "feature".to_string(),
        merged: true,
        merge_commit_sha: Some("abc123".to_string()),
    };

    let result = ProcessingResult::MergedPullRequest {
        repository: repo_info,
        pull_request: pr_info,
    };

    match result {
        ProcessingResult::MergedPullRequest {
            repository,
            pull_request,
        } => {
            assert_eq!(repository.owner, "owner");
            assert_eq!(pull_request.number, 1);
        }
    }
}

#[test]
fn test_pull_request_info() {
    let pr_info = PullRequestInfo {
        number: 42,
        title: "feat: add new feature".to_string(),
        body: "This PR adds a new feature".to_string(),
        base: "main".to_string(),
        head: "feature/new-feature".to_string(),
        merged: true,
        merge_commit_sha: Some("abc123def456".to_string()),
    };

    assert_eq!(pr_info.number, 42);
    assert_eq!(pr_info.title, "feat: add new feature");
    assert!(pr_info.merged);
    assert!(pr_info.merge_commit_sha.is_some());
}

#[test]
fn test_repository_info() {
    let repo_info = RepositoryInfo {
        owner: "release-regent".to_string(),
        name: "test-repo".to_string(),
        default_branch: "main".to_string(),
    };

    assert_eq!(repo_info.owner, "release-regent");
    assert_eq!(repo_info.name, "test-repo");
    assert_eq!(repo_info.default_branch, "main");
}

#[tokio::test]
async fn test_unsupported_event_type() {
    let processor = WebhookProcessor::new(None);

    let event = WebhookEvent::new(
        "issues".to_string(),
        "opened".to_string(),
        serde_json::json!({}),
        HashMap::new(),
    );

    let result = processor.process_event(&event).await.unwrap();
    assert!(result.is_none());
}

#[test]
fn test_webhook_event_creation() {
    let mut headers = HashMap::new();
    headers.insert("x-github-event".to_string(), "pull_request".to_string());

    let payload = serde_json::json!({
        "action": "closed",
        "pull_request": {
            "number": 42,
            "merged": true
        }
    });

    let event = WebhookEvent::new(
        "pull_request".to_string(),
        "closed".to_string(),
        payload,
        headers,
    );

    assert_eq!(event.event_type(), "pull_request");
    assert_eq!(event.action(), "closed");
}

#[tokio::test]
async fn test_webhook_processor_creation() {
    let processor = WebhookProcessor::new(Some("secret123".to_string()));
    assert!(processor.webhook_secret.is_some());

    let processor_no_secret = WebhookProcessor::new(None);
    assert!(processor_no_secret.webhook_secret.is_none());
}
