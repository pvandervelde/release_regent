use super::*;
use crate::{
    traits::{
        git_operations::{
            GetCommitsOptions, GitCommit, GitOperations, GitRepository, GitTag, GitTagType,
            ListTagsOptions,
        },
        github_operations::{
            CollaboratorPermission, CreatePullRequestParams, CreateReleaseParams, GitHubOperations,
            GitUser as GitHubUser, Label, PullRequest, PullRequestBranch, Release, Repository, Tag,
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
    /// When `true`, `search_pull_requests` returns a `CoreError::Network` error.
    search_returns_error: bool,
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
    /// Labels keyed by PR/issue number for `list_pr_labels`.
    pr_labels: HashMap<u64, Vec<Label>>,
    /// When `true`, the next `add_labels` call returns a `CoreError::GitHub` error.
    next_add_labels_error: bool,
    /// When `true`, every `remove_label` call returns a `CoreError::Network` error.
    all_remove_label_network_error: bool,
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
                pr_labels: HashMap::new(),
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

    /// Make all `search_pull_requests` calls return a network error.
    async fn with_search_error(self) -> Self {
        self.state.lock().await.search_returns_error = true;
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

    /// Pre-populate labels for a specific PR/issue number.
    async fn with_pr_labels(self, pr_number: u64, labels: Vec<Label>) -> Self {
        self.state.lock().await.pr_labels.insert(pr_number, labels);
        self
    }

    /// Make the next `add_labels` call return a `CoreError::GitHub` error.
    async fn with_next_add_labels_error(self) -> Self {
        self.state.lock().await.next_add_labels_error = true;
        self
    }

    /// Make all `remove_label` calls return a `CoreError::Network` error.
    async fn with_all_remove_label_network_error(self) -> Self {
        self.state.lock().await.all_remove_label_network_error = true;
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
        let mut st = self.state.lock().await;
        if st.search_returns_error {
            // One-shot: fail once, then succeed on subsequent calls so the
            // orchestrator's internal search can still work.
            st.search_returns_error = false;
            return Err(CoreError::network("Simulated search_pull_requests failure"));
        }
        Ok(st.search_results.clone())
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

    async fn add_labels(
        &self,
        _owner: &str,
        _repo: &str,
        pr_number: u64,
        labels: &[&str],
    ) -> CoreResult<()> {
        let mut st = self.state.lock().await;
        if st.next_add_labels_error {
            st.next_add_labels_error = false;
            return Err(CoreError::github(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Simulated add_labels failure",
            )));
        }
        let entry = st.pr_labels.entry(pr_number).or_default();
        for &name in labels {
            if !entry.iter().any(|l| l.name == name) {
                entry.push(Label {
                    id: entry.len() as u64 + 1,
                    name: name.to_string(),
                    color: "ededed".to_string(),
                    description: None,
                });
            }
        }
        Ok(())
    }

    async fn remove_label(
        &self,
        _owner: &str,
        _repo: &str,
        pr_number: u64,
        label_name: &str,
    ) -> CoreResult<()> {
        let mut st = self.state.lock().await;
        if st.all_remove_label_network_error {
            return Err(CoreError::network("Simulated remove_label network failure"));
        }
        if let Some(labels) = st.pr_labels.get_mut(&pr_number) {
            labels.retain(|l| l.name != label_name);
        }
        Ok(())
    }

    async fn list_pr_labels(
        &self,
        _owner: &str,
        _repo: &str,
        pr_number: u64,
    ) -> CoreResult<Vec<Label>> {
        Ok(self
            .state
            .lock()
            .await
            .pr_labels
            .get(&pr_number)
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

    fn scoped_to(&self, _installation_id: u64) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
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

fn make_release_pr(number: u64, base_sha: &str) -> PullRequest {
    let now = Utc::now();
    let r = stub_repo("acme", "app");
    PullRequest {
        number,
        title: "release: v1.0.0".to_string(),
        body: None,
        state: "open".to_string(),
        draft: false,
        created_at: now,
        updated_at: now,
        merged_at: None,
        user: stub_user(),
        head: PullRequestBranch {
            ref_name: "release/v1.0.0".to_string(),
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

/// Build a release PR whose head branch is `release/v{version}` and whose body
/// contains a `## Changelog` section with `changelog_content`.
fn make_release_pr_with_changelog(
    number: u64,
    version: &str,
    changelog_content: &str,
) -> PullRequest {
    let now = Utc::now();
    let r = stub_repo("acme", "app");
    PullRequest {
        number,
        title: format!("release: v{version}"),
        body: Some(format!(
            "## Changelog\n\n{changelog_content}\n\n## Notes\n\nAutomated."
        )),
        state: "open".to_string(),
        draft: false,
        created_at: now,
        updated_at: now,
        merged_at: None,
        user: stub_user(),
        head: PullRequestBranch {
            ref_name: format!("release/v{version}"),
            sha: "headsha".to_string(),
            repo: stub_repo("acme", "app"),
        },
        base: PullRequestBranch {
            ref_name: "main".to_string(),
            sha: "basesha".to_string(),
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
    test_event_with_login(pr_number, comment_body, pr_state, "test-user")
}

fn test_event_with_login(
    pr_number: u64,
    comment_body: &str,
    pr_state: &str,
    commenter_login: &str,
) -> ProcessingEvent {
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
                    "login": commenter_login
                }
            }
        }),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
        installation_id: 0,
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
    // No GitHub API calls should be made for unrecognised comment bodies —
    // not even a permission check — so the issue_comments list stays empty.
    assert!(github.issue_comments().await.is_empty());
    assert!(github.created_prs().await.is_empty());
}

#[tokio::test]
async fn test_process_bot_comment_with_command_is_noop() {
    // Simulates the feedback loop: a bot (e.g. our own app posting a reply)
    // includes text that looks like a command.  We must never act on it.
    let github = TestGitHub::new().with_pr(make_open_pr(42, "abc123")).await;
    let processor = CommentCommandProcessor::new(default_config(true), &github);

    let event = test_event_with_login(42, "!release minor", "open", "gg-release-regent[bot]");
    let result = processor.process(&event).await;

    assert!(result.is_ok());
    assert!(github.issue_comments().await.is_empty());
}

#[tokio::test]
async fn test_process_other_bot_comment_with_command_is_noop() {
    // A third-party bot that happens to include command-like text should also
    // be ignored.
    let github = TestGitHub::new().with_pr(make_open_pr(42, "abc123")).await;
    let processor = CommentCommandProcessor::new(default_config(true), &github);

    let event = test_event_with_login(42, "!set-version 1.0.0", "open", "some-other-app[bot]");
    let result = processor.process(&event).await;

    assert!(result.is_ok());
    assert!(github.issue_comments().await.is_empty());
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
        .with_pr(make_release_pr(42, "deadbeef"))
        .await;

    let processor = CommentCommandProcessor::new(default_config(true), &github);
    let event = test_event(42, "!set-version 2.0.0", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok());
    // A ✅ confirmation comment must be posted after successful orchestration.
    let comments = github.issue_comments().await;
    assert_eq!(
        comments.len(),
        1,
        "expected one confirmation comment, got {comments:?}"
    );
    assert_eq!(comments[0].0, 42);
    assert!(
        comments[0].1.contains('✅'),
        "expected confirmation (✅), got: {}",
        comments[0].1
    );
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
        .with_pr(make_release_pr(42, "deadbeef"))
        .await;

    let processor = CommentCommandProcessor::new(default_config(true), &github);
    // Skip straight to 1.0.0 — a valid first release.
    let event = test_event(42, "!set-version 1.0.0", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok());
    // A ✅ confirmation comment must be posted after successful orchestration.
    let comments = github.issue_comments().await;
    assert_eq!(
        comments.len(),
        1,
        "expected one confirmation comment, got {comments:?}"
    );
    assert!(comments[0].1.contains('✅'));
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
        .with_pr(make_release_pr(42, "deadbeef"))
        .await;

    let processor = CommentCommandProcessor::new(default_config(true), &github);
    let event = test_event(42, "!set-version 0.0.1", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok());
    // A ✅ confirmation comment must be posted after successful orchestration.
    let comments = github.issue_comments().await;
    assert_eq!(
        comments.len(),
        1,
        "expected one confirmation comment, got {comments:?}"
    );
    assert!(comments[0].1.contains('✅'));
}

#[tokio::test]
async fn test_process_set_version_rejected_when_equal_to_current_tag_posts_rejection_comment() {
    // Pinned == current released → rejected.
    let github = TestGitHub::new()
        .with_tags(vec![make_semver_tag("v1.0.0")])
        .await
        .with_pr(make_release_pr(42, "deadbeef"))
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
        .await
        .with_pr(make_release_pr(42, "deadbeef"))
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
    let github = TestGitHub::new()
        .with_tags(vec![])
        .await
        .with_pr(make_release_pr(42, "deadbeef"))
        .await;

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
async fn test_process_release_bump_applies_label_and_posts_confirmation() {
    // !release major must apply rr:override-major to the feature PR and post
    // a confirmation comment. No release PR should be created at this point.
    let github = TestGitHub::new();
    let processor = CommentCommandProcessor::new(default_config(true), &github);

    let event = test_event(42, "!release major", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok(), "expected Ok, got: {result:?}");
    // A confirmation comment should be posted on the feature PR.
    let comments = github.issue_comments().await;
    assert_eq!(comments.len(), 1, "expected one confirmation comment");
    assert_eq!(comments[0].0, 42);
    assert!(
        comments[0].1.contains("override recorded"),
        "comment should confirm the override, got: {}",
        comments[0].1
    );
    assert!(
        comments[0].1.contains("major"),
        "comment should mention the bump kind, got: {}",
        comments[0].1
    );
    // Label must be applied to the feature PR.
    let labels = github
        .state
        .lock()
        .await
        .pr_labels
        .get(&42)
        .cloned()
        .unwrap_or_default();
    assert!(
        labels.iter().any(|l| l.name == "rr:override-major"),
        "expected rr:override-major label, got: {labels:?}"
    );
    // No release PR should have been created.
    assert!(github.created_prs().await.is_empty());
}

#[tokio::test]
async fn test_process_release_bump_minor_applies_label_and_posts_confirmation() {
    let github = TestGitHub::new();
    let processor = CommentCommandProcessor::new(default_config(true), &github);

    let event = test_event(42, "!release minor", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok());
    let comments = github.issue_comments().await;
    assert_eq!(comments.len(), 1);
    assert!(comments[0].1.contains("override recorded"));
    assert!(comments[0].1.contains("minor"));
    let labels = github
        .state
        .lock()
        .await
        .pr_labels
        .get(&42)
        .cloned()
        .unwrap_or_default();
    assert!(labels.iter().any(|l| l.name == "rr:override-minor"));
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
        .with_pr(make_release_pr(42, "deadbeef"))
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

#[tokio::test]
async fn test_process_release_bump_replaces_existing_override_label() {
    // BA-25: posting !release minor on a PR that already has rr:override-major
    // should replace the label and mention the replacement in the comment.
    let existing_label = Label {
        id: 1,
        name: "rr:override-major".to_string(),
        color: "ededed".to_string(),
        description: None,
    };
    let github = TestGitHub::new()
        .with_pr_labels(55, vec![existing_label])
        .await;
    let processor = CommentCommandProcessor::new(default_config(true), &github);

    let event = test_event(55, "!release minor", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok(), "expected Ok, got: {result:?}");
    // rr:override-minor must be present; rr:override-major must be absent.
    let labels = github
        .state
        .lock()
        .await
        .pr_labels
        .get(&55)
        .cloned()
        .unwrap_or_default();
    assert!(
        labels.iter().any(|l| l.name == "rr:override-minor"),
        "expected rr:override-minor after replacement, got: {labels:?}"
    );
    assert!(
        !labels.iter().any(|l| l.name == "rr:override-major"),
        "rr:override-major should have been removed, got: {labels:?}"
    );
    let comments = github.issue_comments().await;
    assert_eq!(comments.len(), 1);
    assert!(
        comments[0].1.contains("replacing"),
        "confirmation comment should mention replacement, got: {}",
        comments[0].1
    );
}

#[tokio::test]
async fn test_process_set_version_rejected_when_on_non_release_pr_branch() {
    // BA-26: !set-version on a feature PR (head branch does not start with
    // "release/v") must post a scope rejection comment and not call the orchestrator.
    let feature_pr = make_open_pr(90, "abc123"); // head branch: feat/my-feature
    let github = TestGitHub::new()
        .with_tags(vec![make_semver_tag("v1.0.0")])
        .await
        .with_pr(feature_pr)
        .await;

    let processor = CommentCommandProcessor::new(default_config(true), &github);
    let event = test_event(90, "!set-version 2.0.0", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok(), "expected Ok, got: {result:?}");
    let comments = github.issue_comments().await;
    assert_eq!(comments.len(), 1, "expected one scope rejection comment");
    assert_eq!(comments[0].0, 90);
    assert!(
        comments[0].1.contains("release PR"),
        "rejection comment should mention release PR, got: {}",
        comments[0].1
    );
    // Orchestrator must not have been called.
    assert!(
        github.created_prs().await.is_empty(),
        "orchestrator must not be called on scope rejection"
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// !set-version changelog preservation & confirmation tests (task 9.30)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// BA-28a: !set-version when no existing release PR exists → orchestrator is
/// called with an empty changelog and a ✅ Created confirmation comment is posted.
#[tokio::test]
async fn test_process_set_version_posts_created_confirmation_when_no_existing_release_pr() {
    // No tags, no existing release PRs → first-release path.
    // search_results is empty → no existing changelog → orchestrator receives "".
    let github = TestGitHub::new()
        .with_tags(vec![])
        .await
        .with_pr(make_release_pr(42, "deadbeef"))
        .await;

    let processor = CommentCommandProcessor::new(default_config(true), &github);
    let event = test_event(42, "!set-version 1.0.0", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok(), "expected Ok, got: {result:?}");
    let comments = github.issue_comments().await;
    assert_eq!(
        comments.len(),
        1,
        "expected exactly one confirmation comment"
    );
    assert_eq!(comments[0].0, 42, "comment should be on PR 42");
    assert!(
        comments[0].1.contains('✅'),
        "comment should contain ✅, got: {}",
        comments[0].1
    );
    assert!(
        comments[0].1.contains("1.0.0"),
        "comment should mention the pinned version, got: {}",
        comments[0].1
    );
    // Release PR must have been created.
    let prs = github.created_prs().await;
    assert_eq!(prs.len(), 1, "expected one release PR to be created");
}

/// BA-28b: !set-version when an existing release PR has the same version →
/// real changelog is extracted and passed to the orchestrator, ✅ Updated comment.
#[tokio::test]
async fn test_process_set_version_preserves_existing_changelog_on_update_path() {
    // The existing release PR already targets 2.0.0 (same as pinned version).
    // The orchestrator will update its body and return OrchestratorResult::Updated.
    let existing_changelog = "- feat: add widget [aabbccdd]";
    let existing_release_pr = make_release_pr_with_changelog(100, "2.0.0", existing_changelog);

    let github = TestGitHub::new()
        .with_tags(vec![make_semver_tag("v1.0.0")])
        .await
        .with_pr(existing_release_pr.clone())
        .await
        .with_search_results(vec![existing_release_pr])
        .await;

    let processor = CommentCommandProcessor::new(default_config(true), &github);
    // Set the pinned version to 2.0.0 — same as the existing release PR.
    let event = test_event(100, "!set-version 2.0.0", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok(), "expected Ok, got: {result:?}");
    let comments = github.issue_comments().await;
    assert_eq!(comments.len(), 1, "expected one confirmation comment");
    assert!(
        comments[0].1.contains('✅'),
        "expected ✅ confirmation, got: {}",
        comments[0].1
    );
    assert!(
        comments[0].1.contains("2.0.0"),
        "comment should mention the version, got: {}",
        comments[0].1
    );
    // Because an existing PR at the same version was found, orchestrate()
    // updates it rather than creating.  Verify no new PR was created.
    assert!(
        github.created_prs().await.is_empty(),
        "no new PR should be created when updating existing version"
    );
}

/// !set-version raises the version on an existing release PR →
/// real changelog preserved, ✅ Renamed confirmation comment posted.
#[tokio::test]
async fn test_process_set_version_posts_renamed_confirmation_when_version_raised() {
    // Existing release PR is at 1.5.0; pinning to 2.0.0 will trigger a rename.
    let existing_changelog = "- feat: feature A [11223344]";
    let existing_release_pr = make_release_pr_with_changelog(101, "1.5.0", existing_changelog);

    let github = TestGitHub::new()
        .with_tags(vec![make_semver_tag("v1.0.0")])
        .await
        .with_pr(existing_release_pr.clone())
        .await
        .with_search_results(vec![existing_release_pr])
        .await;

    let processor = CommentCommandProcessor::new(default_config(true), &github);
    let event = test_event(101, "!set-version 2.0.0", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok(), "expected Ok, got: {result:?}");
    let comments = github.issue_comments().await;
    assert_eq!(comments.len(), 1, "expected one confirmation comment");
    assert!(
        comments[0].1.contains('✅'),
        "expected ✅ confirmation, got: {}",
        comments[0].1
    );
    // The rename path closes the old PR and creates a new one at the higher version.
    let prs = github.created_prs().await;
    assert_eq!(
        prs.len(),
        1,
        "expected one new release PR created for the renamed version"
    );
    assert!(
        prs[0].head.starts_with("release/v2.0.0"),
        "new PR should be at v2.0.0, got: {}",
        prs[0].head
    );
}

/// BA-29: !set-version below existing release PR version → orchestrator returns
/// NoOp, a ⚠️ warning comment is posted, no PR modification occurs.
#[tokio::test]
async fn test_process_set_version_posts_noop_warning_when_existing_pr_has_higher_version() {
    // Existing release PR is at 3.0.0; pinned to 2.0.0 → orchestrator NoOp.
    let existing_release_pr =
        make_release_pr_with_changelog(102, "3.0.0", "- feat: big thing [deadbeef]");

    let github = TestGitHub::new()
        .with_tags(vec![make_semver_tag("v1.0.0")])
        .await
        .with_pr(existing_release_pr.clone())
        .await
        .with_search_results(vec![existing_release_pr])
        .await;

    let processor = CommentCommandProcessor::new(default_config(true), &github);
    let event = test_event(102, "!set-version 2.0.0", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok(), "expected Ok, got: {result:?}");
    let comments = github.issue_comments().await;
    assert_eq!(comments.len(), 1, "expected one ⚠️ comment");
    assert!(
        comments[0].1.contains('⚠'),
        "expected ⚠️ warning, got: {}",
        comments[0].1
    );
    assert!(
        comments[0].1.contains("2.0.0"),
        "comment should mention the attempted version, got: {}",
        comments[0].1
    );
    // No new PR should have been created.
    assert!(github.created_prs().await.is_empty());
}

/// When the PR body is `None` (no pre-existing changelog content), the
/// changelog defaults to empty; orchestration still proceeds and a ✅ comment
/// is posted.  This exercises the `pr.body.as_deref().unwrap_or("")` fallback
/// that replaced the former `fetch_existing_release_changelog` helper.
#[tokio::test]
async fn test_process_set_version_proceeds_with_empty_changelog_when_pr_body_is_none() {
    let github = TestGitHub::new()
        .with_tags(vec![])
        .await
        .with_pr(make_release_pr(42, "abc123")) // body: None
        .await;

    let processor = CommentCommandProcessor::new(default_config(true), &github);
    let event = test_event(42, "!set-version 1.0.0", "open");
    let result = processor.process(&event).await;

    // Must succeed even though there is no existing changelog body.
    assert!(
        result.is_ok(),
        "expected Ok when pr.body is None, got: {result:?}"
    );
    // Must still post a confirmation comment.
    let comments = github.issue_comments().await;
    assert_eq!(comments.len(), 1, "expected one confirmation comment");
    assert!(
        comments[0].1.contains('✅'),
        "expected ✅ confirmation, got: {}",
        comments[0].1
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// handle_release_bump error-path tests (spec §9 Minor #4)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// When `add_labels` fails with a GitHub error, the error is propagated so
/// the event loop can schedule a retry.
#[tokio::test]
async fn test_handle_release_bump_add_labels_failure_propagates_error() {
    let github = TestGitHub::new().with_next_add_labels_error().await;
    let processor = CommentCommandProcessor::new(default_config(true), &github);

    let event = test_event(77, "!release major", "open");
    let result = processor.process(&event).await;

    // The error must be propagated (not swallowed as Ok).
    assert!(
        result.is_err(),
        "expected Err when add_labels fails, got: {result:?}"
    );
    // No confirmation comment should have been posted.
    assert!(
        github.issue_comments().await.is_empty(),
        "no confirmation comment should be posted when add_labels fails"
    );
}

/// When `remove_label` fails with a network error, the failure is logged as a
/// warning but `add_labels` is still called and a confirmation is still posted.
#[tokio::test]
async fn test_handle_release_bump_remove_label_failure_still_adds_new_label() {
    // Pre-populate an existing override label so the remove path is exercised.
    let existing_label = Label {
        id: 1,
        name: "rr:override-minor".to_string(),
        color: "ededed".to_string(),
        description: None,
    };
    let github = TestGitHub::new()
        .with_pr_labels(33, vec![existing_label])
        .await
        .with_all_remove_label_network_error()
        .await;
    let processor = CommentCommandProcessor::new(default_config(true), &github);

    let event = test_event(33, "!release major", "open");
    let result = processor.process(&event).await;

    // Despite remove_label failing, the overall operation should succeed.
    assert!(
        result.is_ok(),
        "expected Ok even when remove_label fails, got: {result:?}"
    );
    // A confirmation comment must still be posted.
    let comments = github.issue_comments().await;
    assert_eq!(
        comments.len(),
        1,
        "expected one confirmation comment despite remove_label failure"
    );
    assert!(
        comments[0].1.contains("override recorded"),
        "confirmation comment should confirm override, got: {}",
        comments[0].1
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// handle_set_version scope guard edge cases (spec §9 Minor #7)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// `!set-version` posted on a PR whose head branch starts with `release/` but
/// does NOT include a `/v` prefix (e.g. `release/some-branch`) must be rejected
/// with a scope-guard comment. Only `release/v*` branches are accepted.
#[tokio::test]
async fn test_process_set_version_rejected_on_release_branch_without_v_prefix() {
    // Build a PR whose head branch is "release/some-branch" (no "/v").
    let now = Utc::now();
    let r = stub_repo("acme", "app");
    let non_versioned_release_pr = PullRequest {
        number: 55,
        title: "release: some-branch".to_string(),
        body: None,
        state: "open".to_string(),
        draft: false,
        created_at: now,
        updated_at: now,
        merged_at: None,
        user: stub_user(),
        head: PullRequestBranch {
            ref_name: "release/some-branch".to_string(), // starts with "release/" but no "/v"
            sha: "headsha".to_string(),
            repo: stub_repo("acme", "app"),
        },
        base: PullRequestBranch {
            ref_name: "main".to_string(),
            sha: "basesha".to_string(),
            repo: r,
        },
    };
    let github = TestGitHub::new()
        .with_tags(vec![make_semver_tag("v1.0.0")])
        .await
        .with_pr(non_versioned_release_pr)
        .await;

    let processor = CommentCommandProcessor::new(default_config(true), &github);
    let event = test_event(55, "!set-version 2.0.0", "open");
    let result = processor.process(&event).await;

    assert!(
        result.is_ok(),
        "expected Ok (acknowledged), got: {result:?}"
    );
    let comments = github.issue_comments().await;
    assert_eq!(
        comments.len(),
        1,
        "expected one scope-rejection comment, got {comments:?}"
    );
    assert_eq!(comments[0].0, 55, "comment should be on PR #55");
    assert!(
        comments[0].1.contains("release PR"),
        "rejection comment should mention release PR, got: {}",
        comments[0].1
    );
    // Orchestrator must not have been called.
    assert!(
        github.created_prs().await.is_empty(),
        "orchestrator must not be called on scope rejection"
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// handle_release_bump same-override repost test (spec §9 Minor #8)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// When the same `!release patch` override is posted a second time on a PR
/// that already carries `rr:override-patch`, the confirmation comment must use
/// "re-recorded" wording (not "replacing") to confirm idempotent re-application.
#[tokio::test]
async fn test_process_release_bump_same_override_reposted_shows_rererecorded_confirmation() {
    let existing_patch_label = Label {
        id: 1,
        name: "rr:override-patch".to_string(),
        color: "ededed".to_string(),
        description: None,
    };
    let github = TestGitHub::new()
        .with_pr_labels(66, vec![existing_patch_label])
        .await;
    let processor = CommentCommandProcessor::new(default_config(true), &github);

    // Post `!release patch` again on a PR that already has rr:override-patch.
    let event = test_event(66, "!release patch", "open");
    let result = processor.process(&event).await;

    assert!(result.is_ok(), "expected Ok, got: {result:?}");
    let comments = github.issue_comments().await;
    assert_eq!(
        comments.len(),
        1,
        "expected one confirmation comment, got {comments:?}"
    );
    assert!(
        comments[0].1.contains("re-recorded"),
        "confirmation should mention 're-recorded' when same override is reposted, got: {}",
        comments[0].1
    );
    assert!(
        comments[0].1.contains("patch"),
        "confirmation should mention the bump kind, got: {}",
        comments[0].1
    );
    // rr:override-patch must still be present after re-application.
    let labels = github
        .state
        .lock()
        .await
        .pr_labels
        .get(&66)
        .cloned()
        .unwrap_or_default();
    assert!(
        labels.iter().any(|l| l.name == "rr:override-patch"),
        "rr:override-patch must be re-applied after repost, got: {labels:?}"
    );
}
