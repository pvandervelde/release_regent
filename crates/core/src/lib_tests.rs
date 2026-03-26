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
}

impl SpyMergedPRHandler {
    fn new() -> Self {
        Self::default()
    }

    async fn received_event_ids(&self) -> Vec<String> {
        self.received.lock().await.clone()
    }
}

#[async_trait]
impl MergedPullRequestHandler for SpyMergedPRHandler {
    async fn handle_merged_pull_request(&self, event: &ProcessingEvent) -> CoreResult<()> {
        self.received.lock().await.push(event.event_id.clone());
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
        CreatePullRequestParams, CreateReleaseParams, GitHubOperations, GitUser, PullRequest,
        PullRequestBranch, Release, Repository, Tag, UpdateReleaseParams,
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
    created_prs: Arc<Mutex<Vec<(String, String, String)>>>, // (branch, title, body)
    create_branch_calls: Arc<Mutex<Vec<String>>>,
}

impl TestGitHubForLib {
    fn new_empty() -> Self {
        Self {
            tags: vec![],
            existing_prs: vec![],
            created_prs: Arc::new(Mutex::new(vec![])),
            create_branch_calls: Arc::new(Mutex::new(vec![])),
        }
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
        _query: &str,
    ) -> CoreResult<Vec<PullRequest>> {
        Ok(self.existing_prs.clone())
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

// ── TestVersionCalcForLib ───────────────────────────────────────────────────

#[derive(Clone)]
struct TestVersionCalcForLib {
    next_version: SemanticVersion,
    changelog_entries: Vec<ChangelogEntry>,
}

impl TestVersionCalcForLib {
    fn returning(version: &str) -> Self {
        Self {
            next_version: versioning::VersionCalculator::parse_version(version).unwrap(),
            changelog_entries: vec![],
        }
    }

    fn with_entries(mut self, entries: Vec<ChangelogEntry>) -> Self {
        self.changelog_entries = entries;
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
            version_bump: VersionBump::Minor,
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
