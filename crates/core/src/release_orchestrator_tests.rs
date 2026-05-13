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
        self.state.lock().await.rebased_batch_commits.push((
            branch.to_string(),
            paths,
            message.to_string(),
            parent_sha.to_string(),
        ));
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
