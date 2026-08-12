//! Tests for [`super::NotificationDispatcher`].
//!
//! # RED-phase note
//!
//! `NotificationDispatcher::dispatch` is currently a `todo!()` skeleton (see
//! `notifications.rs`), so every test in this file is expected to **fail by
//! panicking** until a Coder implements the real dispatch logic in the GREEN
//! phase. This is the correct, intentional state for a TDD RED phase.
//!
//! # Why local inline test doubles instead of `release_regent_testing` mocks
//!
//! `crates/core/src/*_tests.rs` files are wired into the crate via
//! `#[path = "..."] mod tests;` and are therefore compiled as part of the
//! *library* compilation unit itself (the `--cfg test` unittests binary),
//! **not** as a separate crate. `release_regent_testing` depends on
//! `release-regent-core` as an external crate and is compiled against the
//! plain (non-test) library artifact. Using its mocks directly from a
//! `#[path]`-included unit-test file causes an E0277 cross-crate type-identity
//! failure — the same issue already documented in `lib_tests.rs` and solved in
//! `crates/core/tests/trait_contracts.rs` by using integration tests instead.
//! Every other `#[path]`-included test file in this crate
//! (`pr_status_commenter_tests.rs`, `comment_command_processor_tests.rs`,
//! `release_automator_tests.rs`, etc.) follows the same precedent: define
//! local inline test doubles rather than importing `release_regent_testing`
//! mocks. This file follows that established convention.
//!
//! `MockNotificationSender` was still added to `crates/testing/src/mocks/` as
//! requested, and remains available for future integration tests under
//! `crates/core/tests/`.

use super::*;
use crate::config::{
    GitHubIssueConfig, NotificationConfig, NotificationStrategy, SlackConfig, WebhookConfig,
};
use crate::traits::{
    git_operations::{
        GetCommitsOptions, GitCommit, GitOperations, GitRepository, GitTag, ListTagsOptions,
    },
    github_operations::{
        CollaboratorPermission, CreateIssueParams, CreatePullRequestParams, CreateReleaseParams,
        FileUpdate, GitHubOperations, GitUser, Issue, IssueComment, IssueState, Label,
        ListIssuesOptions, PullRequest, Release, Tag, UpdateReleaseParams,
    },
    notification_sender::NotificationSender,
};
use crate::{CoreError, CoreResult};
use async_trait::async_trait;
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

// ─────────────────────────────────────────────────────────────────────────────
// Local inline test double: GitHubOperations
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct DispatcherGitHubState {
    /// Issues visible to `list_issues` / matched against for idempotency.
    issues: Vec<Issue>,
    /// Recorded `create_issue` calls: `(owner, repo, params)`.
    create_issue_calls: Vec<(String, String, CreateIssueParams)>,
    /// Recorded `list_issues` calls: `(owner, repo, options)`.
    list_issues_calls: Vec<(String, String, ListIssuesOptions)>,
    /// Recorded `create_issue_comment` calls: `(issue_number, body)`.
    create_issue_comment_calls: Vec<(u64, String)>,
    /// When `true`, `list_issues` returns an error.
    list_issues_error: bool,
    /// When `true`, `create_issue` returns an error.
    create_issue_error: bool,
    /// Monotonically increasing counter used to synthesize new issue numbers.
    next_issue_number: u64,
}

/// Minimal in-process `GitHubOperations` double for `notifications_tests.rs`.
///
/// See the module-level doc comment above for why this is not
/// `release_regent_testing::mocks::MockGitHubOperations`.
#[derive(Clone)]
struct TestGitHubOps {
    state: Arc<Mutex<DispatcherGitHubState>>,
}

impl TestGitHubOps {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(DispatcherGitHubState::default())),
        }
    }

    fn with_issues(issues: Vec<Issue>) -> Self {
        Self {
            state: Arc::new(Mutex::new(DispatcherGitHubState {
                issues,
                ..Default::default()
            })),
        }
    }

    fn with_list_issues_error() -> Self {
        Self {
            state: Arc::new(Mutex::new(DispatcherGitHubState {
                list_issues_error: true,
                ..Default::default()
            })),
        }
    }

    fn with_create_issue_error() -> Self {
        Self {
            state: Arc::new(Mutex::new(DispatcherGitHubState {
                create_issue_error: true,
                ..Default::default()
            })),
        }
    }

    async fn create_issue_calls(&self) -> Vec<(String, String, CreateIssueParams)> {
        self.state.lock().await.create_issue_calls.clone()
    }

    async fn list_issues_calls(&self) -> Vec<(String, String, ListIssuesOptions)> {
        self.state.lock().await.list_issues_calls.clone()
    }

    async fn create_issue_comment_calls(&self) -> Vec<(u64, String)> {
        self.state.lock().await.create_issue_comment_calls.clone()
    }

    /// The exact `Issue` records synthesized by prior `create_issue` calls,
    /// including the real assigned number/labels/body — used by tests that
    /// need to feed a just-created issue into a second dispatcher instance
    /// without hardcoding the (not-yet-implemented) marker hash algorithm.
    async fn seeded_issues(&self) -> Vec<Issue> {
        self.state.lock().await.issues.clone()
    }
}

#[async_trait]
impl GitOperations for TestGitHubOps {
    async fn get_commits_between(
        &self,
        _owner: &str,
        _repo: &str,
        _base: &str,
        _head: &str,
        _options: GetCommitsOptions,
    ) -> CoreResult<Vec<GitCommit>> {
        Ok(vec![])
    }

    async fn get_commit(&self, _owner: &str, _repo: &str, _sha: &str) -> CoreResult<GitCommit> {
        Err(CoreError::not_found("stub"))
    }

    async fn list_tags(
        &self,
        _owner: &str,
        _repo: &str,
        _options: ListTagsOptions,
    ) -> CoreResult<Vec<GitTag>> {
        Ok(vec![])
    }

    async fn get_tag(&self, _owner: &str, _repo: &str, _tag_name: &str) -> CoreResult<GitTag> {
        Err(CoreError::not_found("stub"))
    }

    async fn tag_exists(&self, _owner: &str, _repo: &str, _tag_name: &str) -> CoreResult<bool> {
        Ok(false)
    }

    async fn get_head_commit(
        &self,
        _owner: &str,
        _repo: &str,
        _branch: Option<&str>,
    ) -> CoreResult<GitCommit> {
        Err(CoreError::not_found("stub"))
    }

    async fn get_repository_info(
        &self,
        _owner: &str,
        _repo: &str,
    ) -> CoreResult<GitRepository> {
        Err(CoreError::not_found("stub"))
    }
}

#[async_trait]
impl GitHubOperations for TestGitHubOps {
    async fn add_labels(
        &self,
        _owner: &str,
        _repo: &str,
        _issue_number: u64,
        _labels: &[&str],
    ) -> CoreResult<()> {
        Ok(())
    }

    async fn batch_commit_files(
        &self,
        _owner: &str,
        _repo: &str,
        _branch: &str,
        _files: &[FileUpdate],
        _message: &str,
    ) -> CoreResult<()> {
        Ok(())
    }

    async fn batch_commit_files_rebased(
        &self,
        _owner: &str,
        _repo: &str,
        _branch: &str,
        _files: &[FileUpdate],
        _message: &str,
        _parent_sha: &str,
    ) -> CoreResult<()> {
        Ok(())
    }

    async fn create_branch(
        &self,
        _owner: &str,
        _repo: &str,
        _branch_name: &str,
        _sha: &str,
    ) -> CoreResult<()> {
        Ok(())
    }

    async fn create_issue_comment(
        &self,
        _owner: &str,
        _repo: &str,
        issue_number: u64,
        body: &str,
    ) -> CoreResult<()> {
        self.state
            .lock()
            .await
            .create_issue_comment_calls
            .push((issue_number, body.to_string()));
        Ok(())
    }

    async fn create_issue(
        &self,
        owner: &str,
        repo: &str,
        params: CreateIssueParams,
    ) -> CoreResult<Issue> {
        let mut state = self.state.lock().await;
        state
            .create_issue_calls
            .push((owner.to_string(), repo.to_string(), params.clone()));

        if state.create_issue_error {
            return Err(CoreError::network("simulated create_issue failure"));
        }

        state.next_issue_number += 1;
        let number = 1000 + state.next_issue_number;
        let issue = Issue {
            number,
            title: params.title.clone(),
            body: params.body.clone(),
            state: "open".to_string(),
            labels: params
                .labels
                .iter()
                .map(|name| Label {
                    id: 1,
                    name: name.clone(),
                    color: "ededed".to_string(),
                    description: None,
                })
                .collect(),
            html_url: format!("https://github.com/{owner}/{repo}/issues/{number}"),
        };
        state.issues.push(issue.clone());
        Ok(issue)
    }

    async fn create_pull_request(
        &self,
        _owner: &str,
        _repo: &str,
        _params: CreatePullRequestParams,
    ) -> CoreResult<PullRequest> {
        Err(CoreError::not_found("stub"))
    }

    async fn create_release(
        &self,
        _owner: &str,
        _repo: &str,
        _params: CreateReleaseParams,
    ) -> CoreResult<Release> {
        Err(CoreError::not_found("stub"))
    }

    async fn create_tag(
        &self,
        _owner: &str,
        _repo: &str,
        _tag_name: &str,
        _commit_sha: &str,
        _message: Option<String>,
        _tagger: Option<GitUser>,
    ) -> CoreResult<Tag> {
        Err(CoreError::not_found("stub"))
    }

    async fn delete_branch(&self, _owner: &str, _repo: &str, _branch_name: &str) -> CoreResult<()> {
        Ok(())
    }

    async fn force_update_branch(
        &self,
        _owner: &str,
        _repo: &str,
        _branch_name: &str,
        _sha: &str,
    ) -> CoreResult<()> {
        Ok(())
    }

    async fn get_collaborator_permission(
        &self,
        _owner: &str,
        _repo: &str,
        _username: &str,
    ) -> CoreResult<CollaboratorPermission> {
        Ok(CollaboratorPermission::Write)
    }

    async fn get_file_content(
        &self,
        _owner: &str,
        _repo: &str,
        _path: &str,
        _branch: &str,
    ) -> CoreResult<Option<String>> {
        Ok(None)
    }

    async fn get_installation_id_for_repo(&self, _owner: &str, _repo: &str) -> CoreResult<u64> {
        Ok(0)
    }

    async fn get_latest_release(&self, _owner: &str, _repo: &str) -> CoreResult<Option<Release>> {
        Ok(None)
    }

    async fn get_pull_request(
        &self,
        _owner: &str,
        _repo: &str,
        _pr_number: u64,
    ) -> CoreResult<PullRequest> {
        Err(CoreError::not_found("stub"))
    }

    async fn get_release_by_tag(&self, _owner: &str, _repo: &str, _tag: &str) -> CoreResult<Release> {
        Err(CoreError::not_found("stub"))
    }

    async fn list_issue_comments(
        &self,
        _owner: &str,
        _repo: &str,
        _issue_number: u64,
    ) -> CoreResult<Vec<IssueComment>> {
        Ok(vec![])
    }

    async fn list_issues(
        &self,
        owner: &str,
        repo: &str,
        options: ListIssuesOptions,
    ) -> CoreResult<Vec<Issue>> {
        let mut state = self.state.lock().await;
        state
            .list_issues_calls
            .push((owner.to_string(), repo.to_string(), options.clone()));

        if state.list_issues_error {
            return Err(CoreError::network("simulated list_issues failure"));
        }

        let filtered = state
            .issues
            .iter()
            .filter(|issue| match options.state {
                IssueState::Open => issue.state == "open",
                IssueState::Closed => issue.state == "closed",
                IssueState::All => true,
            })
            .filter(|issue| {
                options.labels.is_empty()
                    || options
                        .labels
                        .iter()
                        .all(|wanted| issue.labels.iter().any(|l| &l.name == wanted))
            })
            .cloned()
            .collect();
        Ok(filtered)
    }

    async fn list_pr_labels(
        &self,
        _owner: &str,
        _repo: &str,
        _issue_number: u64,
    ) -> CoreResult<Vec<Label>> {
        Ok(vec![])
    }

    #[allow(clippy::too_many_arguments)]
    async fn list_pull_requests(
        &self,
        _owner: &str,
        _repo: &str,
        _state: Option<&str>,
        _head: Option<&str>,
        _base: Option<&str>,
        _per_page: Option<u8>,
        _page: Option<u32>,
    ) -> CoreResult<Vec<PullRequest>> {
        Ok(vec![])
    }

    async fn list_releases(
        &self,
        _owner: &str,
        _repo: &str,
        _per_page: Option<u8>,
        _page: Option<u32>,
    ) -> CoreResult<Vec<Release>> {
        Ok(vec![])
    }

    async fn remove_label(
        &self,
        _owner: &str,
        _repo: &str,
        _issue_number: u64,
        _label_name: &str,
    ) -> CoreResult<()> {
        Ok(())
    }

    async fn search_pull_requests(
        &self,
        _owner: &str,
        _repo: &str,
        _query: &str,
    ) -> CoreResult<Vec<PullRequest>> {
        Ok(vec![])
    }

    async fn update_issue_comment(
        &self,
        _owner: &str,
        _repo: &str,
        _comment_id: u64,
        _body: &str,
    ) -> CoreResult<()> {
        Ok(())
    }

    async fn update_pull_request(
        &self,
        _owner: &str,
        _repo: &str,
        _pr_number: u64,
        _title: Option<String>,
        _body: Option<String>,
        _state: Option<String>,
    ) -> CoreResult<PullRequest> {
        Err(CoreError::not_found("stub"))
    }

    async fn update_release(
        &self,
        _owner: &str,
        _repo: &str,
        _release_id: u64,
        _params: UpdateReleaseParams,
    ) -> CoreResult<Release> {
        Err(CoreError::not_found("stub"))
    }

    async fn upsert_file(
        &self,
        _owner: &str,
        _repo: &str,
        _path: &str,
        _commit_message: &str,
        _content: &str,
        _branch: &str,
    ) -> CoreResult<()> {
        Ok(())
    }

    fn scoped_to(&self, _installation_id: u64) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Local inline test double: NotificationSender
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct DispatcherSenderState {
    /// Recorded `send_webhook` calls: `(url, headers, payload)`.
    webhook_calls: Vec<(String, HashMap<String, String>, serde_json::Value)>,
    /// Recorded `send_slack` calls: `(webhook_url, channel, text)`.
    slack_calls: Vec<(String, Option<String>, String)>,
    /// When `Some`, `send_webhook` returns this error message.
    webhook_error: Option<String>,
    /// When `Some`, `send_slack` returns this error message.
    slack_error: Option<String>,
}

/// Minimal in-process `NotificationSender` double for `notifications_tests.rs`.
///
/// See the module-level doc comment above for why this is not
/// `release_regent_testing::mocks::MockNotificationSender`.
#[derive(Clone)]
struct TestNotificationSender {
    state: Arc<Mutex<DispatcherSenderState>>,
}

impl TestNotificationSender {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(DispatcherSenderState::default())),
        }
    }

    fn with_send_webhook_error(message: &str) -> Self {
        Self {
            state: Arc::new(Mutex::new(DispatcherSenderState {
                webhook_error: Some(message.to_string()),
                ..Default::default()
            })),
        }
    }

    fn with_send_slack_error(message: &str) -> Self {
        Self {
            state: Arc::new(Mutex::new(DispatcherSenderState {
                slack_error: Some(message.to_string()),
                ..Default::default()
            })),
        }
    }

    async fn webhook_calls(&self) -> Vec<(String, HashMap<String, String>, serde_json::Value)> {
        self.state.lock().await.webhook_calls.clone()
    }

    async fn slack_calls(&self) -> Vec<(String, Option<String>, String)> {
        self.state.lock().await.slack_calls.clone()
    }
}

#[async_trait]
impl NotificationSender for TestNotificationSender {
    async fn send_webhook(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        payload: &serde_json::Value,
    ) -> CoreResult<()> {
        let mut state = self.state.lock().await;
        if let Some(msg) = state.webhook_error.clone() {
            return Err(CoreError::webhook("webhook", msg));
        }
        state
            .webhook_calls
            .push((url.to_string(), headers.clone(), payload.clone()));
        Ok(())
    }

    async fn send_slack(
        &self,
        webhook_url: &str,
        channel: Option<&str>,
        text: &str,
    ) -> CoreResult<()> {
        let mut state = self.state.lock().await;
        if let Some(msg) = state.slack_error.clone() {
            return Err(CoreError::webhook("slack", msg));
        }
        state
            .slack_calls
            .push((webhook_url.to_string(), channel.map(str::to_string), text.to_string()));
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Fixtures
// ─────────────────────────────────────────────────────────────────────────────

fn sample_event(kind: NotificationEventKind, summary: &str) -> NotificationEvent {
    NotificationEvent {
        kind,
        owner: "acme".to_string(),
        repo: "widgets".to_string(),
        correlation_id: "corr-123".to_string(),
        summary: summary.to_string(),
        details: "Detailed failure information for operators.".to_string(),
    }
}

fn github_issue_config(enabled: bool) -> NotificationConfig {
    NotificationConfig {
        enabled,
        strategy: NotificationStrategy::GitHubIssue,
        github_issue: Some(GitHubIssueConfig {
            labels: vec!["release-regent".to_string(), "bug".to_string()],
            assignees: vec!["on-call-team".to_string()],
        }),
        webhook: None,
        slack: None,
    }
}

fn webhook_config(enabled: bool, with_webhook: bool) -> NotificationConfig {
    let mut headers = HashMap::new();
    headers.insert("X-Api-Key".to_string(), "secret".to_string());
    NotificationConfig {
        enabled,
        strategy: NotificationStrategy::Webhook,
        github_issue: None,
        webhook: with_webhook.then_some(WebhookConfig {
            url: "https://example.com/hooks/rr".to_string(),
            headers,
        }),
        slack: None,
    }
}

fn slack_config(enabled: bool, with_slack: bool) -> NotificationConfig {
    NotificationConfig {
        enabled,
        strategy: NotificationStrategy::Slack,
        github_issue: None,
        webhook: None,
        slack: with_slack.then_some(SlackConfig {
            webhook_url: "https://hooks.slack.com/services/x".to_string(),
            channel: Some("#releases".to_string()),
        }),
    }
}

fn none_strategy_config(enabled: bool) -> NotificationConfig {
    NotificationConfig {
        enabled,
        strategy: NotificationStrategy::None,
        github_issue: None,
        webhook: None,
        slack: None,
    }
}

fn marker_regex() -> Regex {
    Regex::new(r"<!-- release-regent:notification:[0-9a-f]{64} -->").expect("valid regex literal")
}

fn marker_from_body(body: &str) -> String {
    let re =
        Regex::new(r"<!-- release-regent:notification:([0-9a-f]{64}) -->").expect("valid regex");
    re.captures(body)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default()
}

// ─────────────────────────────────────────────────────────────────────────────
// Scenario 1 — `enabled == false` => `Ok(())`, zero calls, for every strategy
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_dispatch_disabled_github_issue_strategy_returns_ok() {
    let github = TestGitHubOps::new();
    let sender = TestNotificationSender::new();
    let dispatcher = NotificationDispatcher::new(&github, &sender);
    let mut config = github_issue_config(true);
    config.enabled = false;
    let event = sample_event(NotificationEventKind::ProcessingError, "boom");

    let result = dispatcher.dispatch(&config, &event).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_dispatch_disabled_github_issue_strategy_zero_calls() {
    let github = TestGitHubOps::new();
    let sender = TestNotificationSender::new();
    let dispatcher = NotificationDispatcher::new(&github, &sender);
    let mut config = github_issue_config(true);
    config.enabled = false;
    let event = sample_event(NotificationEventKind::ProcessingError, "boom");

    let _ = dispatcher.dispatch(&config, &event).await;

    assert!(github.create_issue_calls().await.is_empty());
    assert!(github.list_issues_calls().await.is_empty());
    assert!(sender.webhook_calls().await.is_empty());
    assert!(sender.slack_calls().await.is_empty());
}

#[tokio::test]
async fn test_dispatch_disabled_webhook_strategy_zero_calls() {
    let github = TestGitHubOps::new();
    let sender = TestNotificationSender::new();
    let dispatcher = NotificationDispatcher::new(&github, &sender);
    let mut config = webhook_config(true, true);
    config.enabled = false;
    let event = sample_event(NotificationEventKind::ProcessingError, "boom");

    let result = dispatcher.dispatch(&config, &event).await;

    assert!(result.is_ok());
    assert!(sender.webhook_calls().await.is_empty());
    assert!(github.create_issue_calls().await.is_empty());
}

#[tokio::test]
async fn test_dispatch_disabled_slack_strategy_zero_calls() {
    let github = TestGitHubOps::new();
    let sender = TestNotificationSender::new();
    let dispatcher = NotificationDispatcher::new(&github, &sender);
    let mut config = slack_config(true, true);
    config.enabled = false;
    let event = sample_event(NotificationEventKind::ProcessingError, "boom");

    let result = dispatcher.dispatch(&config, &event).await;

    assert!(result.is_ok());
    assert!(sender.slack_calls().await.is_empty());
    assert!(github.create_issue_calls().await.is_empty());
}

#[tokio::test]
async fn test_dispatch_disabled_none_strategy_zero_calls() {
    let github = TestGitHubOps::new();
    let sender = TestNotificationSender::new();
    let dispatcher = NotificationDispatcher::new(&github, &sender);
    let config = none_strategy_config(false);
    let event = sample_event(NotificationEventKind::ProcessingError, "boom");

    let result = dispatcher.dispatch(&config, &event).await;

    assert!(result.is_ok());
    assert!(github.create_issue_calls().await.is_empty());
    assert!(sender.webhook_calls().await.is_empty());
    assert!(sender.slack_calls().await.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// Scenario 2 — `strategy == None`, `enabled == true` => `Ok(())`, zero calls
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_dispatch_none_strategy_enabled_returns_ok_zero_calls() {
    let github = TestGitHubOps::new();
    let sender = TestNotificationSender::new();
    let dispatcher = NotificationDispatcher::new(&github, &sender);
    let config = none_strategy_config(true);
    let event = sample_event(NotificationEventKind::ReleaseCreated, "release created");

    let result = dispatcher.dispatch(&config, &event).await;

    assert!(result.is_ok());
    assert!(github.create_issue_calls().await.is_empty());
    assert!(github.list_issues_calls().await.is_empty());
    assert!(sender.webhook_calls().await.is_empty());
    assert!(sender.slack_calls().await.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// Scenario 3 — GitHubIssue, no existing matching issue
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_dispatch_github_issue_no_existing_issue_lists_open_issues_with_configured_labels() {
    let github = TestGitHubOps::new();
    let sender = TestNotificationSender::new();
    let dispatcher = NotificationDispatcher::new(&github, &sender);
    let config = github_issue_config(true);
    let event = sample_event(NotificationEventKind::ProcessingError, "boom");

    dispatcher
        .dispatch(&config, &event)
        .await
        .expect("dispatch should succeed");

    let calls = github.list_issues_calls().await;
    assert_eq!(calls.len(), 1);
    let (_, _, options) = &calls[0];
    assert_eq!(options.state, IssueState::Open);
    assert_eq!(options.labels, config.github_issue.as_ref().unwrap().labels);
}

#[tokio::test]
async fn test_dispatch_github_issue_no_existing_issue_creates_issue_with_configured_labels_and_assignees(
) {
    let github = TestGitHubOps::new();
    let sender = TestNotificationSender::new();
    let dispatcher = NotificationDispatcher::new(&github, &sender);
    let config = github_issue_config(true);
    let event = sample_event(NotificationEventKind::ProcessingError, "boom");

    dispatcher
        .dispatch(&config, &event)
        .await
        .expect("dispatch should succeed");

    let calls = github.create_issue_calls().await;
    assert_eq!(calls.len(), 1);
    let (_, _, params) = &calls[0];
    let expected = config.github_issue.as_ref().unwrap();
    assert_eq!(params.labels, expected.labels);
    assert_eq!(params.assignees, expected.assignees);
}

#[tokio::test]
async fn test_dispatch_github_issue_no_existing_issue_body_contains_marker_and_details() {
    let github = TestGitHubOps::new();
    let sender = TestNotificationSender::new();
    let dispatcher = NotificationDispatcher::new(&github, &sender);
    let config = github_issue_config(true);
    let event = sample_event(NotificationEventKind::ProcessingError, "boom");

    dispatcher
        .dispatch(&config, &event)
        .await
        .expect("dispatch should succeed");

    let calls = github.create_issue_calls().await;
    let body = calls[0].2.body.clone().unwrap_or_default();
    assert!(
        marker_regex().is_match(&body),
        "body should contain the hidden idempotency marker: {body}"
    );
    assert!(
        body.contains(&event.details),
        "body should contain the details text: {body}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Scenario 4 — existing OPEN issue already carries the exact marker
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_dispatch_github_issue_existing_matching_marker_comments_instead_of_creating() {
    let config = github_issue_config(true);
    let event = sample_event(NotificationEventKind::ProcessingError, "recurring failure");

    // First dispatch establishes the canonical issue (and its real marker).
    let github_seed = TestGitHubOps::new();
    let sender_seed = TestNotificationSender::new();
    NotificationDispatcher::new(&github_seed, &sender_seed)
        .dispatch(&config, &event)
        .await
        .expect("seed dispatch should succeed");
    let existing_issues = github_seed.seeded_issues().await;
    assert_eq!(existing_issues.len(), 1);

    // Re-seed a fresh double with that exact (open) issue and dispatch again
    // with the identical event.
    let github = TestGitHubOps::with_issues(existing_issues.clone());
    let sender = TestNotificationSender::new();
    let dispatcher = NotificationDispatcher::new(&github, &sender);

    dispatcher
        .dispatch(&config, &event)
        .await
        .expect("second dispatch should succeed");

    assert!(
        github.create_issue_calls().await.is_empty(),
        "must not create a duplicate issue"
    );
    let comment_calls = github.create_issue_comment_calls().await;
    assert_eq!(comment_calls.len(), 1);
    assert_eq!(comment_calls[0].0, existing_issues[0].number);
    assert!(comment_calls[0].1.contains(&event.details));
}

// ─────────────────────────────────────────────────────────────────────────────
// Scenario 5 — existing OPEN issue carries a DIFFERENT marker
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_dispatch_github_issue_existing_issue_with_different_marker_creates_new_issue() {
    let config = github_issue_config(true);
    let event_a = sample_event(NotificationEventKind::ProcessingError, "failure A");
    let event_b = sample_event(NotificationEventKind::ProcessingError, "failure B");

    // Seed an existing OPEN issue carrying event_a's marker.
    let github_seed = TestGitHubOps::new();
    let sender_seed = TestNotificationSender::new();
    NotificationDispatcher::new(&github_seed, &sender_seed)
        .dispatch(&config, &event_a)
        .await
        .expect("seed dispatch should succeed");
    let existing_issues = github_seed.seeded_issues().await;

    let github = TestGitHubOps::with_issues(existing_issues);
    let sender = TestNotificationSender::new();
    let dispatcher = NotificationDispatcher::new(&github, &sender);

    dispatcher
        .dispatch(&config, &event_b)
        .await
        .expect("dispatch with different summary should succeed");

    assert_eq!(
        github.create_issue_calls().await.len(),
        1,
        "different marker must not be treated as a duplicate"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Scenario 6 — `list_issues` / `create_issue` error propagation
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_dispatch_github_issue_list_issues_error_propagates() {
    let github = TestGitHubOps::with_list_issues_error();
    let sender = TestNotificationSender::new();
    let dispatcher = NotificationDispatcher::new(&github, &sender);
    let config = github_issue_config(true);
    let event = sample_event(NotificationEventKind::ProcessingError, "boom");

    let result = dispatcher.dispatch(&config, &event).await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), CoreError::Network { .. }));
}

#[tokio::test]
async fn test_dispatch_github_issue_create_issue_error_propagates() {
    let github = TestGitHubOps::with_create_issue_error();
    let sender = TestNotificationSender::new();
    let dispatcher = NotificationDispatcher::new(&github, &sender);
    let config = github_issue_config(true);
    let event = sample_event(NotificationEventKind::ProcessingError, "boom");

    let result = dispatcher.dispatch(&config, &event).await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), CoreError::Network { .. }));
}

// ─────────────────────────────────────────────────────────────────────────────
// Scenario 7 — Webhook strategy
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_dispatch_webhook_sends_to_configured_url_and_headers() {
    let github = TestGitHubOps::new();
    let sender = TestNotificationSender::new();
    let dispatcher = NotificationDispatcher::new(&github, &sender);
    let config = webhook_config(true, true);
    let event = sample_event(NotificationEventKind::ReleaseCreated, "Release v1.2.3 created");

    dispatcher
        .dispatch(&config, &event)
        .await
        .expect("webhook dispatch should succeed");

    let calls = sender.webhook_calls().await;
    assert_eq!(calls.len(), 1);
    let expected = config.webhook.as_ref().unwrap();
    assert_eq!(calls[0].0, expected.url);
    assert_eq!(calls[0].1, expected.headers);
}

#[tokio::test]
async fn test_dispatch_webhook_payload_contains_expected_json_fields() {
    let github = TestGitHubOps::new();
    let sender = TestNotificationSender::new();
    let dispatcher = NotificationDispatcher::new(&github, &sender);
    let config = webhook_config(true, true);
    let event = sample_event(NotificationEventKind::ReleaseCreated, "Release v1.2.3 created");

    dispatcher
        .dispatch(&config, &event)
        .await
        .expect("webhook dispatch should succeed");

    let calls = sender.webhook_calls().await;
    let payload = &calls[0].2;
    let obj = payload.as_object().expect("payload should be a JSON object");
    assert_eq!(obj.get("owner").and_then(|v| v.as_str()), Some(event.owner.as_str()));
    assert_eq!(obj.get("repo").and_then(|v| v.as_str()), Some(event.repo.as_str()));
    assert_eq!(
        obj.get("correlation_id").and_then(|v| v.as_str()),
        Some(event.correlation_id.as_str())
    );
    assert_eq!(obj.get("summary").and_then(|v| v.as_str()), Some(event.summary.as_str()));
    assert_eq!(obj.get("details").and_then(|v| v.as_str()), Some(event.details.as_str()));
    assert!(obj.contains_key("event"), "payload should contain an 'event' field");
    let timestamp = obj
        .get("timestamp")
        .and_then(|v| v.as_str())
        .expect("payload should contain a string 'timestamp' field");
    chrono::DateTime::parse_from_rfc3339(timestamp).expect("timestamp should be RFC3339");
}

#[tokio::test]
async fn test_dispatch_webhook_missing_config_returns_config_error() {
    let github = TestGitHubOps::new();
    let sender = TestNotificationSender::new();
    let dispatcher = NotificationDispatcher::new(&github, &sender);
    let config = webhook_config(true, false);
    let event = sample_event(NotificationEventKind::ProcessingError, "boom");

    let result = dispatcher.dispatch(&config, &event).await;

    assert!(matches!(result, Err(CoreError::Config { .. })));
}

#[tokio::test]
async fn test_dispatch_webhook_missing_config_zero_calls() {
    let github = TestGitHubOps::new();
    let sender = TestNotificationSender::new();
    let dispatcher = NotificationDispatcher::new(&github, &sender);
    let config = webhook_config(true, false);
    let event = sample_event(NotificationEventKind::ProcessingError, "boom");

    let _ = dispatcher.dispatch(&config, &event).await;

    assert!(sender.webhook_calls().await.is_empty());
    assert!(sender.slack_calls().await.is_empty());
    assert!(github.create_issue_calls().await.is_empty());
}

#[tokio::test]
async fn test_dispatch_webhook_send_error_propagates() {
    let github = TestGitHubOps::new();
    let sender = TestNotificationSender::with_send_webhook_error("simulated webhook failure");
    let dispatcher = NotificationDispatcher::new(&github, &sender);
    let config = webhook_config(true, true);
    let event = sample_event(NotificationEventKind::ProcessingError, "boom");

    let result = dispatcher.dispatch(&config, &event).await;

    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// Scenario 10-12 — Slack strategy
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_dispatch_slack_sends_formatted_text_to_configured_webhook() {
    let github = TestGitHubOps::new();
    let sender = TestNotificationSender::new();
    let dispatcher = NotificationDispatcher::new(&github, &sender);
    let config = slack_config(true, true);
    let event = sample_event(NotificationEventKind::ProcessingError, "boom");

    dispatcher
        .dispatch(&config, &event)
        .await
        .expect("slack dispatch should succeed");

    let calls = sender.slack_calls().await;
    assert_eq!(calls.len(), 1);
    let expected = config.slack.as_ref().unwrap();
    assert_eq!(calls[0].0, expected.webhook_url);
    assert_eq!(calls[0].1.as_deref(), expected.channel.as_deref());
    assert_eq!(calls[0].2, format!("*{}*\n{}", event.summary, event.details));
}

#[tokio::test]
async fn test_dispatch_slack_missing_config_returns_config_error() {
    let github = TestGitHubOps::new();
    let sender = TestNotificationSender::new();
    let dispatcher = NotificationDispatcher::new(&github, &sender);
    let config = slack_config(true, false);
    let event = sample_event(NotificationEventKind::ProcessingError, "boom");

    let result = dispatcher.dispatch(&config, &event).await;

    assert!(matches!(result, Err(CoreError::Config { .. })));
}

#[tokio::test]
async fn test_dispatch_slack_missing_config_zero_calls() {
    let github = TestGitHubOps::new();
    let sender = TestNotificationSender::new();
    let dispatcher = NotificationDispatcher::new(&github, &sender);
    let config = slack_config(true, false);
    let event = sample_event(NotificationEventKind::ProcessingError, "boom");

    let _ = dispatcher.dispatch(&config, &event).await;

    assert!(sender.slack_calls().await.is_empty());
    assert!(sender.webhook_calls().await.is_empty());
    assert!(github.create_issue_calls().await.is_empty());
}

#[tokio::test]
async fn test_dispatch_slack_send_error_propagates() {
    let github = TestGitHubOps::new();
    let sender = TestNotificationSender::with_send_slack_error("simulated slack failure");
    let dispatcher = NotificationDispatcher::new(&github, &sender);
    let config = slack_config(true, true);
    let event = sample_event(NotificationEventKind::ProcessingError, "boom");

    let result = dispatcher.dispatch(&config, &event).await;

    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// Tier 3 — Property-based tests (scenario 13)
// ─────────────────────────────────────────────────────────────────────────────

mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// The idempotency marker embedded in the created issue body is a
        /// deterministic function of `(owner, repo, kind, summary)`: dispatching
        /// the identical event twice (against two independent, unseeded
        /// doubles) must produce the same marker both times.
        #[test]
        fn prop_marker_stable_for_identical_inputs(
            owner in "[a-z]{1,10}",
            repo in "[a-z]{1,10}",
            summary in "[a-zA-Z0-9 ]{1,30}",
        ) {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .expect("failed to build test runtime");

            rt.block_on(async move {
                let config = github_issue_config(true);
                let event = NotificationEvent {
                    kind: NotificationEventKind::ProcessingError,
                    owner: owner.clone(),
                    repo: repo.clone(),
                    correlation_id: "corr".to_string(),
                    summary: summary.clone(),
                    details: "details".to_string(),
                };

                let github_a = TestGitHubOps::new();
                let sender_a = TestNotificationSender::new();
                NotificationDispatcher::new(&github_a, &sender_a)
                    .dispatch(&config, &event)
                    .await
                    .expect("dispatch should succeed");
                let calls_a = github_a.create_issue_calls().await;
                prop_assert_eq!(calls_a.len(), 1);
                let marker_a = marker_from_body(&calls_a[0].2.body.clone().unwrap_or_default());

                let github_b = TestGitHubOps::new();
                let sender_b = TestNotificationSender::new();
                NotificationDispatcher::new(&github_b, &sender_b)
                    .dispatch(&config, &event)
                    .await
                    .expect("dispatch should succeed");
                let calls_b = github_b.create_issue_calls().await;
                prop_assert_eq!(calls_b.len(), 1);
                let marker_b = marker_from_body(&calls_b[0].2.body.clone().unwrap_or_default());

                prop_assert_eq!(marker_a, marker_b);
                Ok(())
            })?;
        }

        /// Changing `summary` must change the resulting marker (with
        /// overwhelming probability) — otherwise unrelated recurring errors
        /// would collapse into the same GitHub issue.
        #[test]
        fn prop_marker_differs_when_summary_differs(
            owner in "[a-z]{1,10}",
            repo in "[a-z]{1,10}",
            summary_a in "[a-zA-Z0-9 ]{1,30}",
            summary_b in "[a-zA-Z0-9 ]{1,30}",
        ) {
            prop_assume!(summary_a != summary_b);

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .expect("failed to build test runtime");

            rt.block_on(async move {
                let config = github_issue_config(true);

                let event_a = NotificationEvent {
                    kind: NotificationEventKind::ProcessingError,
                    owner: owner.clone(),
                    repo: repo.clone(),
                    correlation_id: "corr".to_string(),
                    summary: summary_a,
                    details: "details".to_string(),
                };
                let github_a = TestGitHubOps::new();
                let sender_a = TestNotificationSender::new();
                NotificationDispatcher::new(&github_a, &sender_a)
                    .dispatch(&config, &event_a)
                    .await
                    .expect("dispatch should succeed");
                let calls_a = github_a.create_issue_calls().await;
                prop_assert_eq!(calls_a.len(), 1);
                let marker_a = marker_from_body(&calls_a[0].2.body.clone().unwrap_or_default());

                let event_b = NotificationEvent {
                    kind: NotificationEventKind::ProcessingError,
                    owner,
                    repo,
                    correlation_id: "corr".to_string(),
                    summary: summary_b,
                    details: "details".to_string(),
                };
                let github_b = TestGitHubOps::new();
                let sender_b = TestNotificationSender::new();
                NotificationDispatcher::new(&github_b, &sender_b)
                    .dispatch(&config, &event_b)
                    .await
                    .expect("dispatch should succeed");
                let calls_b = github_b.create_issue_calls().await;
                prop_assert_eq!(calls_b.len(), 1);
                let marker_b = marker_from_body(&calls_b[0].2.body.clone().unwrap_or_default());

                prop_assert_ne!(marker_a, marker_b);
                Ok(())
            })?;
        }
    }
}
