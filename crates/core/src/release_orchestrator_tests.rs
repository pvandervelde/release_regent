use super::*;
use crate::{
    traits::{
        git_operations::{
            GetCommitsOptions, GitCommit, GitOperations, GitRepository, GitTag, ListTagsOptions,
        },
        github_operations::{
            CreatePullRequestParams, CreateReleaseParams, FileUpdate, GitHubOperations,
            GitUser as GitHubUser, PullRequest, PullRequestBranch, Release, Repository, Tag,
            UpdateReleaseParams,
        },
    },
    versioning::SemanticVersion,
    CoreError, CoreResult,
};
use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing_test::traced_test;

// ─────────────────────────────────────────────────────────────────────────────
// Inline test double
//
// Defined locally to avoid the E0277 cross-crate blanket-impl issue documented
// in the project's "Rules & Tips".
// ─────────────────────────────────────────────────────────────────────────────

/// State shared inside `TestGitHub`.
#[derive(Default)]
struct TestState {
    /// PRs returned by `search_pull_requests`.
    search_results: Vec<PullRequest>,
    /// PR returned by `get_pull_request` (by number).
    pr_by_number: Vec<PullRequest>,
    /// Whether the next `create_branch` call should return a Conflict error.
    next_create_branch_conflict: bool,
    /// Recorded `create_branch` calls: (branch_name, sha).
    created_branches: Vec<(String, String)>,
    /// Recorded `create_pull_request` calls.
    created_prs: Vec<CreatePullRequestParams>,
    /// Recorded `update_pull_request` calls: (number, title, body, state).
    updated_prs: Vec<(u64, Option<String>, Option<String>, Option<String>)>,
    /// Recorded `delete_branch` calls: branch name.
    deleted_branches: Vec<String>,
    /// Recorded `force_update_branch` calls: (branch_name, sha).
    force_updated_branches: Vec<(String, String)>,
    /// Recorded `upsert_file` calls: (path, branch).
    upserted_files: Vec<(String, String)>,
    /// Recorded `batch_commit_files` calls: (branch, paths, message).
    batch_commits: Vec<(String, Vec<String>, String)>,
    /// Recorded `batch_commit_files_rebased` calls: (branch, paths, message, parent_sha).
    rebased_batch_commits: Vec<(String, Vec<String>, String, String)>,
    /// Full `FileUpdate` lists from each `batch_commit_files_rebased` call.
    rebased_batch_file_updates: Vec<Vec<FileUpdate>>,
    /// Pre-loaded file contents for `get_file_content`: (path, branch) → content.
    file_contents: std::collections::HashMap<(String, String), String>,
    /// Sequential PR number to return from `create_pull_request`.
    next_pr_number: u64,
    /// Whether `search_pull_requests` should return an error.
    search_error: bool,
}

#[derive(Clone, Default)]
pub(super) struct TestGitHub {
    state: Arc<Mutex<TestState>>,
}

impl TestGitHub {
    pub(super) fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(TestState {
                next_pr_number: 100,
                ..Default::default()
            })),
        }
    }

    /// Configure the mock to return these PRs from `search_pull_requests`.
    async fn with_search_results(self, prs: Vec<PullRequest>) -> Self {
        self.state.lock().await.search_results = prs;
        self
    }

    /// Make the *next* `create_branch` call return `CoreError::Conflict`.
    async fn with_next_create_branch_conflict(self) -> Self {
        self.state.lock().await.next_create_branch_conflict = true;
        self
    }

    async fn with_search_error(self) -> Self {
        self.state.lock().await.search_error = true;
        self
    }

    async fn with_pr_by_number(self, pr: PullRequest) -> Self {
        self.state.lock().await.pr_by_number.push(pr);
        self
    }

    async fn created_branches(&self) -> Vec<(String, String)> {
        self.state.lock().await.created_branches.clone()
    }

    async fn created_prs(&self) -> Vec<CreatePullRequestParams> {
        self.state.lock().await.created_prs.clone()
    }

    async fn updated_prs(&self) -> Vec<(u64, Option<String>, Option<String>, Option<String>)> {
        self.state.lock().await.updated_prs.clone()
    }

    async fn deleted_branches(&self) -> Vec<String> {
        self.state.lock().await.deleted_branches.clone()
    }

    async fn force_updated_branches(&self) -> Vec<(String, String)> {
        self.state.lock().await.force_updated_branches.clone()
    }

    async fn upserted_files(&self) -> Vec<(String, String)> {
        self.state.lock().await.upserted_files.clone()
    }

    async fn batch_commits(&self) -> Vec<(String, Vec<String>, String)> {
        self.state.lock().await.batch_commits.clone()
    }

    async fn rebased_batch_commits(&self) -> Vec<(String, Vec<String>, String, String)> {
        self.state.lock().await.rebased_batch_commits.clone()
    }

    async fn rebased_batch_file_updates(&self) -> Vec<Vec<FileUpdate>> {
        self.state.lock().await.rebased_batch_file_updates.clone()
    }

    /// Pre-load a file content so `get_file_content` returns it.
    async fn with_file_content(self, path: &str, branch: &str, content: &str) -> Self {
        self.state
            .lock()
            .await
            .file_contents
            .insert((path.to_string(), branch.to_string()), content.to_string());
        self
    }
}

// ── GitOperations stub impl ───────────────────────────────────────────────

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

    async fn get_repository_info(&self, _owner: &str, _repo: &str) -> CoreResult<GitRepository> {
        Err(CoreError::not_found("stub"))
    }
}

// ── GitHubOperations impl ────────────────────────────────────────────────

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

        let stub_repo = stub_repo(owner, repo);
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
            user: stub_git_user(),
            head: PullRequestBranch {
                ref_name: params.head,
                sha: "abc".to_string(),
                repo: stub_repo.clone(),
            },
            base: PullRequestBranch {
                ref_name: params.base,
                sha: "bcd".to_string(),
                repo: stub_repo,
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
        st.pr_by_number
            .iter()
            .find(|pr| pr.number == pr_number)
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
        let st = self.state.lock().await;
        if st.search_error {
            return Err(CoreError::network("simulated search failure"));
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
        let mut st = self.state.lock().await;
        st.updated_prs
            .push((pr_number, title.clone(), body.clone(), state.clone()));

        // Return a minimal updated PR.
        let now = Utc::now();
        let stub_r = stub_repo("test", "repo");
        Ok(PullRequest {
            number: pr_number,
            title: title.unwrap_or_else(|| "updated".to_string()),
            body,
            state: state.unwrap_or_else(|| "open".to_string()),
            draft: false,
            created_at: now,
            updated_at: now,
            merged_at: None,
            user: stub_git_user(),
            head: PullRequestBranch {
                ref_name: "release/v1.0.0".to_string(),
                sha: "abc".to_string(),
                repo: stub_r.clone(),
            },
            base: PullRequestBranch {
                ref_name: "main".to_string(),
                sha: "bcd".to_string(),
                repo: stub_r,
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

    async fn delete_branch(&self, _owner: &str, _repo: &str, branch_name: &str) -> CoreResult<()> {
        self.state
            .lock()
            .await
            .deleted_branches
            .push(branch_name.to_string());
        Ok(())
    }

    async fn force_update_branch(
        &self,
        _owner: &str,
        _repo: &str,
        branch_name: &str,
        sha: &str,
    ) -> CoreResult<()> {
        self.state
            .lock()
            .await
            .force_updated_branches
            .push((branch_name.to_string(), sha.to_string()));
        Ok(())
    }

    async fn create_issue_comment(
        &self,
        _owner: &str,
        _repo: &str,
        _issue_number: u64,
        _body: &str,
    ) -> CoreResult<()> {
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
        _issue_number: u64,
        _label_name: &str,
    ) -> CoreResult<()> {
        Ok(())
    }

    async fn list_pr_labels(
        &self,
        _owner: &str,
        _repo: &str,
        _issue_number: u64,
    ) -> CoreResult<Vec<crate::traits::github_operations::Label>> {
        Ok(vec![])
    }

    async fn get_installation_id_for_repo(&self, _owner: &str, _repo: &str) -> CoreResult<u64> {
        Ok(0)
    }

    async fn upsert_file(
        &self,
        _owner: &str,
        _repo: &str,
        path: &str,
        _commit_message: &str,
        _content: &str,
        branch: &str,
    ) -> CoreResult<()> {
        self.state
            .lock()
            .await
            .upserted_files
            .push((path.to_string(), branch.to_string()));
        Ok(())
    }

    async fn get_file_content(
        &self,
        _owner: &str,
        _repo: &str,
        path: &str,
        branch: &str,
    ) -> CoreResult<Option<String>> {
        let st = self.state.lock().await;
        Ok(st
            .file_contents
            .get(&(path.to_string(), branch.to_string()))
            .cloned())
    }

    async fn batch_commit_files(
        &self,
        _owner: &str,
        _repo: &str,
        branch: &str,
        files: &[FileUpdate],
        message: &str,
    ) -> CoreResult<()> {
        let paths: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
        self.state.lock().await.batch_commits.push((
            branch.to_string(),
            paths,
            message.to_string(),
        ));
        Ok(())
    }

    async fn batch_commit_files_rebased(
        &self,
        _owner: &str,
        _repo: &str,
        branch: &str,
        files: &[FileUpdate],
        message: &str,
        parent_sha: &str,
    ) -> CoreResult<()> {
        let paths: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
        let mut st = self.state.lock().await;
        st.rebased_batch_commits.push((
            branch.to_string(),
            paths,
            message.to_string(),
            parent_sha.to_string(),
        ));
        st.rebased_batch_file_updates.push(files.to_vec());
        Ok(())
    }

    async fn list_issue_comments(
        &self,
        _owner: &str,
        _repo: &str,
        _issue_number: u64,
    ) -> CoreResult<Vec<crate::traits::github_operations::IssueComment>> {
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

    fn scoped_to(&self, _installation_id: u64) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }
}
// ─────────────────────────────────────────────────────────────────────────────

pub(super) fn stub_repo(owner: &str, repo: &str) -> Repository {
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

pub(super) fn stub_git_user() -> GitHubUser {
    GitHubUser {
        name: "test-user".to_string(),
        email: "test@example.com".to_string(),
        login: Some("test-user".to_string()),
    }
}

pub(super) fn make_open_release_pr(number: u64, branch: &str, body: Option<&str>) -> PullRequest {
    let now = Utc::now();
    let r = stub_repo("testorg", "testrepo");
    PullRequest {
        number,
        title: format!("chore(release): {branch}"),
        body: body.map(std::string::ToString::to_string),
        state: "open".to_string(),
        draft: false,
        created_at: now,
        updated_at: now,
        merged_at: None,
        user: stub_git_user(),
        head: PullRequestBranch {
            ref_name: branch.to_string(),
            sha: "deadbeef".to_string(),
            repo: r.clone(),
        },
        base: PullRequestBranch {
            ref_name: "main".to_string(),
            sha: "cafe1234".to_string(),
            repo: r,
        },
    }
}

pub(super) fn ver(major: u64, minor: u64, patch: u64) -> SemanticVersion {
    SemanticVersion {
        major,
        minor,
        patch,
        prerelease: None,
        build: None,
    }
}

pub(super) fn default_config() -> OrchestratorConfig {
    OrchestratorConfig::default()
}

// ─────────────────────────────────────────────────────────────────────────────
// Structured logging sub-module
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "release_orchestrator_tracing_tests.rs"]
mod tracing_tests;

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

/// No existing release PR → creates new branch and PR.
#[tokio::test]
async fn test_orchestrate_no_existing_pr_creates_branch_and_pr() {
    let github = TestGitHub::new();
    let orchestrator = ReleaseOrchestrator::new(default_config(), &github);
    let version = ver(1, 2, 3);

    let result = orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &version,
            "- feat: add thing [abc1234567890123456789012345678901234567890]",
            "main",
            "sha001",
            "corr-001",
        )
        .await
        .expect("orchestrate should succeed");

    assert!(
        matches!(result, OrchestratorResult::Created { .. }),
        "expected Created, got {result:?}"
    );

    let branches = github.created_branches().await;
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].0, "release/v1.2.3");
    assert_eq!(branches[0].1, "sha001");

    let prs = github.created_prs().await;
    assert_eq!(prs.len(), 1);
    assert_eq!(prs[0].base, "main");
    assert_eq!(prs[0].head, "release/v1.2.3");
    assert!(prs[0].title.contains("v1.2.3"));
    assert!(prs[0]
        .body
        .as_deref()
        .unwrap_or("")
        .contains("## Changelog"));

    // `create_release_branch_and_pr` uses `batch_commit_files_rebased` to avoid
    // the race where setting branch tip == base_sha first would auto-close the PR.
    let rebased = github.rebased_batch_commits().await;
    assert_eq!(
        rebased.len(),
        1,
        "CHANGELOG.md should be committed to the release branch via batch_commit_files_rebased"
    );
    assert!(rebased[0].1.contains(&"CHANGELOG.md".to_string()));
    assert_eq!(rebased[0].0, "release/v1.2.3");
    assert_eq!(
        rebased[0].3, "sha001",
        "rebased commit must use base_sha as parent"
    );

    // No plain batch_commit_files should have been called.
    assert!(
        github.batch_commits().await.is_empty(),
        "create path should use batch_commit_files_rebased, not batch_commit_files"
    );

    // force_update_branch must never be called on the fresh-branch path: it would
    // temporarily set branch tip == base_sha, which auto-closes any open release PR.
    assert!(
        github.force_updated_branches().await.is_empty(),
        "force_update_branch must not be called on the fresh-branch create path"
    );
}

/// Existing PR has the same version → changelog is merged (Updated).
#[tokio::test]
async fn test_orchestrate_existing_equal_version_pr_updates_changelog() {
    let existing_body = "## Changelog\n\n- fix: old fix [aabbccddeeff00112233445566778899aabbccdd]";
    let existing_pr = make_open_release_pr(42, "release/v1.0.0", Some(existing_body));

    let github = TestGitHub::new()
        .with_search_results(vec![existing_pr.clone()])
        .await
        .with_pr_by_number(existing_pr)
        .await;

    let orchestrator = ReleaseOrchestrator::new(default_config(), &github);
    let version = ver(1, 0, 0);

    let result = orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &version,
            "- feat: new feature [1122334455667788990011223344556677889900]",
            "main",
            "sha002",
            "corr-002",
        )
        .await
        .expect("orchestrate should succeed");

    assert!(
        matches!(result, OrchestratorResult::Updated { .. }),
        "expected Updated, got {result:?}"
    );

    // No new branch should be created.
    assert!(github.created_branches().await.is_empty());

    // update_pull_request should have been called with merged body.
    let updates = github.updated_prs().await;
    assert_eq!(updates.len(), 1);
    let body = updates[0].2.as_deref().unwrap_or("");
    assert!(body.contains("old fix"), "should keep existing entry");
    assert!(body.contains("new feature"), "should add new entry");

    // CHANGELOG.md should be committed to the existing branch, rebased onto base SHA.
    let rebased = github.rebased_batch_commits().await;
    assert_eq!(
        rebased.len(),
        1,
        "CHANGELOG.md should be committed via batch_commit_files_rebased"
    );
    assert!(rebased[0].1.contains(&"CHANGELOG.md".to_string()));
    assert_eq!(rebased[0].0, "release/v1.0.0");
    assert_eq!(
        rebased[0].3, "sha002",
        "rebased commit must use the new base SHA as parent"
    );

    // No regular force-update should have been issued on the update path.
    assert!(
        github.force_updated_branches().await.is_empty(),
        "update path must not force-reset the branch to base_sha (would close PR)"
    );

    // No plain batch_commit_files should have been issued on the update path.
    assert!(
        github.batch_commits().await.is_empty(),
        "update path should use batch_commit_files_rebased, not batch_commit_files"
    );
}

/// The update path must never set the branch tip equal to base_sha.
///
/// This is the regression test for the GitHub auto-close race: when
/// `force_update_branch(base_sha)` was called before `batch_commit_files`,
/// the release branch momentarily equalled the base branch and GitHub
/// auto-closed the open PR.  The fix uses `batch_commit_files_rebased` which
/// creates the commit atomically with `base_sha` as the parent, never going
/// through an intermediate state where branch == base.
#[tokio::test]
async fn test_orchestrate_update_does_not_force_reset_branch_to_base_sha() {
    let existing_pr = make_open_release_pr(99, "release/v2.0.0", None);

    let github = TestGitHub::new()
        .with_search_results(vec![existing_pr.clone()])
        .await
        .with_pr_by_number(existing_pr)
        .await;

    let orchestrator = ReleaseOrchestrator::new(default_config(), &github);

    let _ = orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(2, 0, 0),
            "- fix: patch entry [abcdef0123456789abcdef0123456789abcdef01]",
            "main",
            "new-master-sha",
            "corr-race",
        )
        .await
        .expect("orchestrate should succeed");

    // The branch must never have been set to the base SHA directly;
    // doing so would make branch == base and auto-close the PR.
    assert!(
        github.force_updated_branches().await.is_empty(),
        "force_update_branch must not be called on the update path"
    );

    // The rebased commit must target the new master SHA as its parent.
    let rebased = github.rebased_batch_commits().await;
    assert_eq!(rebased.len(), 1, "exactly one rebased commit expected");
    assert_eq!(
        rebased[0].3, "new-master-sha",
        "rebased commit parent must be the new base SHA, not an intermediate state"
    );
}

/// Existing PR has a lower version → rename (Renamed).
#[tokio::test]
async fn test_orchestrate_existing_lower_version_pr_renames() {
    let existing_pr = make_open_release_pr(10, "release/v1.0.0", None);

    let github = TestGitHub::new()
        .with_search_results(vec![existing_pr.clone()])
        .await;

    let orchestrator = ReleaseOrchestrator::new(default_config(), &github);

    let result = orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(1, 1, 0),
            "- feat: bump [ccddee112233445566778899aabbccdd00112233]",
            "main",
            "sha003",
            "corr-003",
        )
        .await
        .expect("orchestrate should succeed");

    assert!(
        matches!(result, OrchestratorResult::Renamed { .. }),
        "expected Renamed, got {result:?}"
    );

    // New branch for v1.1.0 must have been created.
    let branches = github.created_branches().await;
    assert!(
        branches.iter().any(|(b, _)| b == "release/v1.1.0"),
        "branch release/v1.1.0 not created; got {branches:?}"
    );

    // Old PR (number 10) should be closed.
    let updates = github.updated_prs().await;
    assert!(
        updates
            .iter()
            .any(|(num, _, _, state)| *num == 10 && state.as_deref() == Some("closed")),
        "old PR not closed; updates: {updates:?}"
    );

    // Old branch should be deleted.
    let deleted = github.deleted_branches().await;
    assert!(
        deleted.contains(&"release/v1.0.0".to_string()),
        "old branch not deleted; {deleted:?}"
    );

    // CHANGELOG.md should be committed to the new release branch via rebased commit.
    let rebased = github.rebased_batch_commits().await;
    assert_eq!(
        rebased.len(),
        1,
        "CHANGELOG.md should be committed to the new release branch via batch_commit_files_rebased"
    );
    assert!(rebased[0].1.contains(&"CHANGELOG.md".to_string()));
    assert_eq!(rebased[0].0, "release/v1.1.0");
    assert_eq!(
        rebased[0].3, "sha003",
        "rebased commit must use base_sha as parent"
    );

    // No plain batch_commit_files should have been called on the create path.
    assert!(
        github.batch_commits().await.is_empty(),
        "rename path should use batch_commit_files_rebased, not batch_commit_files"
    );
}

/// Existing PR has a *higher* version → NoOp (never downgrade).
#[tokio::test]
async fn test_orchestrate_existing_higher_version_pr_is_no_op() {
    let existing_pr = make_open_release_pr(55, "release/v2.0.0", None);

    let github = TestGitHub::new()
        .with_search_results(vec![existing_pr])
        .await;

    let orchestrator = ReleaseOrchestrator::new(default_config(), &github);

    let result = orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(1, 1, 0),
            "- chore: minor stuff [deadbeef012345678901234567890123456789ab]",
            "main",
            "sha004",
            "corr-004",
        )
        .await
        .expect("orchestrate should succeed");

    assert!(
        matches!(result, OrchestratorResult::NoOp { .. }),
        "expected NoOp, got {result:?}"
    );

    // Nothing should be mutated.
    assert!(github.created_branches().await.is_empty());
    assert!(github.created_prs().await.is_empty());
    assert!(github.updated_prs().await.is_empty());
    assert!(github.deleted_branches().await.is_empty());
    assert!(
        github.batch_commits().await.is_empty(),
        "NoOp should not commit any files"
    );
}

/// Branch already exists (conflict from create_branch) → orchestrator reuses it.
#[tokio::test]
async fn test_orchestrate_branch_already_exists_reuses_it() {
    let github = TestGitHub::new().with_next_create_branch_conflict().await;

    let orchestrator = ReleaseOrchestrator::new(default_config(), &github);

    let result = orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(1, 0, 0),
            "- fix: patch [ab12cd34ef5678901234abcdef12345678901234]",
            "main",
            "sha005",
            "corr-005",
        )
        .await
        .expect("orchestrate should succeed when branch already exists");

    // The result should still be Created, using the canonical branch name.
    if let OrchestratorResult::Created { branch_name, .. } = &result {
        assert_eq!(
            branch_name, "release/v1.0.0",
            "should reuse canonical branch, got {branch_name}"
        );
    } else {
        panic!("expected Created, got {result:?}");
    }

    // No new branch was successfully created (the conflict was on the only attempt).
    assert!(
        github.created_branches().await.is_empty(),
        "no successful branch creation expected when reusing existing"
    );

    // A PR should have been opened on the canonical branch.
    let prs = github.created_prs().await;
    assert_eq!(prs.len(), 1);
    assert_eq!(prs[0].head, "release/v1.0.0");

    // CHANGELOG.md should be committed via batch_commit_files_rebased so the branch
    // tip never passes through base_sha (which would auto-close the open PR).
    let rebased = github.rebased_batch_commits().await;
    assert_eq!(
        rebased.len(),
        1,
        "CHANGELOG.md should be committed via batch_commit_files_rebased on the conflict path"
    );
    assert!(rebased[0].1.contains(&"CHANGELOG.md".to_string()));
    assert_eq!(rebased[0].0, "release/v1.0.0");
    assert_eq!(
        rebased[0].3, "sha005",
        "rebased commit must use base_sha as parent, not an intermediate state"
    );

    // No plain batch_commit_files should have been called.
    assert!(
        github.batch_commits().await.is_empty(),
        "conflict path should use batch_commit_files_rebased, not batch_commit_files"
    );

    // force_update_branch must NOT have been called: it would set branch tip ==
    // base_sha, making head == base of any open PR and causing GitHub to auto-close it.
    assert!(
        github.force_updated_branches().await.is_empty(),
        "force_update_branch must not be called on the conflict path (would close open PR)"
    );
}

/// `search_pull_requests` returns an error → propagated to caller.
#[tokio::test]
async fn test_orchestrate_github_api_failure_propagates_error() {
    let github = TestGitHub::new().with_search_error().await;
    let orchestrator = ReleaseOrchestrator::new(default_config(), &github);

    let result = orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(1, 0, 0),
            "- fix: something",
            "main",
            "sha006",
            "corr-006",
        )
        .await;

    assert!(result.is_err(), "expected error from API failure");
}

/// Same-version update deduplicates commit entries by SHA.
#[tokio::test]
async fn test_orchestrate_changelog_merge_deduplicates_commits() {
    let sha_a = "aabbccddeeff00112233445566778899aabbccdd";
    let existing_body = format!("## Changelog\n\n- fix: existing [{sha_a}]");
    let existing_pr = make_open_release_pr(77, "release/v1.0.0", Some(&existing_body));

    let github = TestGitHub::new()
        .with_search_results(vec![existing_pr.clone()])
        .await
        .with_pr_by_number(existing_pr)
        .await;

    let orchestrator = ReleaseOrchestrator::new(default_config(), &github);

    // New changelog repeats the same SHA → should be deduped.
    let new_changelog = format!("- fix: existing (duplicate) [{sha_a}]");

    orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(1, 0, 0),
            &new_changelog,
            "main",
            "sha007",
            "corr-007",
        )
        .await
        .expect("orchestrate should succeed");

    let updates = github.updated_prs().await;
    let body = updates[0].2.as_deref().unwrap_or("");

    // The SHA should appear exactly once.
    let count = body.matches(sha_a).count();
    assert_eq!(
        count, 1,
        "duplicate SHA should be deduplicated; body:\n{body}"
    );

    // CHANGELOG.md should be committed via batch_commit_files_rebased (same-version
    // update path goes through update_release_pr which uses the rebased variant).
    let rebased = github.rebased_batch_commits().await;
    assert_eq!(
        rebased.len(),
        1,
        "CHANGELOG.md should be committed via batch_commit_files_rebased on the update path"
    );
    assert!(rebased[0].1.contains(&"CHANGELOG.md".to_string()));
    assert_eq!(rebased[0].0, "release/v1.0.0");
    assert_eq!(
        rebased[0].3, "sha007",
        "rebased commit must use base_sha as parent"
    );

    // No plain batch_commit_files should have been called on the update path.
    assert!(
        github.batch_commits().await.is_empty(),
        "update path should use batch_commit_files_rebased, not batch_commit_files"
    );
}

/// Branch name helper: `make_branch_name` formats as `"release/v{major}.{minor}.{patch}"`.
#[test]
fn test_make_branch_name_format() {
    let github = TestGitHub::new();
    let orchestrator = ReleaseOrchestrator::new(default_config(), &github);
    assert_eq!(
        orchestrator.make_branch_name(&ver(1, 2, 3)),
        "release/v1.2.3"
    );
    assert_eq!(
        orchestrator.make_branch_name(&ver(0, 1, 0)),
        "release/v0.1.0"
    );
}

/// Custom `OrchestratorConfig` — branch prefix is respected.
#[tokio::test]
async fn test_custom_branch_prefix_is_used() {
    let config = OrchestratorConfig {
        branch_prefix: "releases".to_string(),
        ..OrchestratorConfig::default()
    };
    let github = TestGitHub::new();
    let orchestrator = ReleaseOrchestrator::new(config, &github);

    let result = orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(2, 0, 0),
            "- feat: big change [ff00aabb1122334455667788990011223344556677]",
            "main",
            "sha008",
            "corr-008",
        )
        .await
        .expect("orchestrate should succeed");

    if let OrchestratorResult::Created { branch_name, .. } = result {
        assert_eq!(branch_name, "releases/v2.0.0");
    } else {
        panic!("expected Created");
    }
}

/// Config-file style `${version}` placeholder in title template is substituted correctly.
#[tokio::test]
async fn test_title_template_dollar_brace_syntax_is_substituted() {
    let config = OrchestratorConfig {
        title_template: "chore(release): ${version}".to_string(),
        ..OrchestratorConfig::default()
    };
    let github = TestGitHub::new();
    let orchestrator = ReleaseOrchestrator::new(config, &github);

    let result = orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(1, 2, 3),
            "- feat: something [ab12cd34ef5678901234abcdef12345678901234]",
            "main",
            "sha009",
            "corr-009",
        )
        .await
        .expect("orchestrate should succeed");

    if let OrchestratorResult::Created { .. } = result {
        let prs = github.created_prs().await;
        assert_eq!(prs.len(), 1);
        assert_eq!(prs[0].title, "chore(release): 1.2.3");
    } else {
        panic!("expected Created, got {result:?}");
    }
}

/// Config-file style `${version_tag}` placeholder in title template is substituted correctly.
#[tokio::test]
async fn test_title_template_dollar_brace_version_tag_syntax_is_substituted() {
    let config = OrchestratorConfig {
        title_template: "chore(release): ${version_tag}".to_string(),
        ..OrchestratorConfig::default()
    };
    let github = TestGitHub::new();
    let orchestrator = ReleaseOrchestrator::new(config, &github);

    let result = orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(1, 2, 3),
            "- feat: something [ab12cd34ef5678901234abcdef12345678901234]",
            "main",
            "sha010",
            "corr-010",
        )
        .await
        .expect("orchestrate should succeed");

    if let OrchestratorResult::Created { .. } = result {
        let prs = github.created_prs().await;
        assert_eq!(prs.len(), 1);
        assert_eq!(prs[0].title, "chore(release): v1.2.3");
    } else {
        panic!("expected Created, got {result:?}");
    }
}

/// Custom `body_template` is substituted into the PR body; only the current
/// release changelog appears under `${changelog}`.
#[tokio::test]
async fn test_body_template_is_used_in_pr_body() {
    let config = OrchestratorConfig {
        body_template: "# Release\n\n${changelog}\n\n---\n*auto-generated*".to_string(),
        ..OrchestratorConfig::default()
    };
    let github = TestGitHub::new();
    let orchestrator = ReleaseOrchestrator::new(config, &github);

    let changelog = "### Added\n\n- feat: shiny thing [ab12cd34ef5678901234abcdef12345678901234]";

    let result = orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(1, 3, 0),
            changelog,
            "main",
            "sha011",
            "corr-011",
        )
        .await
        .expect("orchestrate should succeed");

    if let OrchestratorResult::Created { .. } = result {
        let prs = github.created_prs().await;
        assert_eq!(prs.len(), 1);
        let body = prs[0].body.as_deref().unwrap_or("");
        // Template prefix and suffix preserved.
        assert!(
            body.starts_with("# Release\n\n"),
            "template prefix missing; body:\n{body}"
        );
        assert!(
            body.contains("---\n*auto-generated*"),
            "template suffix missing; body:\n{body}"
        );
        // Only current-release entries are present.
        assert!(
            body.contains("shiny thing"),
            "changelog entry missing; body:\n{body}"
        );
        // The old `## Changelog` sentinel is NOT present because we overrode it.
        assert!(
            !body.contains("## Changelog"),
            "default header should not appear in custom template; body:\n{body}"
        );
    } else {
        panic!("expected Created, got {result:?}");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests for internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/// `merge_changelog_sections` appends new unique entries and keeps existing ones.
#[test]
fn test_merge_changelog_sections_appends_new_unique_entries() {
    let sha_old = "0000000000000000000000000000000000000001";
    let sha_new = "0000000000000000000000000000000000000002";
    let existing = format!("- fix: old fix [{sha_old}]");
    let new_section = format!("- feat: new thing [{sha_new}]");

    let merged = merge_changelog_sections(&existing, &new_section);

    assert!(merged.contains("old fix"), "should keep existing entry");
    assert!(merged.contains("new thing"), "should add new entry");
}

/// `merge_changelog_sections` does not duplicate entries with the same SHA.
#[test]
fn test_merge_changelog_sections_deduplicates_by_sha() {
    let sha = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let existing = format!("- fix: thing [{sha}]");
    let new_section = format!("- fix: thing (dupe) [{sha}]");

    let merged = merge_changelog_sections(&existing, &new_section);

    assert_eq!(
        merged.matches(sha).count(),
        1,
        "SHA should appear exactly once; merged:\n{merged}"
    );
}

/// `merge_changelog_sections` with no new entries returns the existing section unchanged.
#[test]
fn test_merge_changelog_sections_returns_existing_when_no_new_entries() {
    let sha = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let existing = format!("- fix: same [{sha}]");
    let new_section = format!("- fix: same again [{sha}]");

    let merged = merge_changelog_sections(&existing, &new_section);
    assert_eq!(merged, existing);
}

/// `merge_changelog_sections` preserves a section header from `new_section`
/// when that header does not exist in the existing section.
///
/// Regression guard for the data-integrity bug where `### Breaking Changes`
/// entries appended from the new changelog lost their heading.
#[test]
fn test_merge_changelog_sections_preserves_new_section_headers() {
    let sha_old = "1111111111111111111111111111111111111111";
    let sha_new = "2222222222222222222222222222222222222222";

    let existing = format!("### Fixed\n\n- fix: old fix [{sha_old}]");
    let new_section = format!(
        "### Fixed\n\n- fix: old fix [{sha_old}]\n### Breaking Changes\n\n- breaking: new thing [{sha_new}]"
    );

    let merged = merge_changelog_sections(&existing, &new_section);

    assert!(
        merged.contains("### Breaking Changes"),
        "new section header should be preserved; merged:\n{merged}"
    );
    assert!(
        merged.contains("new thing"),
        "entry under new header should be present; merged:\n{merged}"
    );
    // The duplicate sha_old entry must not be duplicated.
    assert_eq!(
        merged.matches(sha_old).count(),
        1,
        "old SHA should appear exactly once; merged:\n{merged}"
    );
}

/// `merge_changelog_sections` deduplicates using abbreviated (7-char) SHAs.
///
/// Regression guard: git-cliff's body template historically truncated commit
/// SHAs to 7 characters.  Even though the template now emits full SHAs, the
/// deduplication logic must tolerate any SHA length ≥ 7 in case the user
/// supplies a custom git-cliff template that still uses short hashes.
#[test]
fn test_merge_changelog_sections_handles_short_sha() {
    let short_sha = "ab5749c";
    let existing = format!("- fix: previous fix [{short_sha}]");
    let new_section = format!("- fix: previous fix (dupe) [{short_sha}]");

    let merged = merge_changelog_sections(&existing, &new_section);

    assert_eq!(
        merged.matches(short_sha).count(),
        1,
        "short SHA should be recognised; merged:\n{merged}"
    );
}

/// `merge_changelog_sections` adds a new entry when the incoming section uses a
/// 7-char SHA that is not already present in the existing section.
#[test]
fn test_merge_changelog_sections_appends_new_entry_with_short_sha() {
    let old_sha = "0000001";
    let new_sha = "aaabbbc";
    let existing = format!("### Bug Fixes\n\n- fix: old thing [{old_sha}]");
    let new_section =
        format!("### Bug Fixes\n\n- fix: old thing [{old_sha}]\n- fix: new thing [{new_sha}]");

    let merged = merge_changelog_sections(&existing, &new_section);

    assert!(
        merged.contains("new thing"),
        "new entry should be appended; merged:\n{merged}"
    );
    assert_eq!(
        merged.matches(old_sha).count(),
        1,
        "old entry must not be duplicated; merged:\n{merged}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Mutation kill tests for merge_changelog_sections
// ─────────────────────────────────────────────────────────────────────────────

/// Lines whose `[...]` bracket token contains non-hex characters must NOT be
/// treated as commit-SHA entries.
///
/// Kills the `&&` → `||` mutation at line 1257 of `extract_sha` inside
/// `merge_changelog_sections`: with `||`, a 7-char non-hex token satisfies the
/// length check alone and is falsely treated as a SHA.
#[test]
fn test_merge_changelog_sections_non_hex_bracket_token_not_treated_as_sha() {
    let sha = "aaaaaaa";
    let existing = format!("- fix: old thing [{sha}]");
    // `[ZZZZZZZ]` is 7 chars but Z is not a hex digit.
    let new_section = "- feat: new entry [ZZZZZZZ]";

    let merged = merge_changelog_sections(&existing, new_section);

    // The new entry has no valid SHA so it must be ignored (not added).
    assert!(
        !merged.contains("new entry"),
        "non-hex bracket token must not be recognised as a SHA; merged:\n{merged}"
    );
}

/// A section header (`### …`) already present in the existing section must NOT
/// be re-emitted when merging new entries under the same header.
///
/// Kills the `&&` → `||` mutation at line 1292: with `||`, the condition is
/// true on the very first new entry even when the header already exists,
/// causing it to be duplicated.
#[test]
fn test_merge_changelog_sections_does_not_duplicate_existing_header() {
    let sha_old = "1111111111111111111111111111111111111111";
    let sha_new = "2222222222222222222222222222222222222222";

    let existing = format!("### Fixed\n\n- fix: old fix [{sha_old}]");
    let new_section =
        format!("### Fixed\n\n- fix: old fix [{sha_old}]\n- fix: new fix [{sha_new}]");

    let merged = merge_changelog_sections(&existing, &new_section);

    assert_eq!(
        merged.matches("### Fixed").count(),
        1,
        "### Fixed header must appear exactly once; merged:\n{merged}"
    );
    assert!(
        merged.contains("new fix"),
        "new entry must be present; merged:\n{merged}"
    );
}

/// When `existing_section` is empty the merged output must not begin with a
/// leading newline.
///
/// Kills the `delete !` mutation at line 1307:8 (`if result.is_empty() && …`)
/// and the `&&` → `||` mutation at line 1307:27: both cause `push('\n')` to
/// fire on an empty result, prepending a spurious newline.
#[test]
fn test_merge_changelog_sections_empty_existing_no_leading_newline() {
    let sha = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let new_section = format!("- feat: new thing [{sha}]");

    let merged = merge_changelog_sections("", &new_section);

    assert!(
        !merged.starts_with('\n'),
        "merged must not start with a newline when existing is empty; merged: {:?}",
        merged
    );
    assert!(
        merged.contains("new thing"),
        "new entry must be present; merged:\n{merged}"
    );
}

/// When `existing_section` ends with a newline the merged output must NOT
/// contain a double-newline separator before the appended entries.
///
/// Kills the `delete !` mutation at line 1307:30 (`… && result.ends_with('\n')`)
/// which pushes an extra newline when the existing content already ends with one.
#[test]
fn test_merge_changelog_sections_trailing_newline_no_double_newline() {
    let sha_old = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let sha_new = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    // existing_section explicitly ends with '\n'.
    let existing = format!("- fix: old fix [{sha_old}]\n");
    let new_section = format!("- feat: new thing [{sha_new}]");

    let merged = merge_changelog_sections(&existing, &new_section);

    assert!(
        !merged.contains("\n\n"),
        "merged must not contain a double-newline separator; merged: {:?}",
        merged
    );
    assert!(
        merged.contains("new thing"),
        "new entry must be present; merged:\n{merged}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests for build_changelog_file_content
// ─────────────────────────────────────────────────────────────────────────────

/// Empty existing content produces a fresh `# Changelog` header followed by
/// the new version section.
#[test]
fn test_build_changelog_file_content_empty_existing() {
    let result =
        build_changelog_file_content("", "1.0.0", "2024-01-15", "### Added\n\n- feat: new");

    assert!(
        result.starts_with("# Changelog\n"),
        "should start with file-level header; got:\n{result}"
    );
    assert!(
        result.contains("## [1.0.0] - 2024-01-15"),
        "should contain version section; got:\n{result}"
    );
    assert!(
        result.contains("feat: new"),
        "should contain changelog body; got:\n{result}"
    );
}

/// Existing history is preserved below the newly inserted section.
#[test]
fn test_build_changelog_file_content_with_existing_history() {
    let existing = "# Changelog\n\n## [0.9.0] - 2024-01-01\n\n### Added\n\n- feat: old\n";

    let result =
        build_changelog_file_content(existing, "1.0.0", "2024-01-15", "### Added\n\n- feat: new");

    let pos_new = result
        .find("## [1.0.0]")
        .expect("new section must be present");
    let pos_old = result
        .find("## [0.9.0]")
        .expect("old section must be preserved");
    assert!(
        pos_new < pos_old,
        "new section must appear before old section; got:\n{result}"
    );
}

/// Calling the function twice with the same version replaces the existing
/// section rather than duplicating it (idempotent behaviour).
#[test]
fn test_build_changelog_file_content_idempotent_same_version() {
    let existing = "# Changelog\n\n## [1.0.0] - 2024-01-15\n\n### Added\n\n- feat: first run\n";

    let result = build_changelog_file_content(
        existing,
        "1.0.0",
        "2024-01-15",
        "### Added\n\n- feat: second run",
    );

    assert_eq!(
        result.matches("## [1.0.0]").count(),
        1,
        "version section must not be duplicated; got:\n{result}"
    );
    assert!(
        result.contains("second run"),
        "updated body must be present; got:\n{result}"
    );
    assert!(
        !result.contains("first run"),
        "old body must be replaced; got:\n{result}"
    );
}

/// Content that already starts with a `## [version]` line (no file-level
/// `# Changelog` header) gets the header prepended automatically.
#[test]
fn test_build_changelog_file_content_no_header() {
    let existing = "## [0.1.0] - 2023-12-01\n\n### Fixed\n\n- fix: old\n";

    let result =
        build_changelog_file_content(existing, "1.0.0", "2024-01-15", "### Added\n\n- feat: new");

    assert!(
        result.starts_with("# Changelog\n"),
        "generated header must be present; got:\n{result}"
    );
    assert!(
        result.contains("## [1.0.0]"),
        "new section must be present; got:\n{result}"
    );
    assert!(
        result.contains("## [0.1.0]"),
        "old section must be preserved; got:\n{result}"
    );
}

/// Existing file-level header text (e.g. a description below `# Changelog`)
/// is preserved verbatim.
#[test]
fn test_build_changelog_file_content_preserves_file_header() {
    let existing =
        "# Changelog\n\nAll notable changes are documented here.\n\n## [0.2.0] - 2024-01-10\n\n### Fixed\n\n- fix: something\n";

    let result =
        build_changelog_file_content(existing, "1.0.0", "2024-01-15", "### Added\n\n- feat: new");

    assert!(
        result.contains("All notable changes are documented here."),
        "header description must be preserved; got:\n{result}"
    );
    assert!(
        result.contains("## [1.0.0]"),
        "new section must be present; got:\n{result}"
    );
    assert!(
        result.contains("## [0.2.0]"),
        "old section must be preserved; got:\n{result}"
    );
}

/// Content whose leading block starts with `### ` (not a proper `# ` file
/// header) is treated as garbage and replaced by a generated `# Changelog`
/// header rather than being preserved verbatim.
///
/// Regression guard for the secondary bug: corrupted CHANGELOG.md files written
/// by old release-regent (raw `### feat`/`### fix` blocks at the top) must not
/// be silently accepted as a valid file-level header.
#[test]
fn test_build_changelog_file_content_rejects_non_hash_prefix_as_file_header() {
    // Simulates the corrupted content written by old release-regent: raw
    // `### feat`/`### fix` section blocks at the top of the file, no
    // `# Changelog` file-level header, and only the current version section.
    let corrupted = "### feat\n- **config**: some commit [aabbccdd]\n### fix\n- **release**: another [bbccddee]\n\n## [1.0.0] - 2024-01-15\n\n### Features\n\n- feat: thing\n";

    let result =
        build_changelog_file_content(corrupted, "1.0.0", "2024-01-15", "### Added\n\n- feat: new");

    assert!(
        result.starts_with("# Changelog\n"),
        "corrupted non-header content must be replaced by '# Changelog'; got:\n{result}"
    );
    assert!(
        !result.contains("### feat\n"),
        "raw section headers must not appear as file-level header; got:\n{result}"
    );
    assert!(
        !result.contains("- **config**: some commit"),
        "corrupted entry lines must not appear anywhere in output; got:\n{result}"
    );
    assert!(
        result.contains("## [1.0.0]"),
        "version section must be present; got:\n{result}"
    );
}

/// Replacing an existing version section preserves subsequent section content
/// at the exact correct byte offset.
///
/// Kills the `+ with -` and `+ with *` mutations at lines 1232:63 and 1233:24
/// inside `skip_existing_version_section`: those mutations shift the slice
/// start by ±1 or via multiplication, corrupting the content of the section
/// that follows the replaced one.
#[test]
fn test_build_changelog_file_content_skip_preserves_subsequent_section_exactly() {
    let existing = concat!(
        "# Changelog\n\n",
        "## [1.0.0] - 2024-01-15\n\n",
        "### Added\n\n",
        "- feat: first run [aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa]\n\n",
        "## [0.9.0] - 2024-01-01\n\n",
        "### Fixed\n\n",
        "- fix: old thing [bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb]\n",
    );

    let result = build_changelog_file_content(
        existing,
        "1.0.0",
        "2024-01-15",
        "### Added\n\n- feat: second run [cccccccccccccccccccccccccccccccccccccccc]",
    );

    // The 0.9.0 heading must appear byte-exact — not shifted by ±1 character.
    assert!(
        result.contains("## [0.9.0] - 2024-01-01"),
        "subsequent section heading must be byte-exact; got:\n{result}"
    );
    assert!(
        result.contains("- fix: old thing"),
        "subsequent section body must be preserved; got:\n{result}"
    );
    let pos_100 = result.find("## [1.0.0]").expect("1.0.0 section missing");
    let pos_090 = result.find("## [0.9.0]").expect("0.9.0 section missing");
    assert!(
        pos_100 < pos_090,
        "1.0.0 must precede 0.9.0; got:\n{result}"
    );
    assert_eq!(
        result.matches("## [1.0.0]").count(),
        1,
        "version section must not be duplicated; got:\n{result}"
    );
}

/// `merge_changelog_bodies` must strip the changelog section header from the
/// merged output.
///
/// Kills the `+ with *` mutation at line 1010 of the private
/// `extract_changelog_from_body` method: when the header is at position 0 in
/// the PR body, `i * header.len()` evaluates to 0 and returns the full body
/// (header included) instead of the content after the header.
#[test]
fn test_merge_changelog_bodies_strips_header_from_extracted_section() {
    let sha_existing = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let sha_new = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    // Header at position 0 — maximises the visible difference when `+` becomes `*`.
    let pr_body = format!("## Changelog\n\n- fix: existing thing [{sha_existing}]\n");
    let new_changelog = format!("- feat: new thing [{sha_new}]");

    let github = TestGitHub::new();
    let orchestrator = ReleaseOrchestrator::new(default_config(), &github);
    let merged = orchestrator.merge_changelog_bodies(&pr_body, &new_changelog);

    assert!(
        !merged.contains("## Changelog"),
        "merge_changelog_bodies must strip the changelog header; got:\n{merged}"
    );
    assert!(
        merged.contains("existing thing"),
        "existing entry must be retained; got:\n{merged}"
    );
    assert!(
        merged.contains("new thing"),
        "new entry must be merged in; got:\n{merged}"
    );
}

/// `update_release_pr` reads CHANGELOG.md from the PR *base* branch (e.g.
/// `main`), not from the PR head branch.
///
/// Regression guard for the primary bug: when the head branch contains a
/// corrupted CHANGELOG.md (written by an older release-regent), the update
/// path must still produce a correct file with history by reading from the
/// authoritative base branch.
#[tokio::test]
async fn test_update_release_pr_reads_changelog_from_base_branch() {
    let version = ver(1, 0, 0);

    let corrupted_head_changelog =
        "### feat\n- some commit [aabbccdd]\n\n## [1.0.0] - 2026-01-01\n\n### Features\n\n- feat: thing\n";
    let correct_base_changelog =
        "# Changelog\n\nAll notable changes.\n\n## [0.9.0] - 2025-12-01\n\n### Fixed\n\n- fix: old thing [cccccccccccccccccccccccccccccccccccccccc]\n";

    let existing_body =
        "## Changelog\n\n- feat: something [1234567890abcdef1234567890abcdef12345678]";
    let existing_pr = make_open_release_pr(55, "release/v1.0.0", Some(existing_body));

    let github = TestGitHub::new()
        .with_search_results(vec![existing_pr.clone()])
        .await
        .with_pr_by_number(existing_pr)
        .await
        // Corrupted content on the head branch.
        .with_file_content("CHANGELOG.md", "release/v1.0.0", corrupted_head_changelog)
        .await
        // Correct content with history on the base branch.
        .with_file_content("CHANGELOG.md", "main", correct_base_changelog)
        .await;

    let orchestrator = ReleaseOrchestrator::new(default_config(), &github);

    orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &version,
            "- feat: new feature [1234567890abcdef1234567890abcdef12345678]",
            "main",
            "sha-base-001",
            "corr-update-001",
        )
        .await
        .expect("orchestrate should succeed");

    let file_updates = github.rebased_batch_file_updates().await;
    assert_eq!(
        file_updates.len(),
        1,
        "expected exactly one rebased batch commit"
    );

    let changelog_update = file_updates[0]
        .iter()
        .find(|f| f.path == "CHANGELOG.md")
        .expect("CHANGELOG.md must be in the committed files");

    assert!(
        changelog_update.content.starts_with("# Changelog\n"),
        "committed CHANGELOG.md must start with '# Changelog'; got:\n{}",
        changelog_update.content
    );
    assert!(
        changelog_update.content.contains("## [0.9.0]"),
        "history from base branch must be preserved; got:\n{}",
        changelog_update.content
    );
    assert!(
        !changelog_update.content.contains("### feat\n"),
        "corrupted head-branch header must not appear; got:\n{}",
        changelog_update.content
    );
}

/// `update_release_pr` reads manifest files (e.g. `Cargo.toml`) from the PR
/// *base* branch (e.g. `main`), not from the PR head branch.
///
/// Regression guard for the bug where manifests were read from the release
/// branch head, causing dependency versions to be silently downgraded on every
/// rebase cycle.  The head branch carries the bot's own previous output; the
/// base branch is always authoritative for dependency content.
#[tokio::test]
async fn test_update_release_pr_reads_manifests_from_base_branch() {
    let version = ver(1, 0, 0);

    // Head branch has a stale Cargo.toml with old dependency versions.
    let stale_head_cargo =
        "[package]\nname = \"myapp\"\nversion = \"0.1.0\"\n\n[dependencies]\ntokio = \"1.0\"\n";
    // Base branch has the current Cargo.toml with up-to-date dependency versions.
    let fresh_base_cargo =
        "[package]\nname = \"myapp\"\nversion = \"0.1.0\"\n\n[dependencies]\ntokio = \"1.47\"\n";

    let existing_body =
        "## Changelog\n\n- feat: something [1234567890abcdef1234567890abcdef12345678]";
    let existing_pr = make_open_release_pr(55, "release/v1.0.0", Some(existing_body));

    let github = TestGitHub::new()
        .with_search_results(vec![existing_pr.clone()])
        .await
        .with_pr_by_number(existing_pr)
        .await
        // Stale content on the head branch — must NOT be used.
        .with_file_content("Cargo.toml", "release/v1.0.0", stale_head_cargo)
        .await
        // Fresh content on the base branch — must be used.
        .with_file_content("Cargo.toml", "main", fresh_base_cargo)
        .await;

    let orchestrator = ReleaseOrchestrator::new(default_config(), &github);

    orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &version,
            "- feat: new feature [1234567890abcdef1234567890abcdef12345678]",
            "main",
            "sha-base-002",
            "corr-update-002",
        )
        .await
        .expect("orchestrate should succeed");

    let file_updates = github.rebased_batch_file_updates().await;
    assert_eq!(
        file_updates.len(),
        1,
        "expected exactly one rebased batch commit"
    );

    let cargo_update = file_updates[0]
        .iter()
        .find(|f| f.path == "Cargo.toml")
        .expect("Cargo.toml must be present in the committed file updates");

    assert!(
        cargo_update.content.contains("tokio = \"1.47\""),
        "committed Cargo.toml must have dep versions from the base branch; got:\n{}",
        cargo_update.content
    );
    assert!(
        !cargo_update.content.contains("tokio = \"1.0\""),
        "stale dep versions from the head branch must not appear; got:\n{}",
        cargo_update.content
    );
    assert!(
        cargo_update.content.contains("1.0.0"),
        "version key must be updated to the release version; got:\n{}",
        cargo_update.content
    );
}

/// When `search_pull_requests` returns multiple open release PRs the
/// orchestrator selects the highest-versioned one, not the first match.
#[tokio::test]
async fn test_orchestrate_selects_highest_version_when_multiple_release_prs_exist() {
    // GitHub happens to return the lower-versioned PR first.
    let older_pr = make_open_release_pr(10, "release/v1.0.0", None);
    let newer_pr = make_open_release_pr(20, "release/v2.0.0", None);

    let github = TestGitHub::new()
        .with_search_results(vec![older_pr, newer_pr])
        .await;

    let orchestrator = ReleaseOrchestrator::new(default_config(), &github);

    // New version is 1.5.0 — lower than v2.0.0 (the highest existing PR).
    // Expected outcome: NoOp because the highest existing PR (v2.0.0) > v1.5.0.
    let result = orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(1, 5, 0),
            "- feat: something [cccccccccccccccccccccccccccccccccccccccc]",
            "main",
            "sha010",
            "corr-010",
        )
        .await
        .expect("orchestrate should succeed");

    assert!(
        matches!(result, OrchestratorResult::NoOp { .. }),
        "expected NoOp because v2.0.0 > v1.5.0, got {result:?}"
    );

    // Nothing should be mutated.
    assert!(github.created_branches().await.is_empty());
    assert!(github.created_prs().await.is_empty());
}

/// In the equal-version update path, `update_pull_request` is NOT called with
/// a title when the rendered title matches the existing PR title.
#[tokio::test]
async fn test_update_release_pr_does_not_patch_title_when_unchanged() {
    let version = ver(1, 0, 0);
    let config = default_config();

    // The default title template is "chore(release): {version_tag}".
    // For v1.0.0 this produces "chore(release): v1.0.0".
    let rendered_title = config
        .title_template
        .replace("{version}", "1.0.0")
        .replace("{version_tag}", "v1.0.0");

    let existing_body = "## Changelog\n\n- fix: old fix [aabbccddeeff00112233445566778899aabbccdd]";
    let mut existing_pr = make_open_release_pr(42, "release/v1.0.0", Some(existing_body));
    // Use the exact title the orchestrator would render — no title change expected.
    existing_pr.title = rendered_title;

    let github = TestGitHub::new()
        .with_search_results(vec![existing_pr.clone()])
        .await
        .with_pr_by_number(existing_pr)
        .await;

    let orchestrator = ReleaseOrchestrator::new(config, &github);

    orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &version,
            "- feat: new feature [1122334455667788990011223344556677889900]",
            "main",
            "sha011",
            "corr-011",
        )
        .await
        .expect("orchestrate should succeed");

    let updates = github.updated_prs().await;
    assert_eq!(updates.len(), 1);
    // Title field should be None because it has not changed.
    assert!(
        updates[0].1.is_none(),
        "title should not be patched when unchanged; got {:?}",
        updates[0].1
    );
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// extract_changelog_from_pr_body — free-function unit tests (task 9.30.1)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_extract_changelog_from_pr_body_returns_section_content() {
    let body = "## Changelog\n\n- feat: add widget [abc123]\n\n## Notes\n\nSee wiki.";
    let result = extract_changelog_from_pr_body(body, "## Changelog");
    assert_eq!(result, "- feat: add widget [abc123]");
}

#[test]
fn test_extract_changelog_from_pr_body_returns_empty_when_header_not_found() {
    let body = "# Release v1.0.0\n\nSome description without a changelog header.";
    let result = extract_changelog_from_pr_body(body, "## Changelog");
    assert_eq!(result, "");
}

#[test]
fn test_extract_changelog_from_pr_body_returns_remainder_when_no_next_heading() {
    let body = "## Changelog\n\n- fix: patch issue [def456]\n- feat: new thing [ghi789]";
    let result = extract_changelog_from_pr_body(body, "## Changelog");
    assert_eq!(
        result,
        "- fix: patch issue [def456]\n- feat: new thing [ghi789]"
    );
}

#[test]
fn test_extract_changelog_from_pr_body_trims_surrounding_whitespace() {
    let body = "## Changelog\n\n\n  - feat: widget [abc]\n\n\n";
    let result = extract_changelog_from_pr_body(body, "## Changelog");
    assert_eq!(result, "- feat: widget [abc]");
}

#[test]
fn test_extract_changelog_from_pr_body_handles_custom_header() {
    let body = "## Release Notes\n\n- feat: something\n\n## Changelog\n\n- other";
    let result = extract_changelog_from_pr_body(body, "## Release Notes");
    assert_eq!(result, "- feat: something");
}

#[test]
fn test_extract_changelog_from_pr_body_empty_section_returns_empty() {
    let body = "## Changelog\n\n## Notes\n\nSome notes.";
    let result = extract_changelog_from_pr_body(body, "## Changelog");
    assert_eq!(result, "");
}

#[test]
fn test_extract_changelog_from_pr_body_empty_body_returns_empty() {
    let result = extract_changelog_from_pr_body("", "## Changelog");
    assert_eq!(result, "");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// extract_changelog_header — free-function unit tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_extract_changelog_header_returns_default_heading_from_default_template() {
    let header = super::extract_changelog_header(
        crate::release_orchestrator::OrchestratorConfig::DEFAULT_BODY_TEMPLATE,
    );
    assert_eq!(header, "## Changelog");
}

#[test]
fn test_extract_changelog_header_returns_custom_heading() {
    let template = "## Release Notes\n\n${changelog}";
    assert_eq!(
        super::extract_changelog_header(template),
        "## Release Notes"
    );
}

#[test]
fn test_extract_changelog_header_falls_back_when_no_heading_before_placeholder() {
    assert_eq!(
        super::extract_changelog_header("${changelog}"),
        "## Changelog"
    );
}

#[test]
fn test_extract_changelog_header_ignores_heading_after_placeholder() {
    let template = "${changelog}\n\n## Ignored";
    assert_eq!(super::extract_changelog_header(template), "## Changelog");
}

#[test]
fn test_extract_changelog_header_picks_nearest_heading_when_multiple_precede_placeholder() {
    let template = "## Top\n\nSome text\n\n## Release Notes\n\n${changelog}";
    assert_eq!(
        super::extract_changelog_header(template),
        "## Release Notes"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// cargo_workspace_member_cargo_tomls — private helper unit tests
// ─────────────────────────────────────────────────────────────────────────────

/// Verify that malformed TOML input returns an empty vec without panicking.
#[test]
fn test_cargo_workspace_member_cargo_tomls_malformed_toml_returns_empty() {
    let result = cargo_workspace_member_cargo_tomls("this is not toml @@@ {{{");
    assert!(
        result.is_empty(),
        "malformed TOML should return empty vec, got {result:?}"
    );
}

/// Verify that a `Cargo.toml` with no `[workspace]` table returns an empty vec.
#[test]
fn test_cargo_workspace_member_cargo_tomls_no_workspace_table_returns_empty() {
    let content = "[package]\nname = \"myapp\"\nversion = \"0.1.0\"\n";
    let result = cargo_workspace_member_cargo_tomls(content);
    assert!(
        result.is_empty(),
        "no [workspace] table should return empty vec, got {result:?}"
    );
}

/// Verify that a workspace without a `members` array returns an empty vec.
#[test]
fn test_cargo_workspace_member_cargo_tomls_no_members_returns_empty() {
    let content = "[workspace]\nresolver = \"2\"\n";
    let result = cargo_workspace_member_cargo_tomls(content);
    assert!(
        result.is_empty(),
        "workspace without members should return empty vec, got {result:?}"
    );
}

/// Verify that glob patterns in the workspace `members` array are filtered out.
///
/// Glob characters `*`, `?`, `[`, and `{` all disqualify an entry from being
/// returned because filesystem enumeration is not available here.
#[test]
fn test_cargo_workspace_member_cargo_tomls_globs_are_filtered() {
    let content =
        "[workspace]\nmembers = [\"crates/*\", \"tools/?\", \"lib/[a-z]*\", \"ext/{a,b}\"]\n";
    let result = cargo_workspace_member_cargo_tomls(content);
    assert!(
        result.is_empty(),
        "all glob patterns should be filtered, got {result:?}"
    );
}

/// Verify that explicit (non-glob) workspace member paths are returned as
/// `<member>/Cargo.toml` strings, while glob entries in the same list are
/// silently skipped.
#[test]
fn test_cargo_workspace_member_cargo_tomls_explicit_paths_returned() {
    let content = "[workspace]\nmembers = [\"crates/foo\", \"crates/bar\", \"crates/*\"]\n";
    let result = cargo_workspace_member_cargo_tomls(content);
    assert_eq!(
        result.len(),
        2,
        "should return exactly two explicit members (glob skipped), got {result:?}"
    );
    assert!(
        result.contains(&"crates/foo/Cargo.toml".to_string()),
        "missing crates/foo/Cargo.toml in {result:?}"
    );
    assert!(
        result.contains(&"crates/bar/Cargo.toml".to_string()),
        "missing crates/bar/Cargo.toml in {result:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// dedup_file_updates_by_path — private helper unit tests
// ─────────────────────────────────────────────────────────────────────────────

/// Verify that an empty input list is returned unchanged with no panic.
#[test]
fn test_dedup_file_updates_by_path_empty_input_returns_empty() {
    let result = dedup_file_updates_by_path(vec![]);
    assert!(result.is_empty(), "empty input should return empty vec");
}

/// Verify that a list with no duplicate paths is returned unchanged (same
/// length and same order).
#[test]
fn test_dedup_file_updates_by_path_no_duplicates_returns_input_unchanged() {
    let updates = vec![
        FileUpdate {
            path: "Cargo.toml".to_string(),
            content: "version1".to_string(),
        },
        FileUpdate {
            path: "package.json".to_string(),
            content: "version2".to_string(),
        },
    ];
    let result = dedup_file_updates_by_path(updates);
    assert_eq!(result.len(), 2, "no duplicates should not drop any entries");
    assert_eq!(result[0].path, "Cargo.toml");
    assert_eq!(result[1].path, "package.json");
}

/// Verify that when two entries share the same path the **last** one is kept.
///
/// `detect_standard_manifests` emits `package.version` first and
/// `workspace.package.version` last for the root `Cargo.toml`, so after
/// deduplication the workspace key must win.
#[test]
fn test_dedup_file_updates_by_path_last_entry_wins_for_duplicates() {
    let updates = vec![
        FileUpdate {
            path: "Cargo.toml".to_string(),
            content: "package.version update".to_string(),
        },
        FileUpdate {
            path: "Cargo.toml".to_string(),
            content: "workspace.package.version update".to_string(),
        },
    ];
    let result = dedup_file_updates_by_path(updates);
    assert_eq!(
        result.len(),
        1,
        "duplicate path should be collapsed to one entry"
    );
    assert_eq!(
        result[0].content, "workspace.package.version update",
        "last entry must win; got content: {:?}",
        result[0].content
    );
}

/// Verify that mixed duplicate and unique paths produce the correct selection:
/// unique paths are kept as-is; duplicate paths keep only the last occurrence.
/// Relative order in the output follows the position of the last (surviving)
/// occurrence of each path in the original list.
#[test]
fn test_dedup_file_updates_by_path_mixed_keeps_correct_entries() {
    // Cargo.toml twice (index 0 and 2), package.json once (index 1).
    // After dedup: package.json (last-seen at index 1) comes before Cargo.toml
    // (last-seen at index 2), preserving the relative order of last occurrences.
    let updates = vec![
        FileUpdate {
            path: "Cargo.toml".to_string(),
            content: "first".to_string(),
        },
        FileUpdate {
            path: "package.json".to_string(),
            content: "unique".to_string(),
        },
        FileUpdate {
            path: "Cargo.toml".to_string(),
            content: "last".to_string(),
        },
    ];
    let result = dedup_file_updates_by_path(updates);
    assert_eq!(result.len(), 2, "should collapse Cargo.toml to one entry");
    // package.json (last-seen at original index 1) precedes Cargo.toml (last-seen at index 2).
    assert_eq!(result[0].path, "package.json");
    assert_eq!(result[0].content, "unique");
    assert_eq!(result[1].path, "Cargo.toml");
    assert_eq!(result[1].content, "last", "last Cargo.toml entry must win");
}

/// `dedup_file_updates_by_path` must emit a `warn!` when duplicate paths are
/// present (kills the `delete !` mutation at line 1116 that inverts the guard
/// to `if duplicates.is_empty()`).
#[test]
#[traced_test]
fn test_dedup_file_updates_by_path_warns_when_duplicates_present() {
    let updates = vec![
        FileUpdate {
            path: "Cargo.toml".to_string(),
            content: "first".to_string(),
        },
        FileUpdate {
            path: "Cargo.toml".to_string(),
            content: "second".to_string(),
        },
    ];
    let _ = dedup_file_updates_by_path(updates);
    assert!(
        logs_contain("Duplicate manifest paths"),
        "warning must be emitted when duplicates are present"
    );
}

/// `dedup_file_updates_by_path` must NOT emit the duplicate warning when all
/// paths are unique (kills the inverted `delete !` mutation at line 1116).
#[test]
#[traced_test]
fn test_dedup_file_updates_by_path_no_warning_when_no_duplicates() {
    let updates = vec![
        FileUpdate {
            path: "Cargo.toml".to_string(),
            content: "v1".to_string(),
        },
        FileUpdate {
            path: "package.json".to_string(),
            content: "v2".to_string(),
        },
    ];
    let _ = dedup_file_updates_by_path(updates);
    assert!(
        !logs_contain("Duplicate manifest paths"),
        "warning must NOT be emitted when there are no duplicates"
    );
}

/// `collect_manifest_updates` must populate the file-update list with
/// version-updated content for auto-detected manifests.
///
/// Kills the mutation that replaces the entire `collect_manifest_updates` body
/// with `()`: with that mutation the `Cargo.toml` update is never appended and
/// the batch commit does not contain it.
#[tokio::test]
async fn test_orchestrate_create_pr_includes_manifest_file_update() {
    // Pre-load Cargo.toml on the base branch; the create path now reads manifests
    // from the base branch (not the release branch head) so fresh deps are used.
    let cargo_toml_initial = "[package]\nname = \"myapp\"\nversion = \"0.1.0\"\n";
    let github = TestGitHub::new()
        .with_file_content("Cargo.toml", "main", cargo_toml_initial)
        .await;

    let orchestrator = ReleaseOrchestrator::new(default_config(), &github);

    orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(1, 2, 3),
            "- feat: add thing [abc1234567890123456789012345678901234567a]",
            "main",
            "sha-manifest-001",
            "corr-manifest-001",
        )
        .await
        .expect("orchestrate should succeed");

    let file_updates = github.rebased_batch_file_updates().await;
    assert_eq!(
        file_updates.len(),
        1,
        "expected exactly one rebased batch commit"
    );

    let cargo_update = file_updates[0]
        .iter()
        .find(|f| f.path == "Cargo.toml")
        .expect("Cargo.toml must be present in the committed file updates");

    assert!(
        cargo_update.content.contains("1.2.3"),
        "Cargo.toml update must contain the new version string; got:\n{}",
        cargo_update.content
    );
}

/// When Cargo.toml is updated as part of a release commit, the orchestrator
/// must also include an updated Cargo.lock whose workspace package version
/// entries match the new release version.
///
/// Without this, `cargo build --locked` fails in Docker because the lock file
/// still records the old workspace version while Cargo.toml declares the new one.
#[tokio::test]
async fn test_orchestrate_create_pr_updates_cargo_lock_workspace_versions() {
    let cargo_toml = "[workspace]\npackage.version = \"0.1.0\"\nmembers = []\n";
    // Cargo.lock with one workspace package (no source field) and one external crate.
    let cargo_lock = concat!(
        "version = 4\n\n",
        "[[package]]\n",
        "name = \"my-app\"\n",
        "version = \"0.1.0\"\n\n",
        "[[package]]\n",
        "name = \"serde\"\n",
        "version = \"1.0.0\"\n",
        "source = \"registry+https://github.com/rust-lang/crates.io-index\"\n",
    );

    let github = TestGitHub::new()
        .with_file_content("Cargo.toml", "main", cargo_toml)
        .await
        .with_file_content("Cargo.lock", "main", cargo_lock)
        .await;

    let orchestrator = ReleaseOrchestrator::new(default_config(), &github);

    orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(1, 2, 3),
            "- feat: add thing [abc1234567890123456789012345678901234567a]",
            "main",
            "sha-lock-001",
            "corr-lock-001",
        )
        .await
        .expect("orchestrate should succeed");

    let file_updates = github.rebased_batch_file_updates().await;
    assert_eq!(
        file_updates.len(),
        1,
        "expected exactly one rebased batch commit"
    );

    let lock_update = file_updates[0]
        .iter()
        .find(|f| f.path == "Cargo.lock")
        .expect("Cargo.lock must be present in the committed file updates");

    assert!(
        lock_update.content.contains("name = \"my-app\""),
        "Cargo.lock update must contain the workspace package; got:\n{}",
        lock_update.content
    );
    assert!(
        lock_update.content.contains("version = \"1.2.3\""),
        "workspace package version must be updated to 1.2.3; got:\n{}",
        lock_update.content
    );
    assert!(
        lock_update.content.contains("version = \"1.0.0\""),
        "external crate serde must keep its version; got:\n{}",
        lock_update.content
    );
}

/// When Cargo.lock is absent from the base branch the orchestrator silently
/// skips the lock-file update and still succeeds.
///
/// Regression guard for the `Ok(None)` branch in `collect_manifest_updates`.
#[tokio::test]
async fn test_orchestrate_create_pr_succeeds_when_cargo_lock_absent() {
    // Cargo.toml present, Cargo.lock absent.
    let cargo_toml = "[workspace]\npackage.version = \"0.1.0\"\nmembers = []\n";
    let github = TestGitHub::new()
        .with_file_content("Cargo.toml", "main", cargo_toml)
        .await;

    let orchestrator = ReleaseOrchestrator::new(default_config(), &github);

    orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(1, 2, 3),
            "- feat: thing [abc1234567890123456789012345678901234567a]",
            "main",
            "sha-nolock-001",
            "corr-nolock-001",
        )
        .await
        .expect("orchestrate should succeed even without a Cargo.lock");

    let file_updates = github.rebased_batch_file_updates().await;
    assert_eq!(
        file_updates.len(),
        1,
        "expected exactly one rebased batch commit"
    );

    // Cargo.lock must not appear in the commit — it wasn't present.
    assert!(
        file_updates[0].iter().all(|f| f.path != "Cargo.lock"),
        "Cargo.lock must not be in the commit when it is absent from the branch"
    );
    // Cargo.toml update must still be present.
    assert!(
        file_updates[0].iter().any(|f| f.path == "Cargo.toml"),
        "Cargo.toml must still be committed"
    );
}

/// When `auto_detect_manifests` is disabled and Cargo.toml is listed explicitly,
/// the Cargo.lock update block still fires because it runs after both manifest
/// branches.
#[tokio::test]
async fn test_orchestrate_explicit_manifest_config_also_updates_cargo_lock() {
    let cargo_toml = "[package]\nname = \"my-app\"\nversion = \"0.1.0\"\n";
    let cargo_lock = concat!(
        "version = 4\n\n",
        "[[package]]\n",
        "name = \"my-app\"\n",
        "version = \"0.1.0\"\n",
    );

    let config = OrchestratorConfig {
        auto_detect_manifests: false,
        manifest_files: vec![crate::manifest::ManifestFileConfig {
            path: "Cargo.toml".to_string(),
            format: crate::manifest::ManifestFormat::Toml,
            version_key: "package.version".to_string(),
        }],
        ..OrchestratorConfig::default()
    };

    let github = TestGitHub::new()
        .with_file_content("Cargo.toml", "main", cargo_toml)
        .await
        .with_file_content("Cargo.lock", "main", cargo_lock)
        .await;

    let orchestrator = ReleaseOrchestrator::new(config, &github);

    orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(2, 0, 0),
            "- feat: explicit [abc1234567890123456789012345678901234567a]",
            "main",
            "sha-explicit-001",
            "corr-explicit-001",
        )
        .await
        .expect("orchestrate should succeed");

    let file_updates = github.rebased_batch_file_updates().await;
    assert_eq!(
        file_updates.len(),
        1,
        "expected exactly one rebased batch commit"
    );

    let lock_update = file_updates[0]
        .iter()
        .find(|f| f.path == "Cargo.lock")
        .expect("Cargo.lock must be updated even in explicit-manifest mode");

    assert!(
        lock_update.content.contains("version = \"2.0.0\""),
        "Cargo.lock workspace version must be bumped to 2.0.0; got:\n{}",
        lock_update.content
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Property-based tests
//
// These tests verify structural invariants of `extract_changelog_from_pr_body`
// that unit tests alone cannot exhaustively cover — in particular, that the
// function is total (never panics), always returns trimmed output, and never
// produces more header occurrences than were present in the input body.
// ─────────────────────────────────────────────────────────────────────────────

mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// `extract_changelog_from_pr_body` is total: it never panics for any
        /// combination of body and header strings, including empty strings,
        /// unicode, and strings with embedded newlines.
        #[test]
        fn prop_extract_changelog_never_panics(
            body   in proptest::arbitrary::any::<String>(),
            header in "[^\n]{0,30}",
        ) {
            let _ = extract_changelog_from_pr_body(&body, &header);
        }

        /// An empty body always yields an empty result, regardless of the header.
        #[test]
        fn prop_extract_changelog_empty_body_yields_empty(
            header in "[^\n]{1,30}",
        ) {
            let result = extract_changelog_from_pr_body("", &header);
            prop_assert!(result.is_empty(), "empty body must produce empty output");
        }

        /// A body that does not contain the header always yields an empty result.
        #[test]
        fn prop_extract_changelog_absent_header_yields_empty(
            body in "[^@]{0,200}",
        ) {
            // Use a sentinel that cannot appear in the generated body.
            let result = extract_changelog_from_pr_body(&body, "@@UNREACHABLE_SENTINEL@@");
            prop_assert!(result.is_empty(), "absent header must produce empty output");
        }

        /// The result is always trimmed: no leading or trailing ASCII whitespace.
        #[test]
        fn prop_extract_changelog_result_is_always_trimmed(
            body   in proptest::arbitrary::any::<String>(),
            header in "[^\n]{1,20}",
        ) {
            let result = extract_changelog_from_pr_body(&body, &header);
            prop_assert_eq!(
                result.as_str(),
                result.trim(),
                "output must be trimmed"
            );
        }

        /// For a realistically-structured PR body (header appears exactly once at
        /// the start of its own line, content lines contain no `#` characters), the
        /// extracted result must not contain any occurrence of the header string.
        #[test]
        fn prop_extract_changelog_section_content_does_not_contain_header(
            pre_lines  in prop::collection::vec("[^#\n]{0,40}", 0..5),
            post_lines in prop::collection::vec("[^#\n]{0,40}", 0..5),
            content    in prop::collection::vec("[^#\n]{0,60}", 0..10),
            header     in "## [A-Z][a-z]{3,15}",
        ) {
            let pre          = pre_lines.join("\n");
            let body_content = content.join("\n");
            let post         = format!("## OtherSection\n{}", post_lines.join("\n"));
            let body         = format!("{pre}\n{header}\n\n{body_content}\n\n{post}");

            let result  = extract_changelog_from_pr_body(&body, &header);
            let count   = result.matches(header.as_str()).count();
            prop_assert_eq!(
                count,
                0,
                "extracted section must not contain header '{}'; got '{}'",
                header,
                result
            );
        }

        /// For any body that contains the header at least once, the output contains
        /// strictly fewer occurrences of the header string than the original body.
        /// This confirms that the extraction consumes (strips) at least one occurrence.
        #[test]
        fn prop_extract_changelog_output_has_fewer_header_occurrences_than_input(
            prefix  in "[^\n]{0,30}",
            content in "[^\n]{0,100}",
            suffix  in "[^\n]{0,30}",
            header  in "## [A-Z][a-z]{3,10}",
        ) {
            // The header appears at least once: after `prefix\n`.
            let body = format!("{prefix}\n{header}\n{content}\n{suffix}");

            let body_count   = body.matches(header.as_str()).count();
            let result       = extract_changelog_from_pr_body(&body, &header);
            let result_count = result.matches(header.as_str()).count();

            // body is constructed as `"{prefix}\n{header}\n{content}\n{suffix}"` so
            // body_count is always ≥ 1 — the header is unconditionally present.
            prop_assert!(
                result_count < body_count,
                "result occurrences ({result_count}) must be < body occurrences ({body_count})"
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// render_body template variable substitution
// ─────────────────────────────────────────────────────────────────────────────

/// `${version}` in the body template is replaced with the plain version string.
#[tokio::test]
async fn test_body_template_version_variable_is_substituted() {
    let config = OrchestratorConfig {
        body_template: "Release ${version} is here.".to_string(),
        ..OrchestratorConfig::default()
    };
    let github = TestGitHub::new();
    let orchestrator = ReleaseOrchestrator::new(config, &github);

    orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(2, 3, 4),
            "- feat: something [ab12cd34ef5678901234abcdef12345678901234]",
            "main",
            "sha-rv-001",
            "corr-rv-001",
        )
        .await
        .expect("orchestrate should succeed");

    let prs = github.created_prs().await;
    let body = prs[0].body.as_deref().unwrap_or("");
    assert!(
        body.contains("Release 2.3.4 is here."),
        "body must contain substituted version; body:\n{body}"
    );
}

/// `${version_tag}` in the body template is replaced with the prefixed version.
#[tokio::test]
async fn test_body_template_version_tag_variable_is_substituted() {
    let config = OrchestratorConfig {
        body_template: "Tag: ${version_tag}".to_string(),
        ..OrchestratorConfig::default()
    };
    let github = TestGitHub::new();
    let orchestrator = ReleaseOrchestrator::new(config, &github);

    orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(1, 0, 0),
            "- feat: something [ab12cd34ef5678901234abcdef12345678901234]",
            "main",
            "sha-rvt-001",
            "corr-rvt-001",
        )
        .await
        .expect("orchestrate should succeed");

    let prs = github.created_prs().await;
    let body = prs[0].body.as_deref().unwrap_or("");
    assert!(
        body.contains("Tag: v1.0.0"),
        "body must contain substituted version_tag; body:\n{body}"
    );
}

/// `${date}` in the body template is replaced with a full ISO 8601 timestamp.
#[tokio::test]
async fn test_body_template_date_variable_is_iso8601_timestamp() {
    let config = OrchestratorConfig {
        body_template: "Generated: ${date}".to_string(),
        ..OrchestratorConfig::default()
    };
    let github = TestGitHub::new();
    let orchestrator = ReleaseOrchestrator::new(config, &github);

    orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(1, 0, 0),
            "- feat: something [ab12cd34ef5678901234abcdef12345678901234]",
            "main",
            "sha-date-001",
            "corr-date-001",
        )
        .await
        .expect("orchestrate should succeed");

    let prs = github.created_prs().await;
    let body = prs[0].body.as_deref().unwrap_or("");
    // The date must follow ISO 8601 full format: YYYY-MM-DDTHH:MM:SSZ
    let iso8601_pattern =
        regex::Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z").expect("valid regex");
    assert!(
        iso8601_pattern.is_match(body),
        "body must contain a full ISO 8601 timestamp; body:\n{body}"
    );
}

/// `${commit_count}` in the body template is replaced with the count of
/// changelog entries (lines starting with `- `).
#[tokio::test]
async fn test_body_template_commit_count_variable_is_substituted() {
    let changelog = "- feat: first [aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1]\n\
                     - fix: second [aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa2]\n\
                     - chore: third [aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa3]";
    let config = OrchestratorConfig {
        body_template: "Count: ${commit_count}".to_string(),
        ..OrchestratorConfig::default()
    };
    let github = TestGitHub::new();
    let orchestrator = ReleaseOrchestrator::new(config, &github);

    orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(1, 0, 0),
            changelog,
            "main",
            "sha-cc-001",
            "corr-cc-001",
        )
        .await
        .expect("orchestrate should succeed");

    let prs = github.created_prs().await;
    let body = prs[0].body.as_deref().unwrap_or("");
    assert!(
        body.contains("Count: 3"),
        "body must contain commit count of 3; body:\n{body}"
    );
}

/// `${correlation_id}` in the body template is replaced with the tracing ID
/// passed to `orchestrate`.
#[tokio::test]
async fn test_body_template_correlation_id_variable_is_substituted() {
    let config = OrchestratorConfig {
        body_template: "Trace: ${correlation_id}".to_string(),
        ..OrchestratorConfig::default()
    };
    let github = TestGitHub::new();
    let orchestrator = ReleaseOrchestrator::new(config, &github);

    orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(1, 0, 0),
            "- feat: something [ab12cd34ef5678901234abcdef12345678901234]",
            "main",
            "sha-cid-001",
            "trace-abc-123",
        )
        .await
        .expect("orchestrate should succeed");

    let prs = github.created_prs().await;
    let body = prs[0].body.as_deref().unwrap_or("");
    assert!(
        body.contains("Trace: trace-abc-123"),
        "body must contain correlation_id; body:\n{body}"
    );
}

/// `${repository}` in the body template is replaced with `"owner/repo"`.
#[tokio::test]
async fn test_body_template_repository_variable_is_substituted() {
    let config = OrchestratorConfig {
        body_template: "Repo: ${repository}".to_string(),
        ..OrchestratorConfig::default()
    };
    let github = TestGitHub::new();
    let orchestrator = ReleaseOrchestrator::new(config, &github);

    orchestrator
        .orchestrate(
            "myorg",
            "myrepo",
            &ver(1, 0, 0),
            "- feat: something [ab12cd34ef5678901234abcdef12345678901234]",
            "main",
            "sha-repo-001",
            "corr-repo-001",
        )
        .await
        .expect("orchestrate should succeed");

    let prs = github.created_prs().await;
    let body = prs[0].body.as_deref().unwrap_or("");
    assert!(
        body.contains("Repo: myorg/myrepo"),
        "body must contain owner/repo format; body:\n{body}"
    );
}

/// `${branch}` in the body template is replaced with the base branch name.
#[tokio::test]
async fn test_body_template_branch_variable_is_substituted() {
    let config = OrchestratorConfig {
        body_template: "Branch: ${branch}".to_string(),
        ..OrchestratorConfig::default()
    };
    let github = TestGitHub::new();
    let orchestrator = ReleaseOrchestrator::new(config, &github);

    orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(1, 0, 0),
            "- feat: something [ab12cd34ef5678901234abcdef12345678901234]",
            "develop",
            "sha-branch-001",
            "corr-branch-001",
        )
        .await
        .expect("orchestrate should succeed");

    let prs = github.created_prs().await;
    let body = prs[0].body.as_deref().unwrap_or("");
    assert!(
        body.contains("Branch: develop"),
        "body must contain branch name; body:\n{body}"
    );
}

/// `${previous_version}` defaults to `"initial release"` when there is no
/// prior release PR (create path).
#[tokio::test]
async fn test_body_template_previous_version_defaults_to_initial_release() {
    let config = OrchestratorConfig {
        body_template: "Previous: ${previous_version}".to_string(),
        ..OrchestratorConfig::default()
    };
    let github = TestGitHub::new();
    let orchestrator = ReleaseOrchestrator::new(config, &github);

    orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(1, 0, 0),
            "- feat: first [ab12cd34ef5678901234abcdef12345678901234]",
            "main",
            "sha-pv-none-001",
            "corr-pv-none-001",
        )
        .await
        .expect("orchestrate should succeed");

    let prs = github.created_prs().await;
    let body = prs[0].body.as_deref().unwrap_or("");
    assert!(
        body.contains("Previous: initial release"),
        "body must show 'initial release' when no prior version; body:\n{body}"
    );
}

/// `${previous_version}` is populated with the old PR's version when an
/// existing lower-version PR is renamed (rename path).
#[tokio::test]
async fn test_body_template_previous_version_set_when_renaming_existing_pr() {
    let config = OrchestratorConfig {
        body_template: "Previous: ${previous_version}".to_string(),
        ..OrchestratorConfig::default()
    };

    // Existing PR at v1.0.0 — lower than the incoming v2.0.0.
    let old_pr = make_open_release_pr(10, "release/v1.0.0", None);

    let github = TestGitHub::new()
        .with_search_results(vec![old_pr.clone()])
        .await;

    let orchestrator = ReleaseOrchestrator::new(config, &github);

    let result = orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(2, 0, 0),
            "- feat: big feature [ab12cd34ef5678901234abcdef12345678901234]",
            "main",
            "sha-pv-rename-001",
            "corr-pv-rename-001",
        )
        .await
        .expect("orchestrate should succeed");

    assert!(
        matches!(result, OrchestratorResult::Renamed { .. }),
        "expected Renamed result; got {result:?}"
    );

    let prs = github.created_prs().await;
    assert_eq!(prs.len(), 1, "one new PR should be created for v2.0.0");
    let body = prs[0].body.as_deref().unwrap_or("");
    assert!(
        body.contains("Previous: 1.0.0"),
        "body must show the old version as previous_version; body:\n{body}"
    );
}

/// Legacy `{variable}` syntax (without the `$` prefix) in the body template is
/// expanded for backward compatibility, matching the behaviour of `render_title`.
#[tokio::test]
async fn test_body_template_legacy_brace_syntax_is_substituted() {
    let config = OrchestratorConfig {
        body_template: "Tag: {version_tag} | Repo: {repository}".to_string(),
        ..OrchestratorConfig::default()
    };
    let github = TestGitHub::new();
    let orchestrator = ReleaseOrchestrator::new(config, &github);

    orchestrator
        .orchestrate(
            "acme",
            "widget",
            &ver(3, 0, 0),
            "- feat: something [ab12cd34ef5678901234abcdef12345678901234]",
            "main",
            "sha-legacy-001",
            "corr-legacy-001",
        )
        .await
        .expect("orchestrate should succeed");

    let prs = github.created_prs().await;
    let body = prs[0].body.as_deref().unwrap_or("");
    assert!(
        body.contains("Tag: v3.0.0"),
        "legacy {{version_tag}} must be substituted; body:\n{body}"
    );
    assert!(
        body.contains("Repo: acme/widget"),
        "legacy {{repository}} must be substituted; body:\n{body}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// render_body sentinel — round-trip tests
// ─────────────────────────────────────────────────────────────────────────────

/// `render_body` appends an HTML sentinel comment so the update path can
/// recover the original `previous_version` without parsing the template output.
#[tokio::test]
async fn test_render_body_appends_previous_version_sentinel() {
    let config = OrchestratorConfig {
        body_template: "## Release ${version}\n\n${changelog}".to_string(),
        ..OrchestratorConfig::default()
    };
    let github = TestGitHub::new();
    let orchestrator = ReleaseOrchestrator::new(config, &github);

    orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(2, 0, 0),
            "- feat: thing [ab12cd34ef5678901234abcdef12345678901234]",
            "main",
            "sha-sentinel-001",
            "corr-sentinel-001",
        )
        .await
        .expect("orchestrate should succeed");

    let prs = github.created_prs().await;
    let body = prs[0].body.as_deref().unwrap_or("");
    // The sentinel must be present.
    assert!(
        body.contains("<!-- release-regent: previous-version=initial release -->"),
        "sentinel must be appended on the create path; body:\n{body}"
    );
    // The visible template content must still be present.
    assert!(
        body.contains("## Release 2.0.0"),
        "template content missing; body:\n{body}"
    );
}

/// `extract_previous_version_sentinel` returns `Some` when the sentinel is present.
#[test]
fn test_extract_previous_version_sentinel_returns_value_when_present() {
    let body = "## Changelog\n\n- fix: thing\n<!-- release-regent: previous-version=1.2.3 -->";
    assert_eq!(
        super::extract_previous_version_sentinel(body),
        Some("1.2.3".to_string())
    );
}

/// `extract_previous_version_sentinel` returns `None` when the sentinel is absent.
#[test]
fn test_extract_previous_version_sentinel_returns_none_when_absent() {
    assert_eq!(
        super::extract_previous_version_sentinel("no sentinel here"),
        None
    );
    assert_eq!(super::extract_previous_version_sentinel(""), None);
}

/// On the update (equal-version) path, `${previous_version}` in the re-rendered
/// body must preserve the value from the existing PR body sentinel rather than
/// resetting to `"initial release"`.
#[tokio::test]
async fn test_update_release_pr_preserves_previous_version_from_sentinel() {
    // The existing PR was created via a rename v1.0.0 → v2.0.0, so its body
    // contains the sentinel with "1.0.0".
    let existing_body = concat!(
        "Previous: 1.0.0\n## Changelog\n\n",
        "- feat: first [aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1]\n",
        "<!-- release-regent: previous-version=1.0.0 -->",
    );
    let config = OrchestratorConfig {
        body_template: "Previous: ${previous_version}\n## Changelog\n\n${changelog}".to_string(),
        ..OrchestratorConfig::default()
    };

    let existing_pr = make_open_release_pr(20, "release/v2.0.0", Some(existing_body));
    let github = TestGitHub::new()
        .with_search_results(vec![existing_pr.clone()])
        .await
        .with_pr_by_number(existing_pr)
        .await;

    let orchestrator = ReleaseOrchestrator::new(config, &github);

    // Incoming version is the same (v2.0.0) → equal-version update path.
    orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(2, 0, 0),
            "- feat: second [aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa2]",
            "main",
            "sha-pv-update-001",
            "corr-pv-update-001",
        )
        .await
        .expect("orchestrate should succeed");

    let updates = github.updated_prs().await;
    assert_eq!(updates.len(), 1);
    let new_body = updates[0].2.as_deref().unwrap_or("");
    assert!(
        new_body.contains("Previous: 1.0.0"),
        "previous_version must be preserved from sentinel on update path; body:\n{new_body}"
    );
    assert!(
        !new_body.contains("Previous: initial release"),
        "must not regress to 'initial release' on update path; body:\n{new_body}"
    );
}

/// Two successive equal-version updates must not accumulate sentinel lines.
///
/// Regression guard for the bug where the sentinel appended by `render_body`
/// leaks into the extracted changelog section (when `${changelog}` is the last
/// section), survives `merge_changelog_sections` verbatim, and is then re-passed
/// as `ctx.changelog` so that each update appends one more sentinel.
#[tokio::test]
async fn test_successive_updates_do_not_accumulate_sentinels() {
    // Build a PR body that already went through one render_body call:
    // it contains the sentinel once (as produced by the previous update).
    let sha1 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1";
    let existing_body = format!(
        concat!(
            "## Changelog\n\n",
            "- feat: first [{sha1}]\n",
            "<!-- release-regent: previous-version=initial release -->"
        ),
        sha1 = sha1
    );

    let existing_pr = make_open_release_pr(30, "release/v1.0.0", Some(&existing_body));
    let github = TestGitHub::new()
        .with_search_results(vec![existing_pr.clone()])
        .await
        .with_pr_by_number(existing_pr)
        .await;

    let sha2 = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb2";
    let orchestrator = ReleaseOrchestrator::new(default_config(), &github);

    // Second update — same version, adds a new commit.
    orchestrator
        .orchestrate(
            "testorg",
            "testrepo",
            &ver(1, 0, 0),
            &format!("- feat: second [{sha2}]"),
            "main",
            "sha-accum-001",
            "corr-accum-001",
        )
        .await
        .expect("orchestrate should succeed");

    let updates = github.updated_prs().await;
    assert_eq!(updates.len(), 1, "expected exactly one PR update");
    let new_body = updates[0].2.as_deref().unwrap_or("");

    // The sentinel must appear exactly once.
    let sentinel = "<!-- release-regent: previous-version=";
    assert_eq!(
        new_body.matches(sentinel).count(),
        1,
        "sentinel must appear exactly once after two updates; body:\n{new_body}"
    );
    // Both changelog entries must be present.
    assert!(
        new_body.contains("first"),
        "first entry must be preserved; body:\n{new_body}"
    );
    assert!(
        new_body.contains("second"),
        "second entry must be present; body:\n{new_body}"
    );
}
