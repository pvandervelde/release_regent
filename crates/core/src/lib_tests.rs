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
// Inline test doubles (avoids cross-crate type identity issues)
// ─────────────────────────────────────────────────────────────────────────────

/// No-op `MergedPullRequestHandler` for loop-infrastructure tests that do not
/// exercise the processor itself.  Always returns `Ok(())`.
struct NoopMergedPRHandler;

#[async_trait]
impl MergedPullRequestHandler for NoopMergedPRHandler {
    async fn handle_merged_pull_request(&self, _event: &ProcessingEvent) -> CoreResult<()> {
        Ok(())
    }
}

/// Spy `MergedPullRequestHandler` that records every event it receives so
/// tests can verify the loop actually invokes the handler.
#[derive(Clone, Default)]
struct SpyMergedPRHandler {
    received: Arc<Mutex<Vec<String>>>,
    received_release_pr: Arc<Mutex<Vec<String>>>,
    received_activity: Arc<Mutex<Vec<String>>>,
}

impl SpyMergedPRHandler {
    fn new() -> Self {
        Self::default()
    }

    async fn received_event_ids(&self) -> Vec<String> {
        self.received.lock().await.clone()
    }

    async fn received_release_pr_event_ids(&self) -> Vec<String> {
        self.received_release_pr.lock().await.clone()
    }

    async fn received_activity_event_ids(&self) -> Vec<String> {
        self.received_activity.lock().await.clone()
    }
}

#[async_trait]
impl MergedPullRequestHandler for SpyMergedPRHandler {
    async fn handle_merged_pull_request(&self, event: &ProcessingEvent) -> CoreResult<()> {
        self.received.lock().await.push(event.event_id.clone());
        Ok(())
    }

    async fn handle_release_pr_merged(&self, event: &ProcessingEvent) -> CoreResult<()> {
        self.received_release_pr
            .lock()
            .await
            .push(event.event_id.clone());
        Ok(())
    }

    async fn handle_pull_request_activity(&self, event: &ProcessingEvent) -> CoreResult<()> {
        self.received_activity
            .lock()
            .await
            .push(event.event_id.clone());
        Ok(())
    }
}

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
        installation_id: 0,
    }
}

/// Poll `acknowledged_ids()` until it contains at least `expected_count`
/// entries, or the deadline expires.  Cancels `token` once done so the loop
/// under test exits.  Returns the final acknowledged list.
async fn wait_for_acks(
    source: &TestEventSource,
    expected_count: usize,
    token: &CancellationToken,
) -> Vec<String> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let acked = source.acknowledged_ids().await;
        if acked.len() >= expected_count {
            token.cancel();
            return acked;
        }
        if tokio::time::Instant::now() >= deadline {
            token.cancel();
            return acked;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ReleaseRegent smoke tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_release_regent_creation() {
    let config = config::ReleaseRegentConfig::default();
    let regent = ReleaseRegent::new(config);

    assert_eq!(regent.config().core.version_prefix, "v");
    assert_eq!(regent.config().core.branches.main, "main");
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

    let result = run_event_loop(&source, &NoopMergedPRHandler, token).await;
    assert!(result.is_ok());
    // Event was never consumed because the token was already cancelled.
    assert_eq!(source.remaining_count().await, 1);
    assert!(source.acknowledged_ids().await.is_empty());
}

/// A single `PullRequestMerged` event is processed and acknowledged.
#[tokio::test]
async fn test_run_event_loop_acknowledges_pull_request_merged_event() {
    let token = CancellationToken::new();
    let source = TestEventSource::new(vec![make_test_event(
        "evt-pr-1",
        EventType::PullRequestMerged,
    )]);
    let source_for_loop = source.clone();
    let loop_token = token.clone();

    let loop_handle = tokio::spawn(async move {
        run_event_loop(&source_for_loop, &NoopMergedPRHandler, loop_token).await
    });

    let acked = wait_for_acks(&source, 1, &token).await;
    loop_handle.await.unwrap().unwrap();
    assert_eq!(acked, vec!["evt-pr-1"]);
    assert!(source.rejected_ids().await.is_empty());
}

/// A single `ReleasePrMerged` event is processed and acknowledged.
#[tokio::test]
async fn test_run_event_loop_acknowledges_release_pr_merged_event() {
    let token = CancellationToken::new();
    let source = TestEventSource::new(vec![make_test_event(
        "evt-rel-1",
        EventType::ReleasePrMerged,
    )]);
    let source_for_loop = source.clone();
    let loop_token = token.clone();

    let loop_handle = tokio::spawn(async move {
        run_event_loop(&source_for_loop, &NoopMergedPRHandler, loop_token).await
    });

    let acked = wait_for_acks(&source, 1, &token).await;
    loop_handle.await.unwrap().unwrap();
    assert_eq!(acked, vec!["evt-rel-1"]);
    assert!(source.rejected_ids().await.is_empty());
}

/// `PullRequestCommentReceived` events are acknowledged.
#[tokio::test]
async fn test_run_event_loop_acknowledges_pr_comment_event() {
    let token = CancellationToken::new();
    let source = TestEventSource::new(vec![make_test_event(
        "evt-comment-1",
        EventType::PullRequestCommentReceived,
    )]);
    let source_for_loop = source.clone();
    let loop_token = token.clone();

    let loop_handle = tokio::spawn(async move {
        run_event_loop(&source_for_loop, &NoopMergedPRHandler, loop_token).await
    });

    let acked = wait_for_acks(&source, 1, &token).await;
    loop_handle.await.unwrap().unwrap();
    assert_eq!(acked, vec!["evt-comment-1"]);
    assert!(source.rejected_ids().await.is_empty());
}

/// Unknown event types are acknowledged (logged-and-dropped, not errors).
#[tokio::test]
async fn test_run_event_loop_acknowledges_unknown_event_type() {
    let token = CancellationToken::new();
    let source = TestEventSource::new(vec![make_test_event(
        "evt-unknown-1",
        EventType::Unknown("novel_event".to_string()),
    )]);
    let source_for_loop = source.clone();
    let loop_token = token.clone();

    let loop_handle = tokio::spawn(async move {
        run_event_loop(&source_for_loop, &NoopMergedPRHandler, loop_token).await
    });

    let acked = wait_for_acks(&source, 1, &token).await;
    loop_handle.await.unwrap().unwrap();
    assert_eq!(acked, vec!["evt-unknown-1"]);
    assert!(source.rejected_ids().await.is_empty());
}

/// Multiple events are processed in FIFO order and all acknowledged.
#[tokio::test]
async fn test_run_event_loop_processes_multiple_events_in_order() {
    let token = CancellationToken::new();
    let source = TestEventSource::new(vec![
        make_test_event("evt-a", EventType::PullRequestMerged),
        make_test_event("evt-b", EventType::ReleasePrMerged),
        make_test_event("evt-c", EventType::PullRequestMerged),
    ]);
    let source_for_loop = source.clone();
    let loop_token = token.clone();

    let loop_handle = tokio::spawn(async move {
        run_event_loop(&source_for_loop, &NoopMergedPRHandler, loop_token).await
    });

    let acked = wait_for_acks(&source, 3, &token).await;
    loop_handle.await.unwrap().unwrap();
    assert_eq!(acked, vec!["evt-a", "evt-b", "evt-c"]);
    assert!(source.rejected_ids().await.is_empty());
}

/// A transient source error is logged and the loop continues; the event
/// that follows the error is still processed and acknowledged.
#[tokio::test]
async fn test_run_event_loop_continues_after_source_error() {
    let token = CancellationToken::new();
    let source = TestEventSource::new(vec![make_test_event(
        "evt-after-err",
        EventType::PullRequestMerged,
    )]);
    source.inject_error(CoreError::network("transient source failure"));
    let source_for_loop = source.clone();
    let loop_token = token.clone();

    let loop_handle = tokio::spawn(async move {
        run_event_loop(&source_for_loop, &NoopMergedPRHandler, loop_token).await
    });

    let acked = wait_for_acks(&source, 1, &token).await;
    loop_handle.await.unwrap().unwrap();
    assert_eq!(acked, vec!["evt-after-err"]);
    assert!(source.rejected_ids().await.is_empty());
}

/// An empty source with a cancellation token exits cleanly.
#[tokio::test]
async fn test_run_event_loop_empty_source_exits_cleanly() {
    let token = CancellationToken::new();
    let source = TestEventSource::empty();
    let source_for_loop = source.clone();
    let loop_token = token.clone();

    let loop_handle = tokio::spawn(async move {
        run_event_loop(&source_for_loop, &NoopMergedPRHandler, loop_token).await
    });

    // Nothing to wait for; cancel immediately and let the loop exit cleanly.
    tokio::time::sleep(Duration::from_millis(50)).await;
    token.cancel();

    loop_handle.await.unwrap().unwrap();
    assert!(source.acknowledged_ids().await.is_empty());
    assert!(source.rejected_ids().await.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// handle_merged_pull_request tests — inline test doubles
// ─────────────────────────────────────────────────────────────────────────────
//
// Using cross-crate mocks from `release_regent_testing` in this file causes
// E0277 (cross-crate type identity) so all three trait doubles are defined
// inline here.

// ─────────────────────────────────────────────────────────────────────────────
// Event loop wiring test — verifies handler is called for PullRequestMerged
// ─────────────────────────────────────────────────────────────────────────────

/// `run_event_loop` invokes the `MergedPullRequestHandler` for every
/// `PullRequestMerged` event and does NOT invoke it for other event types.
#[tokio::test]
async fn test_run_event_loop_calls_handler_for_pull_request_merged_events() {
    let token = CancellationToken::new();
    let source = TestEventSource::new(vec![
        make_test_event("evt-pr-a", EventType::PullRequestMerged),
        make_test_event("evt-pr-b", EventType::PullRequestMerged),
        make_test_event("evt-release", EventType::ReleasePrMerged),
    ]);
    let source_for_loop = source.clone();
    let loop_token = token.clone();

    let handler = SpyMergedPRHandler::new();
    let handler_for_loop = handler.clone();

    let loop_handle = tokio::spawn(async move {
        run_event_loop(&source_for_loop, &handler_for_loop, loop_token).await
    });

    let acked = wait_for_acks(&source, 3, &token).await;
    loop_handle.await.unwrap().unwrap();

    // All three events must be acknowledged.
    assert_eq!(acked.len(), 3);

    // The handler should have been called only for the two PullRequestMerged events.
    let handled = handler.received_event_ids().await;
    assert_eq!(handled, vec!["evt-pr-a", "evt-pr-b"]);

    // The release PR merged handler should have been called for the ReleasePrMerged event.
    let release_handled = handler.received_release_pr_event_ids().await;
    assert_eq!(release_handled, vec!["evt-release"]);
}

/// `run_event_loop` dispatches `PullRequestOpened` events to
/// `handle_pull_request_activity` and acknowledges them.
#[tokio::test]
async fn test_run_event_loop_dispatches_pull_request_opened_to_activity_handler() {
    let token = CancellationToken::new();
    let source = TestEventSource::new(vec![make_test_event(
        "evt-opened",
        EventType::PullRequestOpened,
    )]);
    let source_for_loop = source.clone();
    let loop_token = token.clone();

    let handler = SpyMergedPRHandler::new();
    let handler_for_loop = handler.clone();

    let loop_handle = tokio::spawn(async move {
        run_event_loop(&source_for_loop, &handler_for_loop, loop_token).await
    });

    let acked = wait_for_acks(&source, 1, &token).await;
    loop_handle.await.unwrap().unwrap();

    assert_eq!(acked, vec!["evt-opened"]);
    let activity = handler.received_activity_event_ids().await;
    assert_eq!(activity, vec!["evt-opened"]);
    // Ensure the merged-PR handler was NOT called.
    assert!(handler.received_event_ids().await.is_empty());
}

/// `run_event_loop` dispatches `PullRequestUpdated` events to
/// `handle_pull_request_activity` and acknowledges them.
#[tokio::test]
async fn test_run_event_loop_dispatches_pull_request_updated_to_activity_handler() {
    let token = CancellationToken::new();
    let source = TestEventSource::new(vec![make_test_event(
        "evt-updated",
        EventType::PullRequestUpdated,
    )]);
    let source_for_loop = source.clone();
    let loop_token = token.clone();

    let handler = SpyMergedPRHandler::new();
    let handler_for_loop = handler.clone();

    let loop_handle = tokio::spawn(async move {
        run_event_loop(&source_for_loop, &handler_for_loop, loop_token).await
    });

    let acked = wait_for_acks(&source, 1, &token).await;
    loop_handle.await.unwrap().unwrap();

    assert_eq!(acked, vec!["evt-updated"]);
    let activity = handler.received_activity_event_ids().await;
    assert_eq!(activity, vec!["evt-updated"]);
    assert!(handler.received_event_ids().await.is_empty());
}

/// A `PullRequestMerged` event whose handler returns an error is rejected.
#[tokio::test]
async fn test_run_event_loop_rejects_event_when_handler_fails() {
    #[derive(Clone)]
    struct FailingHandler;

    #[async_trait]
    impl MergedPullRequestHandler for FailingHandler {
        async fn handle_merged_pull_request(&self, _event: &ProcessingEvent) -> CoreResult<()> {
            Err(CoreError::invalid_input("pr", "simulated handler failure"))
        }
    }

    let token = CancellationToken::new();
    let source = TestEventSource::new(vec![make_test_event(
        "evt-fail",
        EventType::PullRequestMerged,
    )]);
    let source_for_loop = source.clone();
    let loop_token = token.clone();

    let loop_handle =
        tokio::spawn(
            async move { run_event_loop(&source_for_loop, &FailingHandler, loop_token).await },
        );

    // Wait for the rejection to land.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if !source.rejected_ids().await.is_empty() {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    token.cancel();
    loop_handle.await.unwrap().unwrap();

    let rejected = source.rejected_ids().await;
    assert_eq!(rejected.len(), 1);
    assert_eq!(rejected[0].0, "evt-fail");
    // InvalidInput is not retryable → permanent = true.
    assert!(rejected[0].1, "expected permanent rejection");
    assert!(source.acknowledged_ids().await.is_empty());
}

use std::collections::HashMap;
use traits::{
    configuration_provider::{
        ConfigurationProvider, ConfigurationSource, LoadOptions, RepositoryConfig, ValidationResult,
    },
    git_operations::{
        GetCommitsOptions, GitCommit, GitOperations, GitRepository, GitTag, GitTagType,
        GitUser as GitOpsUser, ListTagsOptions,
    },
    github_operations::{
        CreatePullRequestParams, CreateReleaseParams, GitHubOperations, GitUser, Label,
        PullRequest, PullRequestBranch, Release, Repository, Tag, UpdateReleaseParams,
    },
    version_calculator::{
        CalculationOptions, ChangelogEntry, CommitAnalysis, ValidationRules, VersionBump,
        VersionCalculationResult, VersionCalculator, VersionContext,
        VersioningStrategy as VCalcStrategy,
    },
};
use versioning::SemanticVersion;

// ── Shared helpers ─────────────────────────────────────────────────────────

fn make_repo() -> Repository {
    Repository {
        clone_url: "https://github.com/acme/app".into(),
        default_branch: "main".into(),
        description: None,
        full_name: "acme/app".into(),
        homepage: None,
        id: 1,
        name: "app".into(),
        owner: "acme".into(),
        private: false,
        ssh_url: "git@github.com:acme/app.git".into(),
    }
}

fn make_pr(number: u64, branch: &str, title: &str) -> PullRequest {
    let repo = make_repo();
    let user = GitUser {
        email: "bot@example.com".into(),
        login: Some("bot".into()),
        name: "Bot".into(),
    };
    PullRequest {
        base: PullRequestBranch {
            ref_name: "main".into(),
            repo: repo.clone(),
            sha: "base000000000000000000000000000000000000".into(),
        },
        body: Some("## Changelog\n\nInitial entry.".into()),
        created_at: Utc::now(),
        draft: false,
        head: PullRequestBranch {
            ref_name: branch.to_string(),
            repo,
            sha: "head000000000000000000000000000000000000".into(),
        },
        merged_at: None,
        number,
        state: "open".into(),
        title: title.to_string(),
        updated_at: Utc::now(),
        user,
    }
}

fn make_git_commit(sha: &str) -> GitCommit {
    GitCommit {
        sha: sha.to_string(),
        author: GitOpsUser {
            name: "Dev".into(),
            email: "dev@example.com".into(),
            username: None,
        },
        committer: GitOpsUser {
            name: "Dev".into(),
            email: "dev@example.com".into(),
            username: None,
        },
        author_date: Utc::now(),
        commit_date: Utc::now(),
        message: "feat: test commit".into(),
        subject: "feat: test commit".into(),
        body: None,
        parents: vec![],
        files: vec![],
    }
}

// ── TestGitHubForLib ────────────────────────────────────────────────────────

#[derive(Clone)]
struct TestGitHubForLib {
    tags: Vec<GitTag>,
    existing_prs: Vec<PullRequest>,
    /// Returned by `search_pull_requests` (independently configurable from
    /// `existing_prs` which drives `list_pull_requests`).
    search_results: Vec<PullRequest>,
    /// Labels keyed by PR / issue number (drives `list_pr_labels`).
    pr_labels: HashMap<u64, Vec<Label>>,
    created_prs: Arc<Mutex<Vec<(String, String, String)>>>, // (branch, title, body)
    create_branch_calls: Arc<Mutex<Vec<String>>>,
    /// Records every `(issue_number, label_name)` passed to `remove_label`.
    removed_labels: Arc<Mutex<Vec<(u64, String)>>>,
    /// Records every `(issue_number, body)` passed to `create_issue_comment`.
    issue_comments: Arc<Mutex<Vec<(u64, String)>>>,
    /// Pre-seeded issue comments returned by `list_issue_comments`, keyed by
    /// PR/issue number.  Each call returns the vec for that number (or empty).
    stored_issue_comments: HashMap<u64, Vec<crate::traits::github_operations::IssueComment>>,
    /// Records every `(comment_id, body)` passed to `update_issue_comment`.
    update_comment_calls: Arc<Mutex<Vec<(u64, String)>>>,
    /// When set, `search_pull_requests` returns an error if the query
    /// contains this label name.  Other searches succeed normally.
    fail_search_for_label: Option<String>,
    /// When set, `remove_label` returns a network error for the specified PR number.
    fail_remove_label_for_pr: Option<u64>,
    /// When set, `create_issue_comment` returns a network error for the specified
    /// PR / issue number.
    fail_comment_for_pr: Option<u64>,
    /// When set, `list_pr_labels` returns a `CoreError::GitHub` for the
    /// specified PR / issue number.
    fail_list_labels_for_pr: Option<u64>,
}

impl TestGitHubForLib {
    fn new_empty() -> Self {
        Self {
            tags: vec![],
            existing_prs: vec![],
            search_results: vec![],
            pr_labels: HashMap::new(),
            created_prs: Arc::new(Mutex::new(vec![])),
            create_branch_calls: Arc::new(Mutex::new(vec![])),
            removed_labels: Arc::new(Mutex::new(vec![])),
            issue_comments: Arc::new(Mutex::new(vec![])),
            stored_issue_comments: HashMap::new(),
            update_comment_calls: Arc::new(Mutex::new(vec![])),
            fail_search_for_label: None,
            fail_remove_label_for_pr: None,
            fail_comment_for_pr: None,
            fail_list_labels_for_pr: None,
        }
    }

    fn with_tags(mut self, tags: Vec<GitTag>) -> Self {
        self.tags = tags;
        self
    }

    fn with_pr_labels(mut self, pr_number: u64, labels: Vec<Label>) -> Self {
        self.pr_labels.insert(pr_number, labels);
        self
    }

    fn with_search_results(mut self, prs: Vec<PullRequest>) -> Self {
        self.search_results = prs;
        self
    }

    /// Pre-seed `list_issue_comments` for a specific issue/PR number.
    fn with_stored_issue_comments(
        mut self,
        issue_number: u64,
        comments: Vec<crate::traits::github_operations::IssueComment>,
    ) -> Self {
        self.stored_issue_comments.insert(issue_number, comments);
        self
    }

    /// Make `search_pull_requests` return an error when the query contains
    /// the specified label name.  Other searches succeed normally.
    fn with_fail_search_for_label(mut self, label: impl Into<String>) -> Self {
        self.fail_search_for_label = Some(label.into());
        self
    }

    /// Make `remove_label` return a network error for the specified PR number.
    fn with_fail_remove_label_for_pr(mut self, pr_number: u64) -> Self {
        self.fail_remove_label_for_pr = Some(pr_number);
        self
    }

    /// Make `create_issue_comment` return a network error for the specified PR
    /// / issue number.
    fn with_fail_comment_for_pr(mut self, pr_number: u64) -> Self {
        self.fail_comment_for_pr = Some(pr_number);
        self
    }

    /// Make `list_pr_labels` return a `CoreError::GitHub` for the specified PR
    /// / issue number.
    fn with_fail_list_labels_for_pr(mut self, pr_number: u64) -> Self {
        self.fail_list_labels_for_pr = Some(pr_number);
        self
    }
}

#[async_trait]
impl GitOperations for TestGitHubForLib {
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

    async fn get_commit(&self, _owner: &str, _repo: &str, sha: &str) -> CoreResult<GitCommit> {
        Ok(make_git_commit(sha))
    }

    async fn list_tags(
        &self,
        _owner: &str,
        _repo: &str,
        _options: ListTagsOptions,
    ) -> CoreResult<Vec<GitTag>> {
        Ok(self.tags.clone())
    }

    async fn get_tag(&self, _owner: &str, _repo: &str, _tag_name: &str) -> CoreResult<GitTag> {
        Err(CoreError::not_found("tag"))
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
        Ok(make_git_commit("head000000000000000000000000000000000000"))
    }

    async fn get_repository_info(&self, owner: &str, repo: &str) -> CoreResult<GitRepository> {
        Ok(GitRepository {
            name: repo.to_string(),
            owner: owner.to_string(),
            full_name: format!("{owner}/{repo}"),
            default_branch: "main".into(),
            clone_url: format!("https://github.com/{owner}/{repo}"),
            ssh_url: format!("git@github.com:{owner}/{repo}.git"),
            private: false,
            description: None,
        })
    }
}

#[async_trait]
impl GitHubOperations for TestGitHubForLib {
    async fn create_pull_request(
        &self,
        _owner: &str,
        _repo: &str,
        params: CreatePullRequestParams,
    ) -> CoreResult<PullRequest> {
        let pr = make_pr(42, &params.head, &params.title);
        self.created_prs.lock().await.push((
            params.head,
            params.title,
            params.body.unwrap_or_default(),
        ));
        Ok(pr)
    }

    async fn create_release(
        &self,
        _owner: &str,
        _repo: &str,
        _params: CreateReleaseParams,
    ) -> CoreResult<Release> {
        Err(CoreError::not_found("release"))
    }

    async fn create_tag(
        &self,
        _owner: &str,
        _repo: &str,
        tag_name: &str,
        sha: &str,
        _message: Option<String>,
        _tagger: Option<GitUser>,
    ) -> CoreResult<Tag> {
        Ok(Tag {
            name: tag_name.to_string(),
            commit_sha: sha.to_string(),
            message: None,
            tagger: None,
            created_at: None,
        })
    }

    async fn get_latest_release(&self, _owner: &str, _repo: &str) -> CoreResult<Option<Release>> {
        Ok(None)
    }

    async fn get_pull_request(
        &self,
        _owner: &str,
        _repo: &str,
        number: u64,
    ) -> CoreResult<PullRequest> {
        self.existing_prs
            .iter()
            .find(|pr| pr.number == number)
            .cloned()
            .ok_or_else(|| CoreError::not_found("PR"))
    }

    async fn get_release_by_tag(
        &self,
        _owner: &str,
        _repo: &str,
        _tag: &str,
    ) -> CoreResult<Release> {
        Err(CoreError::not_found("release"))
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
        Ok(self.existing_prs.clone())
    }

    async fn search_pull_requests(
        &self,
        _owner: &str,
        _repo: &str,
        query: &str,
    ) -> CoreResult<Vec<PullRequest>> {
        if let Some(ref failing_label) = self.fail_search_for_label {
            if query.contains(failing_label.as_str()) {
                return Err(CoreError::network(format!(
                    "Simulated search_pull_requests failure for label {failing_label}"
                )));
            }
        }
        Ok(self.search_results.clone())
    }

    async fn update_pull_request(
        &self,
        _owner: &str,
        _repo: &str,
        number: u64,
        _title: Option<String>,
        _body: Option<String>,
        _state: Option<String>,
    ) -> CoreResult<PullRequest> {
        self.existing_prs
            .iter()
            .find(|pr| pr.number == number)
            .cloned()
            .ok_or_else(|| CoreError::not_found("PR"))
    }

    async fn update_release(
        &self,
        _owner: &str,
        _repo: &str,
        _release_id: u64,
        _params: UpdateReleaseParams,
    ) -> CoreResult<Release> {
        Err(CoreError::not_found("release"))
    }

    async fn create_branch(
        &self,
        _owner: &str,
        _repo: &str,
        branch_name: &str,
        _sha: &str,
    ) -> CoreResult<()> {
        self.create_branch_calls
            .lock()
            .await
            .push(branch_name.to_string());
        Ok(())
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

    async fn create_issue_comment(
        &self,
        _owner: &str,
        _repo: &str,
        issue_number: u64,
        body: &str,
    ) -> CoreResult<()> {
        if self.fail_comment_for_pr == Some(issue_number) {
            return Err(CoreError::network(format!(
                "Simulated create_issue_comment failure for PR #{issue_number}"
            )));
        }
        self.issue_comments
            .lock()
            .await
            .push((issue_number, body.to_string()));
        Ok(())
    }

    async fn get_collaborator_permission(
        &self,
        _owner: &str,
        _repo: &str,
        _username: &str,
    ) -> CoreResult<crate::traits::github_operations::CollaboratorPermission> {
        Ok(crate::traits::github_operations::CollaboratorPermission::Write)
    }

    async fn add_labels(
        &self,
        _owner: &str,
        _repo: &str,
        _issue_number: u64,
        _labels: &[&str],
    ) -> CoreResult<()> {
        Ok(())
    }

    async fn remove_label(
        &self,
        _owner: &str,
        _repo: &str,
        issue_number: u64,
        label_name: &str,
    ) -> CoreResult<()> {
        if self.fail_remove_label_for_pr == Some(issue_number) {
            return Err(CoreError::network(format!(
                "Simulated remove_label failure for PR #{issue_number}"
            )));
        }
        self.removed_labels
            .lock()
            .await
            .push((issue_number, label_name.to_string()));
        Ok(())
    }

    async fn list_pr_labels(
        &self,
        _owner: &str,
        _repo: &str,
        issue_number: u64,
    ) -> CoreResult<Vec<crate::traits::github_operations::Label>> {
        if self.fail_list_labels_for_pr == Some(issue_number) {
            return Err(CoreError::github(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Simulated list_pr_labels failure for PR #{issue_number}"),
            )));
        }
        Ok(self
            .pr_labels
            .get(&issue_number)
            .cloned()
            .unwrap_or_default())
    }

    async fn get_installation_id_for_repo(&self, _owner: &str, _repo: &str) -> CoreResult<u64> {
        Ok(0)
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

    async fn get_file_content(
        &self,
        _owner: &str,
        _repo: &str,
        _path: &str,
        _branch: &str,
    ) -> CoreResult<Option<String>> {
        Ok(None)
    }

    async fn batch_commit_files(
        &self,
        _owner: &str,
        _repo: &str,
        _branch: &str,
        _files: &[crate::traits::github_operations::FileUpdate],
        _message: &str,
    ) -> CoreResult<()> {
        Ok(())
    }

    async fn list_issue_comments(
        &self,
        _owner: &str,
        _repo: &str,
        issue_number: u64,
    ) -> CoreResult<Vec<crate::traits::github_operations::IssueComment>> {
        Ok(self
            .stored_issue_comments
            .get(&issue_number)
            .cloned()
            .unwrap_or_default())
    }

    async fn update_issue_comment(
        &self,
        _owner: &str,
        _repo: &str,
        comment_id: u64,
        body: &str,
    ) -> CoreResult<()> {
        self.update_comment_calls
            .lock()
            .await
            .push((comment_id, body.to_string()));
        Ok(())
    }

    fn scoped_to(&self, _installation_id: u64) -> Self {
        Self {
            tags: self.tags.clone(),
            existing_prs: self.existing_prs.clone(),
            search_results: self.search_results.clone(),
            pr_labels: self.pr_labels.clone(),
            created_prs: Arc::clone(&self.created_prs),
            create_branch_calls: Arc::clone(&self.create_branch_calls),
            removed_labels: Arc::clone(&self.removed_labels),
            issue_comments: Arc::clone(&self.issue_comments),
            stored_issue_comments: self.stored_issue_comments.clone(),
            update_comment_calls: Arc::clone(&self.update_comment_calls),
            fail_search_for_label: self.fail_search_for_label.clone(),
            fail_remove_label_for_pr: self.fail_remove_label_for_pr,
            fail_comment_for_pr: self.fail_comment_for_pr,
            fail_list_labels_for_pr: self.fail_list_labels_for_pr,
        }
    }
}

// ── TestConfigForLib ────────────────────────────────────────────────────────

#[derive(Clone)]
struct TestConfigForLib;

#[async_trait]
impl ConfigurationProvider for TestConfigForLib {
    async fn load_global_config(
        &self,
        _options: LoadOptions,
    ) -> CoreResult<config::ReleaseRegentConfig> {
        Ok(config::ReleaseRegentConfig::default())
    }

    async fn load_repository_config(
        &self,
        _owner: &str,
        _repo: &str,
        _options: LoadOptions,
    ) -> CoreResult<Option<RepositoryConfig>> {
        Ok(None)
    }

    async fn get_merged_config(
        &self,
        _owner: &str,
        _repo: &str,
        _options: LoadOptions,
    ) -> CoreResult<config::ReleaseRegentConfig> {
        Ok(config::ReleaseRegentConfig::default())
    }

    async fn validate_config(
        &self,
        _config: &config::ReleaseRegentConfig,
    ) -> CoreResult<ValidationResult> {
        Ok(ValidationResult {
            is_valid: true,
            errors: vec![],
            warnings: vec![],
        })
    }

    async fn save_config(
        &self,
        _config: &config::ReleaseRegentConfig,
        _owner: Option<&str>,
        _repo: Option<&str>,
        _global: bool,
    ) -> CoreResult<()> {
        Ok(())
    }

    async fn list_repository_configs(
        &self,
        _options: LoadOptions,
    ) -> CoreResult<Vec<RepositoryConfig>> {
        Ok(vec![])
    }

    async fn get_config_source(
        &self,
        _owner: Option<&str>,
        _repo: Option<&str>,
    ) -> CoreResult<ConfigurationSource> {
        Ok(ConfigurationSource {
            format: "yaml".into(),
            loaded_at: Utc::now(),
            location: "default".into(),
            source_type: "default".into(),
        })
    }

    async fn reload_config(&self, _owner: Option<&str>, _repo: Option<&str>) -> CoreResult<()> {
        Ok(())
    }

    async fn config_exists(&self, _owner: Option<&str>, _repo: Option<&str>) -> CoreResult<bool> {
        Ok(true)
    }

    fn supported_formats(&self) -> Vec<String> {
        vec!["yaml".into()]
    }

    async fn get_default_config(&self) -> CoreResult<config::ReleaseRegentConfig> {
        Ok(config::ReleaseRegentConfig::default())
    }
}

/// Configurable variant of `TestConfigForLib` — returns a fixed
/// `ReleaseRegentConfig` supplied at construction time.
#[derive(Clone)]
struct TestConfigWith(config::ReleaseRegentConfig);

impl TestConfigWith {
    fn new(cfg: config::ReleaseRegentConfig) -> Self {
        Self(cfg)
    }
}

#[async_trait]
impl ConfigurationProvider for TestConfigWith {
    async fn load_global_config(
        &self,
        _options: LoadOptions,
    ) -> CoreResult<config::ReleaseRegentConfig> {
        Ok(self.0.clone())
    }

    async fn load_repository_config(
        &self,
        _owner: &str,
        _repo: &str,
        _options: LoadOptions,
    ) -> CoreResult<Option<RepositoryConfig>> {
        Ok(None)
    }

    async fn get_merged_config(
        &self,
        _owner: &str,
        _repo: &str,
        _options: LoadOptions,
    ) -> CoreResult<config::ReleaseRegentConfig> {
        Ok(self.0.clone())
    }

    async fn validate_config(
        &self,
        _config: &config::ReleaseRegentConfig,
    ) -> CoreResult<ValidationResult> {
        Ok(ValidationResult {
            is_valid: true,
            errors: vec![],
            warnings: vec![],
        })
    }

    async fn save_config(
        &self,
        _config: &config::ReleaseRegentConfig,
        _owner: Option<&str>,
        _repo: Option<&str>,
        _global: bool,
    ) -> CoreResult<()> {
        Ok(())
    }

    async fn list_repository_configs(
        &self,
        _options: LoadOptions,
    ) -> CoreResult<Vec<RepositoryConfig>> {
        Ok(vec![])
    }

    async fn get_config_source(
        &self,
        _owner: Option<&str>,
        _repo: Option<&str>,
    ) -> CoreResult<ConfigurationSource> {
        Ok(ConfigurationSource {
            format: "yaml".into(),
            loaded_at: Utc::now(),
            location: "default".into(),
            source_type: "default".into(),
        })
    }

    async fn reload_config(&self, _owner: Option<&str>, _repo: Option<&str>) -> CoreResult<()> {
        Ok(())
    }

    async fn config_exists(&self, _owner: Option<&str>, _repo: Option<&str>) -> CoreResult<bool> {
        Ok(true)
    }

    fn supported_formats(&self) -> Vec<String> {
        vec!["yaml".into()]
    }

    async fn get_default_config(&self) -> CoreResult<config::ReleaseRegentConfig> {
        Ok(self.0.clone())
    }
}

// ── TestVersionCalcForLib ───────────────────────────────────────────────────

#[derive(Clone)]
struct TestVersionCalcForLib {
    next_version: SemanticVersion,
    changelog_entries: Vec<ChangelogEntry>,
    version_bump: VersionBump,
}

impl TestVersionCalcForLib {
    fn returning(version: &str) -> Self {
        Self {
            next_version: versioning::VersionCalculator::parse_version(version).unwrap(),
            changelog_entries: vec![],
            version_bump: VersionBump::Minor,
        }
    }

    fn with_entries(mut self, entries: Vec<ChangelogEntry>) -> Self {
        self.changelog_entries = entries;
        self
    }

    fn with_version_bump(mut self, bump: VersionBump) -> Self {
        self.version_bump = bump;
        self
    }
}

#[async_trait]
impl VersionCalculator for TestVersionCalcForLib {
    async fn calculate_version(
        &self,
        ctx: VersionContext,
        _strategy: VCalcStrategy,
        _options: CalculationOptions,
    ) -> CoreResult<VersionCalculationResult> {
        Ok(VersionCalculationResult {
            next_version: self.next_version.clone(),
            current_version: ctx.current_version,
            version_bump: self.version_bump.clone(),
            is_prerelease: false,
            build_metadata: None,
            analyzed_commits: vec![],
            changelog_entries: self.changelog_entries.clone(),
            strategy: VCalcStrategy::ConventionalCommits {
                custom_types: HashMap::new(),
                include_prerelease: false,
            },
            metadata: HashMap::new(),
        })
    }

    async fn analyze_commits(
        &self,
        _ctx: VersionContext,
        _strategy: VCalcStrategy,
        _shas: Vec<String>,
    ) -> CoreResult<Vec<CommitAnalysis>> {
        Ok(vec![])
    }

    async fn validate_version(
        &self,
        _ctx: VersionContext,
        _proposed: SemanticVersion,
        _rules: ValidationRules,
    ) -> CoreResult<bool> {
        Ok(true)
    }

    async fn get_version_bump(
        &self,
        _ctx: VersionContext,
        _strategy: VCalcStrategy,
        _analyses: Vec<CommitAnalysis>,
    ) -> CoreResult<VersionBump> {
        Ok(VersionBump::Minor)
    }

    async fn generate_changelog_entries(
        &self,
        _ctx: VersionContext,
        _strategy: VCalcStrategy,
        _analyses: Vec<CommitAnalysis>,
        _version: SemanticVersion,
    ) -> CoreResult<Vec<ChangelogEntry>> {
        Ok(self.changelog_entries.clone())
    }

    async fn preview_calculation(
        &self,
        ctx: VersionContext,
        strategy: VCalcStrategy,
        options: CalculationOptions,
    ) -> CoreResult<VersionCalculationResult> {
        self.calculate_version(ctx, strategy, options).await
    }

    fn supported_strategies(&self) -> HashMap<String, String> {
        HashMap::new()
    }

    fn default_strategy(&self) -> VCalcStrategy {
        VCalcStrategy::ConventionalCommits {
            custom_types: HashMap::new(),
            include_prerelease: false,
        }
    }

    fn parse_conventional_commit(&self, _message: &str) -> CoreResult<Option<CommitAnalysis>> {
        Ok(None)
    }

    fn apply_version_bump(
        &self,
        current_version: SemanticVersion,
        _bump_type: VersionBump,
        _prerelease: Option<String>,
        _build: Option<String>,
    ) -> CoreResult<SemanticVersion> {
        Ok(current_version)
    }

    fn scoped_to(&self, _installation_id: u64) -> Arc<dyn VersionCalculator + Send + Sync> {
        Arc::new(TestVersionCalcForLib {
            next_version: self.next_version.clone(),
            changelog_entries: self.changelog_entries.clone(),
            version_bump: self.version_bump.clone(),
        })
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

/// A PR merge event with a valid payload causes a new release PR to be created.
#[tokio::test]
async fn test_handle_merged_pr_creates_release_pr_when_none_exists() {
    let github = TestGitHubForLib::new_empty();
    let config = TestConfigForLib;
    let version_calc = TestVersionCalcForLib::returning("0.2.0");

    let processor = ReleaseRegentProcessor::new(github.clone(), config, version_calc);

    let event = ProcessingEvent {
        event_id: "evt-1".into(),
        correlation_id: "corr-1".into(),
        event_type: EventType::PullRequestMerged,
        repository: RepositoryInfo {
            owner: "acme".into(),
            name: "app".into(),
            default_branch: "main".into(),
        },
        payload: serde_json::json!({
            "pull_request": {
                "base": { "ref": "main" },
                "merge_commit_sha": "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
            }
        }),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
    };

    let result = processor.handle_merged_pull_request(&event).await.unwrap();

    assert!(
        matches!(
            result,
            release_orchestrator::OrchestratorResult::Created { .. }
        ),
        "expected Created, got {result:?}"
    );

    let branch_calls = github.create_branch_calls.lock().await;
    assert_eq!(branch_calls.len(), 1);
    assert!(
        branch_calls[0].starts_with("release/v"),
        "branch should start with release/v, got {}",
        branch_calls[0]
    );

    let created_prs = github.created_prs.lock().await;
    assert_eq!(created_prs.len(), 1);
}

/// When the payload is missing both `merge_commit_sha` and `head.sha`, the
/// method returns `CoreError::InvalidInput`.
#[tokio::test]
async fn test_handle_merged_pr_returns_invalid_input_when_sha_missing() {
    let github = TestGitHubForLib::new_empty();
    let config = TestConfigForLib;
    let version_calc = TestVersionCalcForLib::returning("0.2.0");

    let processor = ReleaseRegentProcessor::new(github, config, version_calc);

    let event = ProcessingEvent {
        event_id: "evt-2".into(),
        correlation_id: "corr-2".into(),
        event_type: EventType::PullRequestMerged,
        repository: RepositoryInfo {
            owner: "acme".into(),
            name: "app".into(),
            default_branch: "main".into(),
        },
        // Payload has no merge_commit_sha or head.sha
        payload: serde_json::json!({ "pull_request": { "base": { "ref": "main" } } }),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
    };

    let err = processor
        .handle_merged_pull_request(&event)
        .await
        .unwrap_err();
    assert!(
        matches!(err, CoreError::InvalidInput { .. }),
        "expected InvalidInput, got {err:?}"
    );
}

/// When base_branch is absent from the payload the repository default_branch
/// is used as the release PR base.
#[tokio::test]
async fn test_handle_merged_pr_uses_default_branch_when_base_ref_absent() {
    let github = TestGitHubForLib::new_empty();
    let config = TestConfigForLib;
    let version_calc = TestVersionCalcForLib::returning("1.0.0");

    let processor = ReleaseRegentProcessor::new(github.clone(), config, version_calc);

    let event = ProcessingEvent {
        event_id: "evt-3".into(),
        correlation_id: "corr-3".into(),
        event_type: EventType::PullRequestMerged,
        repository: RepositoryInfo {
            owner: "acme".into(),
            name: "app".into(),
            default_branch: "trunk".into(), // non-default name
        },
        // No "base.ref" — should fall back to "trunk"
        payload: serde_json::json!({
            "pull_request": {
                "merge_commit_sha": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            }
        }),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
    };

    let result = processor.handle_merged_pull_request(&event).await.unwrap();

    assert!(matches!(
        result,
        release_orchestrator::OrchestratorResult::Created { .. }
    ));

    let prs = github.created_prs.lock().await;
    assert_eq!(prs.len(), 1);
}

/// Changelog entries produced by the version calculator are rendered into the
/// PR body.
#[tokio::test]
async fn test_handle_merged_pr_includes_changelog_entries_in_pr_body() {
    let github = TestGitHubForLib::new_empty();
    let config = TestConfigForLib;
    let version_calc = TestVersionCalcForLib::returning("0.3.0").with_entries(vec![
        ChangelogEntry {
            commit_sha: "a".repeat(40),
            description: "add shiny feature".into(),
            entry_type: "Added".into(),
            is_breaking: false,
            issues: vec![],
            pr_number: None,
            scope: None,
        },
        ChangelogEntry {
            commit_sha: "b".repeat(40),
            description: "fix nasty bug".into(),
            entry_type: "Fixed".into(),
            is_breaking: false,
            issues: vec![],
            pr_number: None,
            scope: None,
        },
    ]);

    let processor = ReleaseRegentProcessor::new(github.clone(), config, version_calc);

    let event = ProcessingEvent {
        event_id: "evt-4".into(),
        correlation_id: "corr-4".into(),
        event_type: EventType::PullRequestMerged,
        repository: RepositoryInfo {
            owner: "acme".into(),
            name: "app".into(),
            default_branch: "main".into(),
        },
        payload: serde_json::json!({
            "pull_request": {
                "base": { "ref": "main" },
                "merge_commit_sha": "cccccccccccccccccccccccccccccccccccccccc"
            }
        }),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
    };

    processor.handle_merged_pull_request(&event).await.unwrap();

    let prs = github.created_prs.lock().await;
    assert_eq!(prs.len(), 1);
    let body = &prs[0].2;
    assert!(
        body.contains("add shiny feature"),
        "body missing 'add shiny feature': {body}"
    );
    assert!(
        body.contains("fix nasty bug"),
        "body missing 'fix nasty bug': {body}"
    );
    assert!(
        body.contains(&"a".repeat(40)),
        "body missing commit sha A: {body}"
    );
    assert!(
        body.contains(&"b".repeat(40)),
        "body missing commit sha B: {body}"
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Bump-override floor tests (task 9.20)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// When the merged feature PR carries an `rr:override-major` label, the
/// calculated version (a minor bump) is raised to a major bump and an audit
/// comment is posted on the resulting release PR.
#[tokio::test]
async fn test_handle_merged_feature_pr_with_override_major_label_applies_floor() {
    // Current released tag: v1.0.0  →  current_version = Some(1.0.0)
    // Version calculator produces 1.1.0 (minor bump).
    // Floor = Major  →  apply_bump_floor(1.0.0, 1.1.0, Major) = 2.0.0
    let tag = GitTag {
        name: "v1.0.0".to_string(),
        target_sha: "a".repeat(40),
        tag_type: GitTagType::Lightweight,
        message: None,
        tagger: None,
        created_at: None,
    };
    let override_label = Label {
        id: 1,
        name: "rr:override-major".to_string(),
        color: "ff0000".to_string(),
        description: None,
    };
    // PR #7 is the feature PR being merged; its labels are read at orchestration time.
    let github = TestGitHubForLib::new_empty()
        .with_tags(vec![tag])
        .with_pr_labels(7, vec![override_label]);

    let config = TestConfigForLib;
    let version_calc = TestVersionCalcForLib::returning("1.1.0");

    let processor = ReleaseRegentProcessor::new(github.clone(), config, version_calc);

    let event = ProcessingEvent {
        event_id: "evt-floor-1".into(),
        correlation_id: "corr-floor-1".into(),
        event_type: EventType::PullRequestMerged,
        repository: RepositoryInfo {
            owner: "acme".into(),
            name: "app".into(),
            default_branch: "main".into(),
        },
        payload: serde_json::json!({
            "pull_request": {
                // Head branch is NOT a release branch → feature PR path.
                "head": { "ref": "feat/cool-feature" },
                "base": { "ref": "main" },
                "number": 7,
                "merge_commit_sha": "b".repeat(40)
            }
        }),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
    };

    let result = processor.handle_merged_pull_request(&event).await.unwrap();

    // Release PR must be created at 2.0.0, not 1.1.0.
    match &result {
        release_orchestrator::OrchestratorResult::Created { pr, branch_name } => {
            assert!(
                branch_name.contains("2.0.0"),
                "branch should reference 2.0.0 (floor applied), got: {branch_name}"
            );
            assert!(
                pr.head.ref_name.contains("2.0.0"),
                "PR head branch should reference 2.0.0, got: {}",
                pr.head.ref_name
            );
        }
        other => panic!("expected Created, got {other:?}"),
    }

    // An audit comment should have been posted on the release PR explaining
    // the floor application.
    let comments = github.issue_comments.lock().await;
    assert!(
        !comments.is_empty(),
        "expected at least one audit comment, got none"
    );
    let audit = comments
        .iter()
        .find(|(_, body)| body.contains("Version floor applied"))
        .expect("audit comment about version floor was not posted");
    assert!(
        audit.1.contains("major"),
        "audit comment should mention 'major', got: {}",
        audit.1
    );
    assert!(
        audit.1.contains("2.0.0"),
        "audit comment should mention effective version 2.0.0, got: {}",
        audit.1
    );

    // The consumed override label must be removed from the merged feature PR.
    let removed = github.removed_labels.lock().await;
    assert!(
        removed
            .iter()
            .any(|(pr, label)| *pr == 7 && label == "rr:override-major"),
        "expected rr:override-major to be removed from merged feature PR #7, got: {removed:?}"
    );
}

/// When the merged feature PR has no override labels, the calculated version
/// is used unchanged and no audit comment is posted.
#[tokio::test]
async fn test_handle_merged_feature_pr_without_override_label_uses_calculated_version() {
    let tag = GitTag {
        name: "v1.0.0".to_string(),
        target_sha: "a".repeat(40),
        tag_type: GitTagType::Lightweight,
        message: None,
        tagger: None,
        created_at: None,
    };
    // No pr_labels configured → list_pr_labels returns [].
    let github = TestGitHubForLib::new_empty().with_tags(vec![tag]);
    let config = TestConfigForLib;
    let version_calc = TestVersionCalcForLib::returning("1.1.0");

    let processor = ReleaseRegentProcessor::new(github.clone(), config, version_calc);

    let event = ProcessingEvent {
        event_id: "evt-floor-2".into(),
        correlation_id: "corr-floor-2".into(),
        event_type: EventType::PullRequestMerged,
        repository: RepositoryInfo {
            owner: "acme".into(),
            name: "app".into(),
            default_branch: "main".into(),
        },
        payload: serde_json::json!({
            "pull_request": {
                "head": { "ref": "feat/other-feature" },
                "base": { "ref": "main" },
                "number": 8,
                "merge_commit_sha": "c".repeat(40)
            }
        }),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
    };

    let result = processor.handle_merged_pull_request(&event).await.unwrap();

    // Release PR should be at 1.1.0 (the calculated version, no floor change).
    match &result {
        release_orchestrator::OrchestratorResult::Created { branch_name, .. } => {
            assert!(
                branch_name.contains("1.1.0"),
                "branch should reference 1.1.0 (no floor), got: {branch_name}"
            );
        }
        other => panic!("expected Created, got {other:?}"),
    }

    // No audit comment should have been posted (no floor was applied).
    let comments = github.issue_comments.lock().await;
    assert!(
        comments
            .iter()
            .all(|(_, body)| !body.contains("Version floor applied")),
        "unexpected audit comment when no floor was applied"
    );
}

/// When a release PR is merged, stale `rr:override-*` labels on open feature
/// PRs are removed and a cleanup comment is posted on each affected PR.
#[tokio::test]
async fn test_handle_merged_release_pr_clears_stale_override_labels_from_open_prs() {
    // Feature PR #99 has a stale rr:override-major label from a previous
    // !release command.  It will appear in search results for all three label
    // queries (the stub returns `search_results` regardless of the query, which
    // exercises the cleanup loop for all three label names).
    let stale_pr = make_pr(99, "feat/stale-feature", "feat: stale work");

    let github = TestGitHubForLib::new_empty().with_search_results(vec![stale_pr]);
    let config = TestConfigForLib;
    let version_calc = TestVersionCalcForLib::returning("1.1.0");

    let processor = ReleaseRegentProcessor::new(github.clone(), config, version_calc);

    let event = ProcessingEvent {
        event_id: "evt-floor-3".into(),
        correlation_id: "corr-floor-3".into(),
        event_type: EventType::PullRequestMerged,
        repository: RepositoryInfo {
            owner: "acme".into(),
            name: "app".into(),
            default_branch: "main".into(),
        },
        payload: serde_json::json!({
            "pull_request": {
                // Head branch starts with "release/v" → release PR path.
                "head": { "ref": "release/v1.0.0" },
                "base": { "ref": "main" },
                "number": 100,
                "merge_commit_sha": "d".repeat(40)
            }
        }),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
    };

    processor.handle_merged_pull_request(&event).await.unwrap();

    // remove_label should have been called once per override label constant
    // (major, minor, patch), each time on PR #99.
    let removed = github.removed_labels.lock().await;
    assert_eq!(
        removed.len(),
        3,
        "expected 3 remove_label calls (one per override label), got: {removed:?}"
    );
    let removed_names: Vec<&str> = removed.iter().map(|(_, n)| n.as_str()).collect();
    assert!(
        removed_names.contains(&"rr:override-major"),
        "expected rr:override-major to be removed"
    );
    assert!(
        removed_names.contains(&"rr:override-minor"),
        "expected rr:override-minor to be removed"
    );
    assert!(
        removed_names.contains(&"rr:override-patch"),
        "expected rr:override-patch to be removed"
    );
    assert!(
        removed.iter().all(|(pr, _)| *pr == 99),
        "all removals should target PR #99"
    );

    // A cleanup comment should have been posted for each removal.
    let comments = github.issue_comments.lock().await;
    assert_eq!(
        comments.len(),
        3,
        "expected 3 cleanup comments (one per override label), got: {comments:?}"
    );
    assert!(
        comments.iter().all(|(pr, _)| *pr == 99),
        "all cleanup comments should target PR #99"
    );
    assert!(
        comments
            .iter()
            .any(|(_, body)| body.contains("cleared because a new release was published")),
        "cleanup comment should explain why the label was cleared"
    );
}

/// When the merged feature PR carries an `rr:override-minor` label, the
/// calculated patch version is raised to the next minor version.
#[tokio::test]
async fn test_handle_merged_feature_pr_with_override_minor_label_applies_floor() {
    // Current released tag: v1.2.3  →  current_version = Some(1.2.3)
    // Version calculator produces 1.2.4 (patch bump).
    // Floor = Minor  →  apply_bump_floor(1.2.3, 1.2.4, Minor) = 1.3.0
    let tag = GitTag {
        name: "v1.2.3".to_string(),
        target_sha: "a".repeat(40),
        tag_type: GitTagType::Lightweight,
        message: None,
        tagger: None,
        created_at: None,
    };
    let override_label = Label {
        id: 2,
        name: "rr:override-minor".to_string(),
        color: "0075ca".to_string(),
        description: None,
    };
    let github = TestGitHubForLib::new_empty()
        .with_tags(vec![tag])
        .with_pr_labels(10, vec![override_label]);

    let config = TestConfigForLib;
    let version_calc = TestVersionCalcForLib::returning("1.2.4");

    let processor = ReleaseRegentProcessor::new(github.clone(), config, version_calc);

    let event = ProcessingEvent {
        event_id: "evt-minor-floor".into(),
        correlation_id: "corr-minor-floor".into(),
        event_type: EventType::PullRequestMerged,
        repository: RepositoryInfo {
            owner: "acme".into(),
            name: "app".into(),
            default_branch: "main".into(),
        },
        payload: serde_json::json!({
            "pull_request": {
                "head": { "ref": "feat/minor-feature" },
                "base": { "ref": "main" },
                "number": 10,
                "merge_commit_sha": "e".repeat(40)
            }
        }),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
    };

    let result = processor.handle_merged_pull_request(&event).await.unwrap();

    // Release PR must be at 1.3.0 (minor floor applied), not 1.2.4.
    match &result {
        release_orchestrator::OrchestratorResult::Created { branch_name, .. } => {
            assert!(
                branch_name.contains("1.3.0"),
                "branch should reference 1.3.0 (minor floor applied), got: {branch_name}"
            );
        }
        other => panic!("expected Created, got {other:?}"),
    }

    // The consumed rr:override-minor label must be removed from PR #10 (task 9.20.5).
    let removed = github.removed_labels.lock().await;
    assert!(
        removed
            .iter()
            .any(|(pr, label)| *pr == 10 && label == "rr:override-minor"),
        "expected rr:override-minor to be removed from merged PR #10, got: {removed:?}"
    );
}

/// When the merged feature PR has `rr:override-patch` and the calculated version
/// is already a minor bump (which exceeds the patch floor), the calculated version
/// is used unchanged and the label is still removed.
#[tokio::test]
async fn test_handle_merged_feature_pr_with_patch_floor_no_effect_when_calculated_exceeds() {
    // Current released tag: v1.2.3  →  current_version = Some(1.2.3)
    // Version calculator produces 1.3.0 (minor bump).
    // Floor = Patch  →  floor_version = 1.2.4, which is LESS than 1.3.0.
    // Effective = max(1.3.0, 1.2.4) = 1.3.0 (unchanged).
    let tag = GitTag {
        name: "v1.2.3".to_string(),
        target_sha: "a".repeat(40),
        tag_type: GitTagType::Lightweight,
        message: None,
        tagger: None,
        created_at: None,
    };
    let override_label = Label {
        id: 3,
        name: "rr:override-patch".to_string(),
        color: "e4e669".to_string(),
        description: None,
    };
    let github = TestGitHubForLib::new_empty()
        .with_tags(vec![tag])
        .with_pr_labels(11, vec![override_label]);

    let config = TestConfigForLib;
    let version_calc = TestVersionCalcForLib::returning("1.3.0");

    let processor = ReleaseRegentProcessor::new(github.clone(), config, version_calc);

    let event = ProcessingEvent {
        event_id: "evt-patch-floor".into(),
        correlation_id: "corr-patch-floor".into(),
        event_type: EventType::PullRequestMerged,
        repository: RepositoryInfo {
            owner: "acme".into(),
            name: "app".into(),
            default_branch: "main".into(),
        },
        payload: serde_json::json!({
            "pull_request": {
                "head": { "ref": "feat/patch-feature" },
                "base": { "ref": "main" },
                "number": 11,
                "merge_commit_sha": "f".repeat(40)
            }
        }),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
    };

    let result = processor.handle_merged_pull_request(&event).await.unwrap();

    // Release PR must be at 1.3.0 (patch floor has no effect on a minor bump).
    match &result {
        release_orchestrator::OrchestratorResult::Created { branch_name, .. } => {
            assert!(
                branch_name.contains("1.3.0"),
                "branch should remain 1.3.0 (patch floor has no effect), got: {branch_name}"
            );
        }
        other => panic!("expected Created, got {other:?}"),
    }

    // No audit comment should be posted because the floor had no effect.
    let comments = github.issue_comments.lock().await;
    assert!(
        comments
            .iter()
            .all(|(_, body)| !body.contains("Version floor applied")),
        "no audit comment should be posted when floor has no effect"
    );

    // Even though the floor had no effect, the label must still be removed (task 9.20.5).
    let removed = github.removed_labels.lock().await;
    assert!(
        removed
            .iter()
            .any(|(pr, label)| *pr == 11 && label == "rr:override-patch"),
        "expected rr:override-patch to be removed from merged PR #11, got: {removed:?}"
    );
}
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// handle_merged_pull_request — release-PR path error tests (spec §9 Minor #5)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// When `search_pull_requests` fails for one override label (e.g., `rr:override-major`),
/// a warning is logged and the cleanup for the remaining labels still proceeds.
/// The overall event must succeed.
#[tokio::test]
async fn test_handle_merged_release_pr_search_failure_for_one_label_continues() {
    // search_results contains PR #99 which will be found for override-minor and
    // override-patch, but the search for override-major will fail.
    let stale_pr = make_pr(99, "feat/stale-feature", "feat: stale work");
    let github = TestGitHubForLib::new_empty()
        .with_search_results(vec![stale_pr])
        .with_fail_search_for_label("rr:override-major");

    let config = TestConfigForLib;
    let version_calc = TestVersionCalcForLib::returning("1.1.0");
    let processor = ReleaseRegentProcessor::new(github.clone(), config, version_calc);

    let event = ProcessingEvent {
        event_id: "evt-search-fail".into(),
        correlation_id: "corr-search-fail".into(),
        event_type: EventType::PullRequestMerged,
        repository: RepositoryInfo {
            owner: "acme".into(),
            name: "app".into(),
            default_branch: "main".into(),
        },
        payload: serde_json::json!({
            "pull_request": {
                "head": { "ref": "release/v1.1.0" },
                "base": { "ref": "main" },
                "number": 100,
                "merge_commit_sha": "d".repeat(40)
            }
        }),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
    };

    // Event must succeed despite the partial search failure.
    let result = processor.handle_merged_pull_request(&event).await;
    assert!(
        result.is_ok(),
        "expected Ok when one search fails, got: {result:?}"
    );

    // override-minor and override-patch searches succeeded → PR #99 was processed
    // for at least 2 labels.  override-major search failed → no remove for that label.
    let removed = github.removed_labels.lock().await;
    assert!(
        removed.len() >= 2,
        "expected at least 2 label removals (minor + patch), got: {removed:?}"
    );
    let has_major_removal = removed
        .iter()
        .any(|(_, label)| label == "rr:override-major");
    assert!(
        !has_major_removal,
        "should NOT have removed override-major (search failed), got: {removed:?}"
    );
}

/// When `remove_label` fails for one PR, a warning is logged and the cleanup
/// comment for that PR is still attempted.  The overall event must succeed.
#[tokio::test]
async fn test_handle_merged_release_pr_remove_label_failure_still_posts_cleanup_comment() {
    let stale_pr = make_pr(99, "feat/stale-feature", "feat: stale work");
    let github = TestGitHubForLib::new_empty()
        .with_search_results(vec![stale_pr])
        .with_fail_remove_label_for_pr(99); // remove_label on PR #99 will fail

    let config = TestConfigForLib;
    let version_calc = TestVersionCalcForLib::returning("1.1.0");
    let processor = ReleaseRegentProcessor::new(github.clone(), config, version_calc);

    let event = ProcessingEvent {
        event_id: "evt-remove-fail".into(),
        correlation_id: "corr-remove-fail".into(),
        event_type: EventType::PullRequestMerged,
        repository: RepositoryInfo {
            owner: "acme".into(),
            name: "app".into(),
            default_branch: "main".into(),
        },
        payload: serde_json::json!({
            "pull_request": {
                "head": { "ref": "release/v1.1.0" },
                "base": { "ref": "main" },
                "number": 100,
                "merge_commit_sha": "e".repeat(40)
            }
        }),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
    };

    // Event must succeed despite remove_label failures.
    let result = processor.handle_merged_pull_request(&event).await;
    assert!(
        result.is_ok(),
        "expected Ok when remove_label fails, got: {result:?}"
    );

    // Cleanup comments must still be posted on PR #99 even though remove_label failed.
    let comments = github.issue_comments.lock().await;
    let cleanup_comments: Vec<_> = comments.iter().filter(|(pr, _)| *pr == 99).collect();
    assert!(
        !cleanup_comments.is_empty(),
        "cleanup comments should still be posted even when remove_label fails, got: {comments:?}"
    );
}

/// When posting the cleanup comment fails for a PR, a warning is logged but
/// the cleanup loop continues and the overall event still succeeds.
#[tokio::test]
async fn test_handle_merged_release_pr_cleanup_comment_failure_event_still_succeeds() {
    let stale_pr = make_pr(99, "feat/stale-feature", "feat: stale work");
    let github = TestGitHubForLib::new_empty()
        .with_search_results(vec![stale_pr])
        .with_fail_comment_for_pr(99); // comment on PR #99 will fail

    let config = TestConfigForLib;
    let version_calc = TestVersionCalcForLib::returning("1.1.0");
    let processor = ReleaseRegentProcessor::new(github.clone(), config, version_calc);

    let event = ProcessingEvent {
        event_id: "evt-comment-fail".into(),
        correlation_id: "corr-comment-fail".into(),
        event_type: EventType::PullRequestMerged,
        repository: RepositoryInfo {
            owner: "acme".into(),
            name: "app".into(),
            default_branch: "main".into(),
        },
        payload: serde_json::json!({
            "pull_request": {
                "head": { "ref": "release/v1.1.0" },
                "base": { "ref": "main" },
                "number": 100,
                "merge_commit_sha": "g".repeat(40)
            }
        }),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
    };

    // Event must succeed despite comment posting failure.
    let result = processor.handle_merged_pull_request(&event).await;
    assert!(
        result.is_ok(),
        "expected Ok when cleanup comment fails, got: {result:?}"
    );

    // Labels should have been removed even though the comment failed.
    let removed = github.removed_labels.lock().await;
    assert!(
        !removed.is_empty(),
        "labels should still be removed even when comment posting fails, got {removed:?}"
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// handle_merged_pull_request — feature-PR path audit-comment failure (Minor #6)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// When the floor IS applied but posting the audit comment to the release PR
/// fails, a warning is logged and `Ok(orch_result)` is returned.
#[tokio::test]
async fn test_handle_merged_feature_pr_audit_comment_failure_returns_ok() {
    // Current released tag: v1.0.0  →  current_version = Some(1.0.0)
    // Version calculator produces 1.1.0 (minor bump).
    // Floor = Major  →  effective = 2.0.0, so the floor IS applied.
    // The release PR created by the orchestrator gets number 42 (from TestGitHubForLib).
    let tag = GitTag {
        name: "v1.0.0".to_string(),
        target_sha: "a".repeat(40),
        tag_type: GitTagType::Lightweight,
        message: None,
        tagger: None,
        created_at: None,
    };
    let override_label = Label {
        id: 1,
        name: "rr:override-major".to_string(),
        color: "ff0000".to_string(),
        description: None,
    };
    // Fail create_issue_comment on the release PR (number 42
    // as returned by create_pull_request in the double).
    let github = TestGitHubForLib::new_empty()
        .with_tags(vec![tag])
        .with_pr_labels(7, vec![override_label])
        .with_fail_comment_for_pr(42); // audit comment on release PR #42 will fail

    let config = TestConfigForLib;
    let version_calc = TestVersionCalcForLib::returning("1.1.0");
    let processor = ReleaseRegentProcessor::new(github.clone(), config, version_calc);

    let event = ProcessingEvent {
        event_id: "evt-audit-fail".into(),
        correlation_id: "corr-audit-fail".into(),
        event_type: EventType::PullRequestMerged,
        repository: RepositoryInfo {
            owner: "acme".into(),
            name: "app".into(),
            default_branch: "main".into(),
        },
        payload: serde_json::json!({
            "pull_request": {
                "head": { "ref": "feat/cool-feature" },
                "base": { "ref": "main" },
                "number": 7,
                "merge_commit_sha": "b".repeat(40)
            }
        }),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
    };

    // The overall result must be Ok even though the audit comment failed.
    let result = processor.handle_merged_pull_request(&event).await;
    assert!(
        result.is_ok(),
        "expected Ok when audit comment posting fails, got: {result:?}"
    );

    // The release PR must have been created at 2.0.0 (floor applied).
    let created = github.created_prs.lock().await;
    assert_eq!(created.len(), 1, "expected one release PR");
    assert!(
        created[0].0.contains("2.0.0"),
        "release PR branch should reference 2.0.0, got: {}",
        created[0].0
    );

    // No audit comment should be present (it failed).
    let comments = github.issue_comments.lock().await;
    assert!(
        comments
            .iter()
            .all(|(_, b)| !b.contains("Version floor applied")),
        "no floor audit comment should be posted when it fails, got: {comments:?}"
    );
}

/// Regression test for the bug where merging a non-version-bumping PR immediately
/// after a release recreates the just-released release branch.
///
/// Scenario:
///   1. `v0.3.0` release branch is merged → tag `v0.3.0` and GitHub release
///      are created, branch is deleted (handled by `ReleasePrMerged` path).
///   2. A `chore:` PR is merged — no `feat:` or `fix:`, so `VersionBump::None`
///      and the version calculator returns `v0.3.0` (unchanged).
///   3. Without the fix, `process_feature_pr_merged` would call the orchestrator
///      with `v0.3.0`, which would find no open release PR and create a new
///      `release/v0.3.0` branch — resurrecting a version that is already tagged.
///   4. With the fix, the handler detects that `effective_version == current_version`
///      and returns `OrchestratorResult::NoBumpNeeded` without touching GitHub.
#[tokio::test]
async fn test_handle_merged_non_bumping_feature_pr_after_release_does_not_recreate_release_branch()
{
    // Current released tag is v0.3.0.
    let tag = GitTag {
        name: "v0.3.0".to_string(),
        target_sha: "a".repeat(40),
        tag_type: GitTagType::Lightweight,
        message: None,
        tagger: None,
        created_at: None,
    };
    // The version calculator returns v0.3.0 with VersionBump::None — the
    // commits in the merged PR are all non-bumping (chore:, docs:, etc.).
    let version_calc =
        TestVersionCalcForLib::returning("0.3.0").with_version_bump(VersionBump::None);
    let github = TestGitHubForLib::new_empty().with_tags(vec![tag]);
    let config = TestConfigForLib;
    let processor = ReleaseRegentProcessor::new(github.clone(), config, version_calc);

    let event = ProcessingEvent {
        event_id: "evt-chore-after-release".into(),
        correlation_id: "corr-chore-after-release".into(),
        event_type: EventType::PullRequestMerged,
        repository: RepositoryInfo {
            owner: "acme".into(),
            name: "app".into(),
            default_branch: "main".into(),
        },
        payload: serde_json::json!({
            "pull_request": {
                // Regular feature branch — NOT a release branch.
                "head": { "ref": "chore/update-deps", "sha": "b".repeat(40) },
                "base": { "ref": "main" },
                "number": 99,
                "merge_commit_sha": "c".repeat(40)
            }
        }),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
    };

    let result = processor
        .handle_merged_pull_request(&event)
        .await
        .expect("expected Ok for non-bumping PR after release");
    // The guard is version-equality-based: effective_version == current_version
    // triggers NoBumpNeeded regardless of the VersionBump variant.  The
    // VersionBump::None field on the version_calc is incidental.
    assert!(
        matches!(
            result,
            release_orchestrator::OrchestratorResult::NoBumpNeeded
        ),
        "expected NoBumpNeeded but got: {result:?}"
    );

    // No release branch or PR should have been created.
    let created = github.created_prs.lock().await;
    assert!(
        created.is_empty(),
        "release branch must NOT be recreated for an already-released version, \
         but found PRs: {created:?}"
    );
    let branches = github.create_branch_calls.lock().await;
    assert!(
        branches.is_empty(),
        "release branch must NOT be recreated for an already-released version, \
         but found branches: {branches:?}"
    );
}

/// Variant: a non-bumping PR with a `!release patch` override AFTER a release
/// SHOULD create a new release branch for the next patch version.
///
/// The bump-floor must not be suppressed by the `NoBumpNeeded` guard because
/// the floor raises `effective_version` above `current_version`.
#[tokio::test]
async fn test_handle_merged_non_bumping_pr_with_patch_floor_creates_next_patch_release() {
    let tag = GitTag {
        name: "v0.3.0".to_string(),
        target_sha: "a".repeat(40),
        tag_type: GitTagType::Lightweight,
        message: None,
        tagger: None,
        created_at: None,
    };
    // Calculator returns v0.3.0 with None — no conventional bump from commits.
    let version_calc =
        TestVersionCalcForLib::returning("0.3.0").with_version_bump(VersionBump::None);
    let override_label = Label {
        id: 1,
        name: "rr:override-patch".to_string(),
        color: "00ff00".to_string(),
        description: None,
    };
    let github = TestGitHubForLib::new_empty()
        .with_tags(vec![tag])
        .with_pr_labels(99, vec![override_label]);
    let config = TestConfigForLib;
    let processor = ReleaseRegentProcessor::new(github.clone(), config, version_calc);

    let event = ProcessingEvent {
        event_id: "evt-patch-floor-after-release".into(),
        correlation_id: "corr-patch-floor-after-release".into(),
        event_type: EventType::PullRequestMerged,
        repository: RepositoryInfo {
            owner: "acme".into(),
            name: "app".into(),
            default_branch: "main".into(),
        },
        payload: serde_json::json!({
            "pull_request": {
                "head": { "ref": "chore/bump-floor", "sha": "b".repeat(40) },
                "base": { "ref": "main" },
                "number": 99,
                "merge_commit_sha": "c".repeat(40)
            }
        }),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
    };

    let result = processor.handle_merged_pull_request(&event).await;
    assert!(
        result.is_ok(),
        "expected Ok when patch floor applied after release, got: {result:?}"
    );

    // The orchestrator must have created a release branch for v0.3.1.
    let created = github.created_prs.lock().await;
    assert_eq!(created.len(), 1, "expected one release PR for v0.3.1");
    assert!(
        created[0].0.contains("0.3.1"),
        "release branch should reference 0.3.1, got: {}",
        created[0].0
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Phase F — PR status comment end-to-end scenarios
// ─────────────────────────────────────────────────────────────────────────────

fn make_pr_activity_payload(
    pr_number: u64,
    head_sha: &str,
    head_branch: &str,
    author_login: &str,
) -> serde_json::Value {
    serde_json::json!({
        "pull_request": {
            "number": pr_number,
            "head": { "sha": head_sha, "ref": head_branch },
            "user": { "login": author_login }
        }
    })
}

fn make_open_pr_with_author(
    number: u64,
    head_branch: &str,
    head_sha: &str,
    author_login: &str,
) -> PullRequest {
    let repo = make_repo();
    PullRequest {
        base: PullRequestBranch {
            ref_name: "main".into(),
            repo: repo.clone(),
            sha: "base000000000000000000000000000000000000".into(),
        },
        body: None,
        created_at: Utc::now(),
        draft: false,
        head: PullRequestBranch {
            ref_name: head_branch.to_string(),
            repo,
            sha: head_sha.to_string(),
        },
        merged_at: None,
        number,
        state: "open".into(),
        title: format!("PR #{number}"),
        updated_at: Utc::now(),
        user: GitUser {
            email: format!("{author_login}@example.com"),
            login: Some(author_login.to_string()),
            name: author_login.to_string(),
        },
    }
}

/// (a) `PullRequestOpened` for a feature branch → status comment created with
/// projected version; `create_issue_comment` called exactly once.
#[tokio::test]
async fn test_feature_pr_opened_creates_status_comment_with_projected_version() {
    let github = TestGitHubForLib::new_empty();
    let config = TestConfigForLib;
    let version_calc = TestVersionCalcForLib::returning("1.2.0");
    let processor = ReleaseRegentProcessor::new(github.clone(), config, version_calc);

    let event = ProcessingEvent {
        event_id: "f4-a".into(),
        correlation_id: "corr-f4-a".into(),
        event_type: EventType::PullRequestOpened,
        repository: test_repo(),
        payload: make_pr_activity_payload(42, "abc1234", "feat/my-feature", "dev"),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
    };

    processor
        .handle_pull_request_activity(&event)
        .await
        .expect("handle_pull_request_activity should succeed");

    let comments = github.issue_comments.lock().await;
    assert_eq!(comments.len(), 1, "expected exactly one comment");
    assert_eq!(comments[0].0, 42, "comment must target PR #42");
    assert!(
        comments[0].1.contains("1.2.0"),
        "comment body must contain projected version; got: {}",
        comments[0].1
    );
    assert!(
        comments[0]
            .1
            .contains(pr_status_commenter::PR_STATUS_MARKER),
        "comment must contain the status marker"
    );
}

/// (b) `PullRequestUpdated` for the same feature branch after a comment
/// already exists → `update_issue_comment` called; no duplicate comment.
#[tokio::test]
async fn test_feature_pr_updated_updates_existing_status_comment_in_place() {
    use crate::traits::github_operations::IssueComment;

    let existing_comment = IssueComment {
        id: 99,
        body: format!(
            "{}\nOld projected version: v1.1.0",
            pr_status_commenter::PR_STATUS_MARKER
        ),
        user_login: Some("release-regent[bot]".into()),
    };

    let github =
        TestGitHubForLib::new_empty().with_stored_issue_comments(42, vec![existing_comment]);
    let config = TestConfigForLib;
    let version_calc = TestVersionCalcForLib::returning("1.2.0");
    let processor = ReleaseRegentProcessor::new(github.clone(), config, version_calc);

    let event = ProcessingEvent {
        event_id: "f4-b".into(),
        correlation_id: "corr-f4-b".into(),
        event_type: EventType::PullRequestUpdated,
        repository: test_repo(),
        payload: make_pr_activity_payload(42, "abc2345", "feat/my-feature", "dev"),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
    };

    processor
        .handle_pull_request_activity(&event)
        .await
        .expect("handle_pull_request_activity should succeed");

    // update_issue_comment must have been called (not create_issue_comment).
    let updates = github.update_comment_calls.lock().await;
    assert_eq!(updates.len(), 1, "expected one update call");
    assert_eq!(updates[0].0, 99, "must update comment id 99");

    // create_issue_comment must NOT have been called.
    let creates = github.issue_comments.lock().await;
    assert!(
        creates.is_empty(),
        "create_issue_comment must not be called when updating"
    );
}

/// (c) After a feature PR merges, open PRs with an existing marker comment
/// are refreshed (max 25).
#[tokio::test]
async fn test_merged_feature_pr_refreshes_open_prs_with_existing_marker() {
    use crate::traits::github_operations::IssueComment;

    // Two open feature PRs, both with an existing marker comment.
    let pr_10 = make_open_pr_with_author(10, "feat/alpha", "sha-alpha", "alice");
    let pr_11 = make_open_pr_with_author(11, "feat/beta", "sha-beta", "bob");

    let marker_comment = IssueComment {
        id: 1,
        body: format!("{}\nOld version", pr_status_commenter::PR_STATUS_MARKER),
        user_login: None,
    };

    let github = TestGitHubForLib::new_empty()
        .with_search_results(vec![pr_10, pr_11])
        .with_stored_issue_comments(10, vec![marker_comment.clone()])
        .with_stored_issue_comments(11, vec![marker_comment]);
    let config = TestConfigForLib;
    let version_calc = TestVersionCalcForLib::returning("0.2.0");
    let processor = ReleaseRegentProcessor::new(github.clone(), config, version_calc);

    // Build a merged-PR event
    let event = ProcessingEvent {
        event_id: "f4-c".into(),
        correlation_id: "corr-f4-c".into(),
        event_type: EventType::PullRequestMerged,
        repository: test_repo(),
        payload: serde_json::json!({
            "pull_request": {
                "number": 5,
                "base": { "ref": "main" },
                "merge_commit_sha": "a".repeat(40),
                "head": { "sha": "a".repeat(40), "ref": "feat/update" }
            }
        }),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
    };

    MergedPullRequestHandler::handle_merged_pull_request(&processor, &event)
        .await
        .expect("handle_merged_pull_request should succeed");

    // Both open PRs should have received a refreshed comment (via update since
    // they already had a marker comment).
    let updates = github.update_comment_calls.lock().await;
    assert_eq!(
        updates.len(),
        2,
        "both open PRs with marker comment must be refreshed"
    );
}

/// (d) `PullRequestOpened` for a release branch → status comment shows the
/// release version extracted from the branch name.
#[tokio::test]
async fn test_release_pr_opened_creates_status_comment_with_release_version() {
    let github = TestGitHubForLib::new_empty();
    let config = TestConfigForLib;
    let version_calc = TestVersionCalcForLib::returning("2.0.0"); // should NOT be used
    let processor = ReleaseRegentProcessor::new(github.clone(), config, version_calc);

    let event = ProcessingEvent {
        event_id: "f4-d".into(),
        correlation_id: "corr-f4-d".into(),
        event_type: EventType::PullRequestOpened,
        repository: test_repo(),
        payload: make_pr_activity_payload(55, "deadbeef", "release/v1.5.0", "release-bot"),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
    };

    processor
        .handle_pull_request_activity(&event)
        .await
        .expect("handle_pull_request_activity should succeed");

    let comments = github.issue_comments.lock().await;
    assert_eq!(comments.len(), 1, "expected exactly one comment");
    assert_eq!(comments[0].0, 55);
    assert!(
        comments[0].1.contains("1.5.0"),
        "comment must show release version 1.5.0; got: {}",
        comments[0].1
    );
    assert!(
        comments[0].1.contains("release PR"),
        "comment must identify itself as a release PR comment"
    );
}

/// (e) `allow_override: false` → no `### Available commands` section in
/// either feature or release PR status comments.
#[tokio::test]
async fn test_pr_status_comment_omits_commands_when_allow_override_is_false() {
    let mut cfg = config::ReleaseRegentConfig::default();
    cfg.versioning.allow_override = false;

    let github = TestGitHubForLib::new_empty();
    let config = TestConfigWith::new(cfg);
    let version_calc = TestVersionCalcForLib::returning("1.1.0");
    let processor = ReleaseRegentProcessor::new(github.clone(), config, version_calc);

    // Feature PR
    let feature_event = ProcessingEvent {
        event_id: "f4-e-feat".into(),
        correlation_id: "corr-f4-e-feat".into(),
        event_type: EventType::PullRequestOpened,
        repository: test_repo(),
        payload: make_pr_activity_payload(10, "sha1", "feat/x", "user1"),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
    };

    processor
        .handle_pull_request_activity(&feature_event)
        .await
        .expect("feature PR activity should succeed");

    // Release PR
    let release_event = ProcessingEvent {
        event_id: "f4-e-rel".into(),
        correlation_id: "corr-f4-e-rel".into(),
        event_type: EventType::PullRequestOpened,
        repository: test_repo(),
        payload: make_pr_activity_payload(20, "sha2", "release/v1.1.0", "release-bot"),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
    };

    processor
        .handle_pull_request_activity(&release_event)
        .await
        .expect("release PR activity should succeed");

    let comments = github.issue_comments.lock().await;
    assert_eq!(comments.len(), 2);
    for (_, body) in comments.iter() {
        assert!(
            !body.contains("### Available commands"),
            "commands section must be absent when allow_override is false; got: {body}"
        );
    }
}

/// (f) `PullRequestOpened` from a login in `excluded_pr_authors` → no
/// comment posted.
#[tokio::test]
async fn test_feature_pr_from_excluded_author_skips_status_comment() {
    let mut cfg = config::ReleaseRegentConfig::default();
    cfg.versioning
        .excluded_pr_authors
        .push("dependabot[bot]".to_string());

    let github = TestGitHubForLib::new_empty();
    let config = TestConfigWith::new(cfg);
    let version_calc = TestVersionCalcForLib::returning("1.0.1");
    let processor = ReleaseRegentProcessor::new(github.clone(), config, version_calc);

    let event = ProcessingEvent {
        event_id: "f4-f".into(),
        correlation_id: "corr-f4-f".into(),
        event_type: EventType::PullRequestOpened,
        repository: test_repo(),
        payload: make_pr_activity_payload(77, "sha-bot", "deps/update", "dependabot[bot]"),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
    };

    processor
        .handle_pull_request_activity(&event)
        .await
        .expect("excluded author should be handled cleanly");

    // No comment must be posted.
    let comments = github.issue_comments.lock().await;
    assert!(
        comments.is_empty(),
        "no comment must be posted for excluded author"
    );
}

/// (g) Batch refresh after merge: excluded-author PR skipped; normal PR with
/// marker comment is refreshed.
#[tokio::test]
async fn test_batch_refresh_skips_excluded_author_and_refreshes_normal_pr() {
    use crate::traits::github_operations::IssueComment;

    let mut cfg = config::ReleaseRegentConfig::default();
    cfg.versioning
        .excluded_pr_authors
        .push("bot-account".to_string());

    let bot_pr = make_open_pr_with_author(30, "deps/bump", "sha-bot", "bot-account");
    let normal_pr = make_open_pr_with_author(31, "feat/real-feature", "sha-real", "alice");

    let marker_comment = IssueComment {
        id: 50,
        body: format!("{}\nOld version", pr_status_commenter::PR_STATUS_MARKER),
        user_login: None,
    };

    let github = TestGitHubForLib::new_empty()
        .with_search_results(vec![bot_pr, normal_pr])
        // Both PRs have an existing marker comment to ensure the only reason
        // the bot PR is skipped is the excluded-author filter.
        .with_stored_issue_comments(30, vec![marker_comment.clone()])
        .with_stored_issue_comments(31, vec![marker_comment]);
    let config = TestConfigWith::new(cfg);
    let version_calc = TestVersionCalcForLib::returning("0.2.0");
    let processor = ReleaseRegentProcessor::new(github.clone(), config, version_calc);

    let event = ProcessingEvent {
        event_id: "f4-g".into(),
        correlation_id: "corr-f4-g".into(),
        event_type: EventType::PullRequestMerged,
        repository: test_repo(),
        payload: serde_json::json!({
            "pull_request": {
                "number": 7,
                "base": { "ref": "main" },
                "merge_commit_sha": "b".repeat(40),
                "head": { "sha": "b".repeat(40), "ref": "feat/something" }
            }
        }),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
    };

    MergedPullRequestHandler::handle_merged_pull_request(&processor, &event)
        .await
        .expect("handle_merged_pull_request should succeed");

    // Only the normal PR (id 31) should be refreshed.
    let updates = github.update_comment_calls.lock().await;
    assert_eq!(updates.len(), 1, "only one PR must be refreshed");
    assert_eq!(updates[0].0, 50, "must update the normal PR's comment");
}

/// (h) When an open release PR exists, the feature PR comment includes the
/// queued-release annotation.  This test guards the wildcard fix: the search
/// query must use "head:release/v*" (prefix match) not "head:release/v"
/// (exact match), otherwise no release branch would ever match the query and
/// `queued_release_version` would always be `None`.
#[tokio::test]
async fn test_feature_pr_opened_with_queued_release_includes_annotation() {
    use crate::pr_status_commenter::PR_STATUS_MARKER;

    // seed an open release PR so search_pull_requests returns it
    let release_pr = make_open_pr_with_author(99, "release/v1.3.0", "sha-release", "release-bot");

    let github = TestGitHubForLib::new_empty().with_search_results(vec![release_pr]);
    let config = TestConfigForLib;
    // version_calc returns 1.3.0 — same as already-queued release
    let version_calc = TestVersionCalcForLib::returning("1.3.0");
    let processor = ReleaseRegentProcessor::new(github.clone(), config, version_calc);

    let event = ProcessingEvent {
        event_id: "f4-h".into(),
        correlation_id: "corr-f4-h".into(),
        event_type: EventType::PullRequestOpened,
        repository: test_repo(),
        payload: make_pr_activity_payload(42, "abc1234", "feat/my-feature", "dev"),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
    };

    processor
        .handle_pull_request_activity(&event)
        .await
        .expect("handle_pull_request_activity should succeed");

    let comments = github.issue_comments.lock().await;
    assert_eq!(comments.len(), 1, "expected exactly one comment");
    let body = &comments[0].1;

    assert!(
        body.contains(PR_STATUS_MARKER),
        "comment must contain the status marker; got: {body}"
    );
    assert!(
        body.contains("1.3.0"),
        "comment must mention the queued release version; got: {body}"
    );
    assert!(
        body.contains("already open"),
        "comment must note the queued release PR is open; got: {body}"
    );
}
