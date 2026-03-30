use super::*;
use crate::{
    traits::{
        git_operations::{
            GetCommitsOptions, GitCommit, GitOperations, GitRepository, GitTag, GitTagType,
            ListTagsOptions,
        },
        github_operations::{
            CollaboratorPermission, CreatePullRequestParams, CreateReleaseParams, GitHubOperations,
            GitUser as GitHubUser, PullRequest, PullRequestBranch, Release, Repository, Tag,
            UpdateReleaseParams,
        },
    },
    CoreError,
};
use async_trait::async_trait;
use chrono::Utc;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

// ─────────────────────────────────────────────────────────────────────────────
// Inline test double
//
// Defined locally to avoid the E0277 cross-crate blanket-impl issue documented
// in the project's "Rules & Tips".
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct TestState {
    /// Tags returned by `list_tags` (drives `resolve_current_version`).
    tags: Vec<GitTag>,
    /// PRs keyed by number (for `get_pull_request`).
    prs_by_number: HashMap<u64, PullRequest>,
    /// PRs returned by `search_pull_requests` (drives `ReleaseOrchestrator`).
    search_results: Vec<PullRequest>,
    /// Whether the next `create_branch` call should return `CoreError::Conflict`.
    next_create_branch_conflict: bool,
    /// Recorded `create_issue_comment` calls: `(issue_number, body)`.
    created_issue_comments: Vec<(u64, String)>,
    /// Recorded `create_pull_request` calls.
    created_prs: Vec<CreatePullRequestParams>,
    /// Recorded `create_branch` calls: `(branch_name, sha)`.
    created_branches: Vec<(String, String)>,
    /// Sequential PR number returned by `create_pull_request`.
    next_pr_number: u64,
    /// Collaborator permission returned by `get_collaborator_permission`.
    /// `None` defaults to `CollaboratorPermission::Write`.
    commenter_permission: Option<CollaboratorPermission>,
}

#[derive(Clone, Default)]
struct TestGitHub {
    state: Arc<Mutex<TestState>>,
}

impl TestGitHub {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(TestState {
                next_pr_number: 200,
                ..Default::default()
            })),
        }
    }

    /// Pre-load tags that `resolve_current_version` will return.
    async fn with_tags(self, tags: Vec<GitTag>) -> Self {
        self.state.lock().await.tags = tags;
        self
    }

    /// Pre-load a PR for `get_pull_request`.
    async fn with_pr(self, pr: PullRequest) -> Self {
        self.state.lock().await.prs_by_number.insert(pr.number, pr);
        self
    }

    /// Pre-load search results for `ReleaseOrchestrator::search_for_existing_release_pr`.
    async fn with_search_results(self, prs: Vec<PullRequest>) -> Self {
        self.state.lock().await.search_results = prs;
        self
    }

    /// Make the next `create_branch` call fail with `CoreError::Conflict`.
    async fn with_next_create_branch_conflict(self) -> Self {
        self.state.lock().await.next_create_branch_conflict = true;
        self
    }

    /// Pre-set the collaborator permission returned for any username.
    async fn with_commenter_permission(self, permission: CollaboratorPermission) -> Self {
        self.state.lock().await.commenter_permission = Some(permission);
        self
    }

    async fn issue_comments(&self) -> Vec<(u64, String)> {
        self.state.lock().await.created_issue_comments.clone()
    }

    async fn created_prs(&self) -> Vec<CreatePullRequestParams> {
        self.state.lock().await.created_prs.clone()
    }
}

// ── GitOperations stub impl ──────────────────────────────────────────────────

#[async_trait]
impl GitOperations for TestGitHub {
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
        Ok(self.state.lock().await.tags.clone())
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

    async fn get_repository_info(&self, _owner: &str, _repo: &str) -> CoreResult<GitRepository> {
        Err(CoreError::not_found("stub"))
    }
}

// ── GitHubOperations impl ────────────────────────────────────────────────────

#[async_trait]
impl GitHubOperations for TestGitHub {
    async fn create_pull_request(
        &self,
        owner: &str,
        repo: &str,
        params: CreatePullRequestParams,
    ) -> CoreResult<PullRequest> {
        let mut st = self.state.lock().await;
        let number = st.next_pr_number;
        st.next_pr_number += 1;
        st.created_prs.push(params.clone());
        let r = stub_repo(owner, repo);
        let now = Utc::now();
        Ok(PullRequest {
            number,
            title: params.title,
            body: params.body,
            state: "open".to_string(),
            draft: false,
            created_at: now,
            updated_at: now,
            merged_at: None,
            user: stub_user(),
            head: PullRequestBranch {
                ref_name: params.head,
                sha: "aaaa".to_string(),
                repo: r.clone(),
            },
            base: PullRequestBranch {
                ref_name: params.base,
                sha: "bbbb".to_string(),
                repo: r,
            },
        })
    }

    async fn create_release(
        &self,
        _owner: &str,
        _repo: &str,
        _params: CreateReleaseParams,
    ) -> CoreResult<Release> {
        Err(CoreError::not_supported("create_release", "stub"))
    }

    async fn create_tag(
        &self,
        _owner: &str,
        _repo: &str,
        _tag_name: &str,
        _sha: &str,
        _message: Option<String>,
        _tagger: Option<GitHubUser>,
    ) -> CoreResult<Tag> {
        Err(CoreError::not_supported("create_tag", "stub"))
    }

    async fn get_latest_release(&self, _owner: &str, _repo: &str) -> CoreResult<Option<Release>> {
        Ok(None)
    }

    async fn get_pull_request(
        &self,
        _owner: &str,
        _repo: &str,
        pr_number: u64,
    ) -> CoreResult<PullRequest> {
        let st = self.state.lock().await;
        st.prs_by_number
            .get(&pr_number)
            .cloned()
            .ok_or_else(|| CoreError::not_found(format!("PR #{pr_number}")))
    }

    async fn get_release_by_tag(
        &self,
        _owner: &str,
        _repo: &str,
        _tag: &str,
    ) -> CoreResult<Release> {
        Err(CoreError::not_found("stub"))
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
        Ok(vec![])
    }

    async fn search_pull_requests(
        &self,
        _owner: &str,
        _repo: &str,
        _query: &str,
    ) -> CoreResult<Vec<PullRequest>> {
        Ok(self.state.lock().await.search_results.clone())
    }

    async fn update_pull_request(
        &self,
        _owner: &str,
        _repo: &str,
        pr_number: u64,
        title: Option<String>,
        body: Option<String>,
        state: Option<String>,
    ) -> CoreResult<PullRequest> {
        let now = Utc::now();
        let r = stub_repo("test", "repo");
        Ok(PullRequest {
            number: pr_number,
            title: title.unwrap_or_else(|| "updated".to_string()),
            body,
            state: state.unwrap_or_else(|| "open".to_string()),
            draft: false,
            created_at: now,
            updated_at: now,
            merged_at: None,
            user: stub_user(),
            head: PullRequestBranch {
                ref_name: "release/v0.0.0".to_string(),
                sha: "aaaa".to_string(),
                repo: r.clone(),
            },
            base: PullRequestBranch {
                ref_name: "main".to_string(),
                sha: "bbbb".to_string(),
                repo: r,
            },
        })
    }

    async fn update_release(
        &self,
        _owner: &str,
        _repo: &str,
        _release_id: u64,
        _params: UpdateReleaseParams,
    ) -> CoreResult<Release> {
        Err(CoreError::not_supported("update_release", "stub"))
    }

    async fn create_branch(
        &self,
        _owner: &str,
        _repo: &str,
        branch_name: &str,
        sha: &str,
    ) -> CoreResult<()> {
        let mut st = self.state.lock().await;
        if st.next_create_branch_conflict {
            st.next_create_branch_conflict = false;
            return Err(CoreError::conflict(format!(
                "branch '{branch_name}' already exists"
            )));
        }
        st.created_branches
            .push((branch_name.to_string(), sha.to_string()));
        Ok(())
    }

    async fn delete_branch(&self, _owner: &str, _repo: &str, _branch_name: &str) -> CoreResult<()> {
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
            .created_issue_comments
            .push((issue_number, body.to_string()));
        Ok(())
    }

    async fn get_collaborator_permission(
        &self,
        _owner: &str,
        _repo: &str,
        _username: &str,
    ) -> CoreResult<CollaboratorPermission> {
        Ok(self
            .state
            .lock()
            .await
            .commenter_permission
            .clone()
            .unwrap_or(CollaboratorPermission::Write))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn stub_repo(owner: &str, repo: &str) -> Repository {
    Repository {
        id: 1,
        name: repo.to_string(),
        full_name: format!("{owner}/{repo}"),
        owner: owner.to_string(),
        description: None,
        private: false,
        default_branch: "main".to_string(),
        clone_url: format!("https://github.com/{owner}/{repo}.git"),
        ssh_url: format!("git@github.com:{owner}/{repo}.git"),
        homepage: None,
    }
}

fn stub_user() -> GitHubUser {
    GitHubUser {
        name: "test-user".to_string(),
        email: "test@example.com".to_string(),
        login: Some("test-user".to_string()),
    }
}

fn make_open_pr(number: u64, base_sha: &str) -> PullRequest {
    let now = Utc::now();
    let r = stub_repo("acme", "app");
    PullRequest {
        number,
        title: "feat: add feature".to_string(),
        body: None,
        state: "open".to_string(),
        draft: false,
        created_at: now,
        updated_at: now,
        merged_at: None,
        user: stub_user(),
        head: PullRequestBranch {
            ref_name: "feat/my-feature".to_string(),
            sha: "headsha".to_string(),
            repo: stub_repo("acme", "app"),
        },
        base: PullRequestBranch {
            ref_name: "main".to_string(),
            sha: base_sha.to_string(),
            repo: r,
        },
    }
}

fn make_semver_tag(name: &str) -> GitTag {
    GitTag {
        name: name.to_string(),
        target_sha: "0000000000000000000000000000000000000001".to_string(),
        tag_type: GitTagType::Lightweight,
        message: None,
        tagger: None,
        created_at: None,
    }
}

fn test_event(pr_number: u64, comment_body: &str, pr_state: &str) -> ProcessingEvent {
    use crate::traits::event_source::{EventSourceKind, EventType, RepositoryInfo};
    ProcessingEvent {
        event_id: "evt-001".to_string(),
        correlation_id: "corr-001".to_string(),
        event_type: EventType::PullRequestCommentReceived,
        repository: RepositoryInfo {
            owner: "acme".to_string(),
            name: "app".to_string(),
            default_branch: "main".to_string(),
        },
        payload: serde_json::json!({
            "issue": {
                "number": pr_number,
                "state": pr_state
            },
            "comment": {
                "body": comment_body,
                "user": {
                    "login": "test-user"
                }
            }
        }),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
    }
}

fn default_config(allow_override: bool) -> CommentCommandConfig {
    CommentCommandConfig {
        orchestrator_config: crate::release_orchestrator::OrchestratorConfig::default(),
        allow_override,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// parse_comment_command unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_parse_comment_command_set_version_valid_semver() {
    use crate::versioning::SemanticVersion;

    let cmd = parse_comment_command("!set-version 2.3.0");
    assert_eq!(
        cmd,
        CommentCommand::SetVersion(SemanticVersion {
            major: 2,
            minor: 3,
            patch: 0,
            prerelease: None,
            build: None,
        })
    );
}

#[test]
fn test_parse_comment_command_set_version_with_v_prefix() {
    use crate::versioning::SemanticVersion;

    let cmd = parse_comment_command("!set-version v1.0.0");
    assert_eq!(
        cmd,
        CommentCommand::SetVersion(SemanticVersion {
            major: 1,
            minor: 0,
            patch: 0,
            prerelease: None,
            build: None,
        })
    );
}

#[test]
fn test_parse_comment_command_set_version_case_insensitive() {
    use crate::versioning::SemanticVersion;

    let cmd = parse_comment_command("!SET-VERSION 3.1.4");
    assert_eq!(
        cmd,
        CommentCommand::SetVersion(SemanticVersion {
            major: 3,
            minor: 1,
            patch: 4,
            prerelease: None,
            build: None,
        })
    );
}

#[test]
fn test_parse_comment_command_set_version_with_surrounding_text_on_same_line() {
    // The command must appear at the start of the trimmed line.
    // "Please !set-version 2.0.0" does NOT start with !set-version after trim.
    let cmd = parse_comment_command("Please !set-version 2.0.0");
    assert_eq!(cmd, CommentCommand::Unknown);
}

#[test]
fn test_parse_comment_command_set_version_on_second_line() {
    use crate::versioning::SemanticVersion;

    let body = "Some description.\n!set-version 1.5.0\nMore text.";
    let cmd = parse_comment_command(body);
    assert_eq!(
        cmd,
        CommentCommand::SetVersion(SemanticVersion {
            major: 1,
            minor: 5,
            patch: 0,
            prerelease: None,
            build: None,
        })
    );
}

#[test]
fn test_parse_comment_command_set_version_malformed_version_returns_unknown() {
    // "!set-version notaversion" has an invalid semver string, so it should
    // continue scanning and ultimately return Unknown.
    let cmd = parse_comment_command("!set-version notaversion");
    assert_eq!(cmd, CommentCommand::Unknown);
}

#[test]
fn test_parse_comment_command_release_major() {
    let cmd = parse_comment_command("!release major");
    assert_eq!(cmd, CommentCommand::ReleaseBump(BumpKind::Major));
}

#[test]
fn test_parse_comment_command_release_minor() {
    let cmd = parse_comment_command("!release minor");
    assert_eq!(cmd, CommentCommand::ReleaseBump(BumpKind::Minor));
}

#[test]
fn test_parse_comment_command_release_patch() {
    let cmd = parse_comment_command("!release patch");
    assert_eq!(cmd, CommentCommand::ReleaseBump(BumpKind::Patch));
}

#[test]
fn test_parse_comment_command_release_case_insensitive() {
    let cmd = parse_comment_command("!RELEASE MINOR");
    assert_eq!(cmd, CommentCommand::ReleaseBump(BumpKind::Minor));
}

#[test]
fn test_parse_comment_command_release_unknown_dimension_returns_unknown() {
    let cmd = parse_comment_command("!release huge");
    assert_eq!(cmd, CommentCommand::Unknown);
}

#[test]
fn test_parse_comment_command_unknown_returns_unknown_variant() {
    assert_eq!(
        parse_comment_command("just a regular comment"),
        CommentCommand::Unknown
    );
    assert_eq!(parse_comment_command(""), CommentCommand::Unknown);
    assert_eq!(parse_comment_command("   "), CommentCommand::Unknown);
    assert_eq!(parse_comment_command("lgtm"), CommentCommand::Unknown);
}

// ─────────────────────────────────────────────────────────────────────────────
// CommentCommandProcessor::process unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_process_allow_override_false_makes_no_github_calls() {
    let github = TestGitHub::new();
    let processor = CommentCommandProcessor::new(default_config(false), &github);

    // Any command in the body — but allow_override is false, so nothing happens.
    let event = test_event(42, "!set-version 2.0.0", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok());
    assert!(github.issue_comments().await.is_empty());
    assert!(github.created_prs().await.is_empty());
}

#[tokio::test]
async fn test_process_unrecognised_comment_body_is_noop() {
    let github = TestGitHub::new().with_pr(make_open_pr(42, "abc123")).await;
    let processor = CommentCommandProcessor::new(default_config(true), &github);

    let event = test_event(42, "just a regular comment, no commands here", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok());
    assert!(github.issue_comments().await.is_empty());
    assert!(github.created_prs().await.is_empty());
}

#[tokio::test]
async fn test_process_empty_comment_body_is_noop() {
    let github = TestGitHub::new().with_pr(make_open_pr(42, "abc123")).await;
    let processor = CommentCommandProcessor::new(default_config(true), &github);

    let event = test_event(42, "", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok());
    assert!(github.issue_comments().await.is_empty());
}

#[tokio::test]
async fn test_process_comment_on_closed_pr_is_noop() {
    let github = TestGitHub::new();
    let processor = CommentCommandProcessor::new(default_config(true), &github);

    // PR state is "closed" — should be silently ignored.
    let event = test_event(42, "!set-version 2.0.0", "closed");
    let result = processor.process(&event).await;

    assert!(result.is_ok());
    assert!(github.issue_comments().await.is_empty());
    assert!(github.created_prs().await.is_empty());
}

#[tokio::test]
async fn test_process_comment_on_merged_pr_is_noop() {
    let github = TestGitHub::new();
    let processor = CommentCommandProcessor::new(default_config(true), &github);

    let event = test_event(42, "!set-version 2.0.0", "merged");
    let result = processor.process(&event).await;

    assert!(result.is_ok());
    assert!(github.issue_comments().await.is_empty());
}

#[tokio::test]
async fn test_process_set_version_accepted_when_greater_than_current_released_tag() {
    // Current released version: v1.0.0 (from tags)
    // Pin: 2.0.0 — strictly greater → should trigger orchestration.
    let github = TestGitHub::new()
        .with_tags(vec![make_semver_tag("v1.0.0")])
        .await
        .with_pr(make_open_pr(42, "deadbeef"))
        .await;

    let processor = CommentCommandProcessor::new(default_config(true), &github);
    let event = test_event(42, "!set-version 2.0.0", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok());
    // No rejection comment should have been posted.
    assert!(github.issue_comments().await.is_empty());
    // The orchestrator should have created a new release PR.
    let prs = github.created_prs().await;
    assert_eq!(prs.len(), 1);
    assert!(prs[0].head.starts_with("release/v2.0.0"));
}

#[tokio::test]
async fn test_process_set_version_accepted_when_no_existing_tags_first_release_path() {
    // No existing tags (first release) → any valid semver ≥ 0.0.1 is accepted.
    let github = TestGitHub::new()
        .with_tags(vec![])
        .await
        .with_pr(make_open_pr(42, "deadbeef"))
        .await;

    let processor = CommentCommandProcessor::new(default_config(true), &github);
    // Skip straight to 1.0.0 — a valid first release.
    let event = test_event(42, "!set-version 1.0.0", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok());
    assert!(github.issue_comments().await.is_empty());
    let prs = github.created_prs().await;
    assert_eq!(prs.len(), 1);
    assert!(prs[0].head.starts_with("release/v1.0.0"));
}

#[tokio::test]
async fn test_process_set_version_accepted_when_no_tags_and_version_is_zero_zero_one() {
    // 0.0.1 is the absolute minimum; should succeed when no tags exist.
    let github = TestGitHub::new()
        .with_tags(vec![])
        .await
        .with_pr(make_open_pr(42, "deadbeef"))
        .await;

    let processor = CommentCommandProcessor::new(default_config(true), &github);
    let event = test_event(42, "!set-version 0.0.1", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok());
    assert!(github.issue_comments().await.is_empty());
}

#[tokio::test]
async fn test_process_set_version_rejected_when_equal_to_current_tag_posts_rejection_comment() {
    // Pinned == current released → rejected.
    let github = TestGitHub::new()
        .with_tags(vec![make_semver_tag("v1.0.0")])
        .await;

    let processor = CommentCommandProcessor::new(default_config(true), &github);
    let event = test_event(42, "!set-version 1.0.0", "open");
    let result = processor.process(&event).await;

    // Should return Ok (acknowledged, not retried).
    assert!(result.is_ok());
    let comments = github.issue_comments().await;
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].0, 42);
    assert!(comments[0].1.contains("rejected"));
    assert!(comments[0].1.contains("1.0.0"));
    // No PR should have been created.
    assert!(github.created_prs().await.is_empty());
}

#[tokio::test]
async fn test_process_set_version_rejected_when_lower_than_current_tag_posts_rejection_comment() {
    // Pinned < current released → rejected.
    let github = TestGitHub::new()
        .with_tags(vec![make_semver_tag("v2.0.0")])
        .await;

    let processor = CommentCommandProcessor::new(default_config(true), &github);
    let event = test_event(42, "!set-version 1.5.0", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok());
    let comments = github.issue_comments().await;
    assert_eq!(comments.len(), 1);
    assert!(comments[0].1.contains("rejected"));
    assert!(github.created_prs().await.is_empty());
}

#[tokio::test]
async fn test_process_set_version_zero_zero_zero_rejected_when_no_tags() {
    // 0.0.0 is below the minimum (0.0.1) even with no existing tags.
    let github = TestGitHub::new().with_tags(vec![]).await;

    let processor = CommentCommandProcessor::new(default_config(true), &github);
    let event = test_event(42, "!set-version 0.0.0", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok());
    let comments = github.issue_comments().await;
    assert_eq!(comments.len(), 1);
    assert!(comments[0].1.contains("rejected"));
    assert!(github.created_prs().await.is_empty());
}

#[tokio::test]
async fn test_process_release_bump_posts_informational_comment_and_acknowledges() {
    // !release bump commands are a stub until the design is completed.
    // The user should receive an informational comment and the event is acknowledged (Ok).
    let github = TestGitHub::new();
    let processor = CommentCommandProcessor::new(default_config(true), &github);

    let event = test_event(42, "!release major", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok(), "expected Ok, got: {result:?}");
    let comments = github.issue_comments().await;
    assert_eq!(comments.len(), 1, "expected one informational comment");
    assert_eq!(comments[0].0, 42);
    assert!(
        comments[0].1.contains("not yet"),
        "comment should mention feature is not yet available: {}",
        comments[0].1
    );
    // No release PR should have been created.
    assert!(github.created_prs().await.is_empty());
}

#[tokio::test]
async fn test_process_release_bump_minor_also_posts_informational_comment() {
    let github = TestGitHub::new();
    let processor = CommentCommandProcessor::new(default_config(true), &github);

    let event = test_event(42, "!release minor", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok());
    let comments = github.issue_comments().await;
    assert_eq!(comments.len(), 1);
    assert!(comments[0].1.contains("not yet"));
}

#[tokio::test]
async fn test_process_repeated_calls_both_succeed() {
    // Processing !set-version 1.5.0 twice: both calls should succeed and
    // trigger orchestration. In production the second call would find the
    // existing branch/PR and update it (orchestrator idempotency); here both
    // create a new PR in the test double.
    let github = TestGitHub::new()
        .with_tags(vec![make_semver_tag("v1.0.0")])
        .await
        .with_pr(make_open_pr(42, "deadbeef"))
        .await;

    let processor = CommentCommandProcessor::new(default_config(true), &github);
    let event = test_event(42, "!set-version 1.5.0", "open");

    let r1 = processor.process(&event).await;
    let r2 = processor.process(&event).await;

    assert!(r1.is_ok());
    assert!(r2.is_ok());
    // Both calls should have invoked orchestration.
    assert_eq!(github.created_prs().await.len(), 2);
}

#[tokio::test]
async fn test_process_unauthorized_commenter_gets_rejection_comment() {
    // A read-only collaborator may not issue commands. The processor should
    // post a rejection comment and return Ok (acknowledged, not retried).
    let github = TestGitHub::new()
        .with_commenter_permission(CollaboratorPermission::Read)
        .await;
    let processor = CommentCommandProcessor::new(default_config(true), &github);

    let event = test_event(42, "!set-version 2.0.0", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok(), "expected Ok, got: {result:?}");
    let comments = github.issue_comments().await;
    assert_eq!(comments.len(), 1, "expected one rejection comment");
    assert_eq!(comments[0].0, 42);
    assert!(
        comments[0].1.contains("write access"),
        "rejection should mention write access, got: {}",
        comments[0].1
    );
    // No release PR should have been created.
    assert!(github.created_prs().await.is_empty());
}

#[tokio::test]
async fn test_process_triage_collaborator_gets_rejection_comment() {
    // Triage collaborators cannot push; they must also not be able to issue commands.
    let github = TestGitHub::new()
        .with_commenter_permission(CollaboratorPermission::Triage)
        .await;
    let processor = CommentCommandProcessor::new(default_config(true), &github);

    let event = test_event(42, "!set-version 2.0.0", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok(), "expected Ok, got: {result:?}");
    let comments = github.issue_comments().await;
    assert_eq!(comments.len(), 1, "expected one rejection comment");
    assert!(comments[0].1.contains("write access"));
    assert!(github.created_prs().await.is_empty());
}

#[tokio::test]
async fn test_process_no_access_user_gets_rejection_comment() {
    // A user with no repository access must be rejected.
    let github = TestGitHub::new()
        .with_commenter_permission(CollaboratorPermission::None)
        .await;
    let processor = CommentCommandProcessor::new(default_config(true), &github);

    let event = test_event(42, "!set-version 2.0.0", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok(), "expected Ok, got: {result:?}");
    let comments = github.issue_comments().await;
    assert_eq!(comments.len(), 1, "expected one rejection comment");
    assert!(comments[0].1.contains("write access"));
    assert!(github.created_prs().await.is_empty());
}

#[tokio::test]
async fn test_process_release_bare_no_argument_is_noop() {
    // `!release` typed alone (no major/minor/patch) parses as Unknown —
    // just silently acknowledge with no comment or PR creation.
    let github = TestGitHub::new();
    let processor = CommentCommandProcessor::new(default_config(true), &github);

    let event = test_event(42, "!release", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok());
    assert!(github.issue_comments().await.is_empty());
    assert!(github.created_prs().await.is_empty());
}
