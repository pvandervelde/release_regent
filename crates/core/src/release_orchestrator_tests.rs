use super::*;
use crate::{
    traits::{
        git_operations::{
            GetCommitsOptions, GitCommit, GitOperations, GitRepository, GitTag, ListTagsOptions,
        },
        github_operations::{
            CreatePullRequestParams, CreateReleaseParams, GitHubOperations, GitUser as GitHubUser,
            PullRequest, PullRequestBranch, Release, Repository, Tag, UpdateReleaseParams,
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
    /// Recorded `upsert_file` calls: (path, branch).
    upserted_files: Vec<(String, String)>,
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

    async fn upserted_files(&self) -> Vec<(String, String)> {
        self.state.lock().await.upserted_files.clone()
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

    async fn get_installation_id_for_repo(
        &self,
        _owner: &str,
        _repo: &str,
    ) -> CoreResult<u64> {
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

    let upserted = github.upserted_files().await;
    assert_eq!(upserted.len(), 1, "CHANGELOG.md should be committed to the release branch");
    assert_eq!(upserted[0].0, "CHANGELOG.md");
    assert_eq!(upserted[0].1, "release/v1.2.3");
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

    // CHANGELOG.md should be committed to the existing branch.
    let upserted = github.upserted_files().await;
    assert_eq!(upserted.len(), 1, "CHANGELOG.md should be updated on the existing branch");
    assert_eq!(upserted[0].0, "CHANGELOG.md");
    assert_eq!(upserted[0].1, "release/v1.0.0");
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

    // CHANGELOG.md should be committed to the new release branch.
    let upserted = github.upserted_files().await;
    assert_eq!(upserted.len(), 1, "CHANGELOG.md should be committed to the new release branch");
    assert_eq!(upserted[0].0, "CHANGELOG.md");
    assert_eq!(upserted[0].1, "release/v1.1.0");
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
    assert!(github.upserted_files().await.is_empty(), "NoOp should not commit any files");
}

/// Branch name conflict on first attempt → retries with timestamped fallback.
#[tokio::test]
async fn test_orchestrate_branch_conflict_uses_timestamped_fallback() {
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
        .expect("orchestrate should succeed despite first branch conflict");

    // The result should still be Created.
    if let OrchestratorResult::Created { branch_name, .. } = &result {
        // The fallback branch should start with "release/v1.0.0-" (plus timestamp).
        assert!(
            branch_name.starts_with("release/v1.0.0-"),
            "fallback branch name unexpected: {branch_name}"
        );
    } else {
        panic!("expected Created, got {result:?}");
    }

    // Two branch creation attempts should have been recorded.
    let branches = github.created_branches().await;
    assert_eq!(branches.len(), 1, "only the successful attempt is tracked");
    assert!(
        branches[0].0.starts_with("release/v1.0.0-"),
        "expected timestamped branch, got {:?}",
        branches[0].0
    );

    // CHANGELOG.md should be committed to the fallback branch.
    let upserted = github.upserted_files().await;
    assert_eq!(upserted.len(), 1, "CHANGELOG.md should be committed to the fallback branch");
    assert_eq!(upserted[0].0, "CHANGELOG.md");
    assert!(upserted[0].1.starts_with("release/v1.0.0-"), "expected fallback branch, got {:?}", upserted[0].1);
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

    // CHANGELOG.md should be committed to the existing branch.
    let upserted = github.upserted_files().await;
    assert_eq!(upserted.len(), 1, "CHANGELOG.md should be updated on the existing branch");
    assert_eq!(upserted[0].0, "CHANGELOG.md");
    assert_eq!(upserted[0].1, "release/v1.0.0");
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

/// Fallback branch name starts with the canonical name and has a numeric suffix.
#[test]
fn test_make_fallback_branch_name_contains_timestamp() {
    let github = TestGitHub::new();
    let orchestrator = ReleaseOrchestrator::new(default_config(), &github);
    let fallback = orchestrator.make_fallback_branch_name(&ver(1, 0, 0));
    assert!(
        fallback.starts_with("release/v1.0.0-"),
        "fallback should start with canonical prefix: {fallback}"
    );
    let suffix = fallback.trim_start_matches("release/v1.0.0-");
    assert!(
        suffix.chars().all(|c| c.is_ascii_digit()),
        "suffix should be numeric timestamp: {suffix}"
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
