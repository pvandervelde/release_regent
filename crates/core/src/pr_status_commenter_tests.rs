use super::*;
use crate::{
    traits::{
        git_operations::{
            GetCommitsOptions, GitCommit, GitOperations, GitRepository, GitTag, ListTagsOptions,
        },
        github_operations::{
            CollaboratorPermission, CreatePullRequestParams, CreateReleaseParams, FileUpdate,
            GitHubOperations, GitUser, IssueComment, Label, PullRequest, PullRequestBranch,
            Release, Repository, Tag, UpdateReleaseParams,
        },
    },
    versioning::SemanticVersion,
    CoreError,
};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

// ── Minimal inline test double for upsert tests ──────────────────────────────

#[derive(Clone, Default)]
struct UpsertTestState {
    /// Pre-loaded comments returned by `list_issue_comments`.
    comments: Vec<IssueComment>,
    /// Recorded `create_issue_comment` calls: `(issue_number, body)`.
    create_calls: Vec<(u64, String)>,
    /// Recorded `update_issue_comment` calls: `(comment_id, body)`.
    update_calls: Vec<(u64, String)>,
    /// When `true`, `list_issue_comments` returns an error.
    list_error: bool,
    /// When `true`, `update_issue_comment` returns an error.
    update_error: bool,
}

#[derive(Clone)]
struct TestGitHubForUpsert {
    state: Arc<Mutex<UpsertTestState>>,
}

impl TestGitHubForUpsert {
    fn new(comments: Vec<IssueComment>) -> Self {
        Self {
            state: Arc::new(Mutex::new(UpsertTestState {
                comments,
                ..Default::default()
            })),
        }
    }

    fn with_list_error() -> Self {
        Self {
            state: Arc::new(Mutex::new(UpsertTestState {
                list_error: true,
                ..Default::default()
            })),
        }
    }

    fn with_update_error(existing_comment_id: u64) -> Self {
        Self {
            state: Arc::new(Mutex::new(UpsertTestState {
                comments: vec![IssueComment {
                    id: existing_comment_id,
                    body: format!("{PR_STATUS_MARKER}\nold body"),
                    user_login: None,
                }],
                update_error: true,
                ..Default::default()
            })),
        }
    }

    async fn create_calls(&self) -> Vec<(u64, String)> {
        self.state.lock().await.create_calls.clone()
    }

    async fn update_calls(&self) -> Vec<(u64, String)> {
        self.state.lock().await.update_calls.clone()
    }
}

#[async_trait]
impl GitOperations for TestGitHubForUpsert {
    async fn get_commits_between(
        &self,
        _owner: &str,
        _repo: &str,
        _base: &str,
        _head: &str,
        _options: GetCommitsOptions,
    ) -> crate::CoreResult<Vec<GitCommit>> {
        Ok(vec![])
    }

    async fn get_commit(
        &self,
        _owner: &str,
        _repo: &str,
        _sha: &str,
    ) -> crate::CoreResult<GitCommit> {
        Err(CoreError::not_found("stub"))
    }

    async fn list_tags(
        &self,
        _owner: &str,
        _repo: &str,
        _options: ListTagsOptions,
    ) -> crate::CoreResult<Vec<GitTag>> {
        Ok(vec![])
    }

    async fn get_tag(
        &self,
        _owner: &str,
        _repo: &str,
        _tag_name: &str,
    ) -> crate::CoreResult<GitTag> {
        Err(CoreError::not_found("stub"))
    }

    async fn tag_exists(
        &self,
        _owner: &str,
        _repo: &str,
        _tag_name: &str,
    ) -> crate::CoreResult<bool> {
        Ok(false)
    }

    async fn get_head_commit(
        &self,
        _owner: &str,
        _repo: &str,
        _branch: Option<&str>,
    ) -> crate::CoreResult<GitCommit> {
        Err(CoreError::not_found("stub"))
    }

    async fn get_repository_info(
        &self,
        _owner: &str,
        _repo: &str,
    ) -> crate::CoreResult<GitRepository> {
        Err(CoreError::not_found("stub"))
    }
}

#[async_trait]
impl GitHubOperations for TestGitHubForUpsert {
    async fn add_labels(
        &self,
        _owner: &str,
        _repo: &str,
        _issue_number: u64,
        _labels: &[&str],
    ) -> crate::CoreResult<()> {
        Ok(())
    }

    async fn batch_commit_files(
        &self,
        _owner: &str,
        _repo: &str,
        _branch: &str,
        _files: &[FileUpdate],
        _message: &str,
    ) -> crate::CoreResult<()> {
        Ok(())
    }

    async fn create_branch(
        &self,
        _owner: &str,
        _repo: &str,
        _branch_name: &str,
        _sha: &str,
    ) -> crate::CoreResult<()> {
        Ok(())
    }

    async fn create_issue_comment(
        &self,
        _owner: &str,
        _repo: &str,
        issue_number: u64,
        body: &str,
    ) -> crate::CoreResult<()> {
        self.state
            .lock()
            .await
            .create_calls
            .push((issue_number, body.to_string()));
        Ok(())
    }

    async fn create_pull_request(
        &self,
        _owner: &str,
        _repo: &str,
        _params: CreatePullRequestParams,
    ) -> crate::CoreResult<PullRequest> {
        Err(CoreError::not_found("stub"))
    }

    async fn create_release(
        &self,
        _owner: &str,
        _repo: &str,
        _params: CreateReleaseParams,
    ) -> crate::CoreResult<Release> {
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
    ) -> crate::CoreResult<Tag> {
        Err(CoreError::not_found("stub"))
    }

    async fn delete_branch(
        &self,
        _owner: &str,
        _repo: &str,
        _branch_name: &str,
    ) -> crate::CoreResult<()> {
        Ok(())
    }

    async fn force_update_branch(
        &self,
        _owner: &str,
        _repo: &str,
        _branch_name: &str,
        _sha: &str,
    ) -> crate::CoreResult<()> {
        Ok(())
    }

    async fn get_collaborator_permission(
        &self,
        _owner: &str,
        _repo: &str,
        _username: &str,
    ) -> crate::CoreResult<CollaboratorPermission> {
        Ok(CollaboratorPermission::Write)
    }

    async fn get_file_content(
        &self,
        _owner: &str,
        _repo: &str,
        _path: &str,
        _branch: &str,
    ) -> crate::CoreResult<Option<String>> {
        Ok(None)
    }

    async fn get_installation_id_for_repo(
        &self,
        _owner: &str,
        _repo: &str,
    ) -> crate::CoreResult<u64> {
        Ok(0)
    }

    async fn get_latest_release(
        &self,
        _owner: &str,
        _repo: &str,
    ) -> crate::CoreResult<Option<Release>> {
        Ok(None)
    }

    async fn get_pull_request(
        &self,
        _owner: &str,
        _repo: &str,
        _pr_number: u64,
    ) -> crate::CoreResult<PullRequest> {
        Err(CoreError::not_found("stub"))
    }

    async fn get_release_by_tag(
        &self,
        _owner: &str,
        _repo: &str,
        _tag: &str,
    ) -> crate::CoreResult<Release> {
        Err(CoreError::not_found("stub"))
    }

    async fn list_issue_comments(
        &self,
        _owner: &str,
        _repo: &str,
        _issue_number: u64,
    ) -> crate::CoreResult<Vec<IssueComment>> {
        if self.state.lock().await.list_error {
            return Err(CoreError::network("simulated list_issue_comments failure"));
        }
        Ok(self.state.lock().await.comments.clone())
    }

    async fn list_pr_labels(
        &self,
        _owner: &str,
        _repo: &str,
        _issue_number: u64,
    ) -> crate::CoreResult<Vec<Label>> {
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
    ) -> crate::CoreResult<Vec<PullRequest>> {
        Ok(vec![])
    }

    async fn list_releases(
        &self,
        _owner: &str,
        _repo: &str,
        _per_page: Option<u8>,
        _page: Option<u32>,
    ) -> crate::CoreResult<Vec<Release>> {
        Ok(vec![])
    }

    async fn remove_label(
        &self,
        _owner: &str,
        _repo: &str,
        _issue_number: u64,
        _label_name: &str,
    ) -> crate::CoreResult<()> {
        Ok(())
    }

    async fn search_pull_requests(
        &self,
        _owner: &str,
        _repo: &str,
        _query: &str,
    ) -> crate::CoreResult<Vec<PullRequest>> {
        Ok(vec![])
    }

    async fn update_issue_comment(
        &self,
        _owner: &str,
        _repo: &str,
        comment_id: u64,
        body: &str,
    ) -> crate::CoreResult<()> {
        if self.state.lock().await.update_error {
            return Err(CoreError::network("simulated update_issue_comment failure"));
        }
        self.state
            .lock()
            .await
            .update_calls
            .push((comment_id, body.to_string()));
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
    ) -> crate::CoreResult<PullRequest> {
        Err(CoreError::not_found("stub"))
    }

    async fn update_release(
        &self,
        _owner: &str,
        _repo: &str,
        _release_id: u64,
        _params: UpdateReleaseParams,
    ) -> crate::CoreResult<Release> {
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
    ) -> crate::CoreResult<()> {
        Ok(())
    }

    fn scoped_to(&self, _installation_id: u64) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }
}

// ── upsert_pr_status_comment tests ──────────────────────────────────────────

#[tokio::test]
async fn test_upsert_pr_status_comment_no_prior_comment_calls_create() {
    let github = TestGitHubForUpsert::new(vec![]);
    let body = format!("{PR_STATUS_MARKER}\nfresh status");

    upsert_pr_status_comment(&github, "owner", "repo", 7, &body)
        .await
        .expect("upsert should succeed");

    let creates = github.create_calls().await;
    let updates = github.update_calls().await;

    assert_eq!(
        creates.len(),
        1,
        "expected exactly one create_issue_comment call"
    );
    assert_eq!(creates[0].0, 7, "create called with wrong issue number");
    assert_eq!(creates[0].1, body, "create called with wrong body");
    assert!(
        updates.is_empty(),
        "update_issue_comment must not be called when no prior comment exists"
    );
}

#[tokio::test]
async fn test_upsert_pr_status_comment_prior_marker_present_calls_update() {
    let existing = IssueComment {
        id: 999,
        body: format!("{PR_STATUS_MARKER}\nold status"),
        user_login: Some("release-regent[bot]".into()),
    };
    let github = TestGitHubForUpsert::new(vec![existing]);
    let new_body = format!("{PR_STATUS_MARKER}\nnew status");

    upsert_pr_status_comment(&github, "owner", "repo", 7, &new_body)
        .await
        .expect("upsert should succeed");

    let creates = github.create_calls().await;
    let updates = github.update_calls().await;

    assert!(
        creates.is_empty(),
        "create_issue_comment must not be called when a marker comment already exists"
    );
    assert_eq!(
        updates.len(),
        1,
        "expected exactly one update_issue_comment call"
    );
    assert_eq!(updates[0].0, 999, "update called with wrong comment id");
    assert_eq!(updates[0].1, new_body, "update called with wrong body");
}

#[tokio::test]
async fn test_upsert_pr_status_comment_prior_comment_without_marker_calls_create() {
    let unrelated = IssueComment {
        id: 777,
        body: "This is a regular review comment with no marker".into(),
        user_login: Some("reviewer".into()),
    };
    let github = TestGitHubForUpsert::new(vec![unrelated]);
    let body = format!("{PR_STATUS_MARKER}\nnew status");

    upsert_pr_status_comment(&github, "owner", "repo", 42, &body)
        .await
        .expect("upsert should succeed");

    let creates = github.create_calls().await;
    let updates = github.update_calls().await;

    assert_eq!(
        creates.len(),
        1,
        "should create when no marker present in existing comments"
    );
    assert!(updates.is_empty(), "should not update when no marker found");
}

#[tokio::test]
async fn test_upsert_pr_status_comment_list_error_propagates() {
    let github = TestGitHubForUpsert::with_list_error();
    let body = format!("{PR_STATUS_MARKER}\nsome body");

    let result = upsert_pr_status_comment(&github, "owner", "repo", 1, &body).await;

    assert!(result.is_err(), "list error must propagate");
    assert!(github.create_calls().await.is_empty());
    assert!(github.update_calls().await.is_empty());
}

#[tokio::test]
async fn test_upsert_pr_status_comment_update_error_propagates() {
    let github = TestGitHubForUpsert::with_update_error(555);
    let body = format!("{PR_STATUS_MARKER}\nnew body");

    let result = upsert_pr_status_comment(&github, "owner", "repo", 1, &body).await;

    assert!(result.is_err(), "update error must propagate");
    assert!(github.create_calls().await.is_empty());
}

// ── render_feature_pr_comment tests ─────────────────────────────────────────

fn v100() -> SemanticVersion {
    SemanticVersion {
        major: 1,
        minor: 0,
        patch: 0,
        prerelease: None,
        build: None,
    }
}

fn v110() -> SemanticVersion {
    SemanticVersion {
        major: 1,
        minor: 1,
        patch: 0,
        prerelease: None,
        build: None,
    }
}

#[test]
fn test_render_feature_pr_comment_starts_with_html_marker() {
    let body = render_feature_pr_comment(&v110(), &v100(), true);
    assert!(
        body.starts_with(PR_STATUS_MARKER),
        "feature PR comment must start with the HTML marker; got: {body}"
    );
}

#[test]
fn test_render_feature_pr_comment_includes_projected_version() {
    let body = render_feature_pr_comment(&v110(), &v100(), false);
    assert!(
        body.contains("v1.1.0"),
        "feature PR comment must include projected version; got: {body}"
    );
}

#[test]
fn test_render_feature_pr_comment_includes_base_version_in_subtitle() {
    let body = render_feature_pr_comment(&v110(), &v100(), false);
    assert!(
        body.contains("v1.0.0"),
        "feature PR comment must include base version in subtitle; got: {body}"
    );
}

#[test]
fn test_render_feature_pr_comment_includes_commands_table_when_allow_override_true() {
    let body = render_feature_pr_comment(&v110(), &v100(), true);
    assert!(
        body.contains("### Available commands"),
        "commands section must be present when allow_override=true; got: {body}"
    );
    assert!(body.contains("!release major"));
    assert!(body.contains("!release minor"));
    assert!(body.contains("!release patch"));
    assert!(body.contains("!set-version X.Y.Z"));
}

#[test]
fn test_render_feature_pr_comment_omits_commands_table_when_allow_override_false() {
    let body = render_feature_pr_comment(&v110(), &v100(), false);
    assert!(
        !body.contains("### Available commands"),
        "commands section must be absent when allow_override=false; got: {body}"
    );
}

// ── render_release_pr_comment tests ─────────────────────────────────────────

fn v200() -> SemanticVersion {
    SemanticVersion {
        major: 2,
        minor: 0,
        patch: 0,
        prerelease: None,
        build: None,
    }
}

#[test]
fn test_render_release_pr_comment_starts_with_html_marker() {
    let body = render_release_pr_comment(&v200(), true);
    assert!(
        body.starts_with(PR_STATUS_MARKER),
        "release PR comment must start with the HTML marker; got: {body}"
    );
}

#[test]
fn test_render_release_pr_comment_includes_release_version() {
    let body = render_release_pr_comment(&v200(), false);
    assert!(
        body.contains("v2.0.0"),
        "release PR comment must include the release version; got: {body}"
    );
}

#[test]
fn test_render_release_pr_comment_includes_commands_when_allow_override_true() {
    let body = render_release_pr_comment(&v200(), true);
    assert!(
        body.contains("### Available commands"),
        "commands section must be present when allow_override=true; got: {body}"
    );
    assert!(body.contains("!set-version X.Y.Z"));
}

#[test]
fn test_render_release_pr_comment_omits_commands_when_allow_override_false() {
    let body = render_release_pr_comment(&v200(), false);
    assert!(
        !body.contains("### Available commands"),
        "commands section must be absent when allow_override=false; got: {body}"
    );
}
