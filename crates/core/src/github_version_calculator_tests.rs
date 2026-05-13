//! Tests for [`GitHubVersionCalculator`].
//!
//! These tests exercise `analyze_commits` directly using an inline test double
//! for [`GitHubOperations`] so that the concurrent fan-out behaviour can be
//! verified without a live GitHub API.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::Mutex;

use crate::{
    github_version_calculator::GitHubVersionCalculator,
    traits::{
        git_operations::{
            GetCommitsOptions, GitCommit, GitRepository, GitTag, GitUser, ListTagsOptions,
        },
        github_operations::{
            CreatePullRequestParams, CreateReleaseParams, GitHubOperations, GitUser as GHGitUser,
            Label, PullRequest, Release, Repository, Tag, UpdateReleaseParams,
        },
        version_calculator::{VersionCalculator, VersionContext, VersioningStrategy},
    },
    CoreError, CoreResult,
};

// ─────────────────────────────────────────────────────────────────────────────
// Test double
// ─────────────────────────────────────────────────────────────────────────────

/// A minimal `GitHubOperations` stub that serves a pre-loaded commit map and
/// records how many times `get_commit` has been called.
#[derive(Clone)]
struct StubGitHub {
    /// Commits keyed by SHA; a missing key produces `CoreError::not_found`.
    commits: HashMap<String, GitCommit>,
    /// Counter incremented on every `get_commit` call.
    get_commit_call_count: Arc<Mutex<usize>>,
}

impl StubGitHub {
    fn new(commits: Vec<GitCommit>) -> Self {
        let map = commits.into_iter().map(|c| (c.sha.clone(), c)).collect();
        Self {
            commits: map,
            get_commit_call_count: Arc::new(Mutex::new(0)),
        }
    }

    async fn get_commit_call_count(&self) -> usize {
        *self.get_commit_call_count.lock().await
    }
}

fn make_commit(sha: &str, message: &str) -> GitCommit {
    GitCommit {
        sha: sha.to_string(),
        author: GitUser {
            name: "Dev".to_string(),
            email: "dev@example.com".to_string(),
            login: None,
        },
        committer: GitUser {
            name: "Dev".to_string(),
            email: "dev@example.com".to_string(),
            login: None,
        },
        author_date: Utc::now(),
        commit_date: Utc::now(),
        message: message.to_string(),
        subject: message.to_string(),
        body: None,
        parents: vec![],
        files: vec![],
    }
}

#[async_trait]
impl crate::traits::git_operations::GitOperations for StubGitHub {
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

    async fn get_commit(
        &self,
        _owner: &str,
        _repo: &str,
        commit_sha: &str,
    ) -> CoreResult<GitCommit> {
        *self.get_commit_call_count.lock().await += 1;
        self.commits
            .get(commit_sha)
            .cloned()
            .ok_or_else(|| CoreError::not_found(format!("commit {commit_sha}")))
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
        Err(CoreError::not_found("head commit"))
    }

    async fn get_repository_info(&self, owner: &str, repo: &str) -> CoreResult<GitRepository> {
        Ok(GitRepository {
            name: repo.to_string(),
            owner: owner.to_string(),
            full_name: format!("{owner}/{repo}"),
            default_branch: "main".to_string(),
            clone_url: format!("https://github.com/{owner}/{repo}"),
            ssh_url: format!("git@github.com:{owner}/{repo}.git"),
            private: false,
            description: None,
        })
    }
}

fn stub_repo() -> Repository {
    Repository {
        id: 1,
        name: "repo".to_string(),
        full_name: "owner/repo".to_string(),
        owner: "owner".to_string(),
        description: None,
        private: false,
        default_branch: "main".to_string(),
        clone_url: "https://github.com/owner/repo.git".to_string(),
        ssh_url: "git@github.com:owner/repo.git".to_string(),
        homepage: None,
    }
}

fn stub_git_user() -> GHGitUser {
    GHGitUser {
        name: "bot".to_string(),
        email: "bot@example.com".to_string(),
        login: Some("bot".to_string()),
    }
}

#[async_trait]
impl GitHubOperations for StubGitHub {
    async fn create_pull_request(
        &self,
        _owner: &str,
        _repo: &str,
        _params: CreatePullRequestParams,
    ) -> CoreResult<PullRequest> {
        Err(CoreError::not_found("not implemented"))
    }

    async fn create_release(
        &self,
        _owner: &str,
        _repo: &str,
        _params: CreateReleaseParams,
    ) -> CoreResult<Release> {
        Err(CoreError::not_found("not implemented"))
    }

    async fn create_tag(
        &self,
        _owner: &str,
        _repo: &str,
        tag_name: &str,
        commit_sha: &str,
        message: Option<String>,
        tagger: Option<GHGitUser>,
    ) -> CoreResult<Tag> {
        Ok(Tag {
            name: tag_name.to_string(),
            commit_sha: commit_sha.to_string(),
            message,
            tagger,
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
        _number: u64,
    ) -> CoreResult<PullRequest> {
        Err(CoreError::not_found("PR"))
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
        Ok(vec![])
    }

    async fn search_pull_requests(
        &self,
        _owner: &str,
        _repo: &str,
        _query: &str,
    ) -> CoreResult<Vec<PullRequest>> {
        Ok(vec![])
    }

    async fn update_pull_request(
        &self,
        _owner: &str,
        _repo: &str,
        _number: u64,
        _title: Option<String>,
        _body: Option<String>,
        _state: Option<String>,
    ) -> CoreResult<PullRequest> {
        Err(CoreError::not_found("PR"))
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
        _branch_name: &str,
        _sha: &str,
    ) -> CoreResult<()> {
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
    ) -> CoreResult<Vec<Label>> {
        Ok(vec![])
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

    async fn batch_commit_files_rebased(
        &self,
        _owner: &str,
        _repo: &str,
        _branch: &str,
        _files: &[crate::traits::github_operations::FileUpdate],
        _message: &str,
        _parent_sha: &str,
    ) -> CoreResult<()> {
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
        self.clone()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn make_context() -> VersionContext {
    VersionContext {
        base_ref: None,
        current_version: None,
        head_ref: "main".to_string(),
        owner: "owner".to_string(),
        repo: "repo".to_string(),
        target_branch: "main".to_string(),
    }
}

fn conventional_strategy() -> VersioningStrategy {
    VersioningStrategy::ConventionalCommits {
        custom_types: HashMap::new(),
        include_prerelease: false,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// analyze_commits tests
// ─────────────────────────────────────────────────────────────────────────────

/// An empty SHA list produces an empty analysis result without calling the API.
#[tokio::test]
async fn test_analyze_commits_with_empty_list_returns_empty() {
    let stub = StubGitHub::new(vec![]);
    let calc = GitHubVersionCalculator::new(stub.clone());

    let result = calc
        .analyze_commits(make_context(), conventional_strategy(), vec![])
        .await
        .unwrap();

    assert!(result.is_empty());
    assert_eq!(stub.get_commit_call_count().await, 0);
}

/// A single SHA is fetched and its conventional-commit message is analysed.
#[tokio::test]
async fn test_analyze_commits_single_sha_returns_one_analysis() {
    let sha = "abc1234".to_string();
    let stub = StubGitHub::new(vec![make_commit(&sha, "feat: add login endpoint")]);
    let calc = GitHubVersionCalculator::new(stub.clone());

    let result = calc
        .analyze_commits(make_context(), conventional_strategy(), vec![sha.clone()])
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].sha, sha);
    assert_eq!(result[0].commit_type, Some("feat".to_string()));
    assert!(!result[0].is_breaking);
    assert_eq!(stub.get_commit_call_count().await, 1);
}

/// Multiple SHAs are all fetched and each produces a separate analysis entry.
#[tokio::test]
async fn test_analyze_commits_multiple_shas_returns_all_analyses() {
    let commits = vec![
        make_commit("sha1", "feat: add feature"),
        make_commit("sha2", "fix: patch bug"),
        make_commit("sha3", "chore: update deps"),
    ];
    let shas: Vec<String> = commits.iter().map(|c| c.sha.clone()).collect();
    let stub = StubGitHub::new(commits);
    let calc = GitHubVersionCalculator::new(stub.clone());

    let result = calc
        .analyze_commits(make_context(), conventional_strategy(), shas)
        .await
        .unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(stub.get_commit_call_count().await, 3);

    let types: Vec<Option<String>> = result.iter().map(|a| a.commit_type.clone()).collect();
    assert!(types.contains(&Some("feat".to_string())));
    assert!(types.contains(&Some("fix".to_string())));
    assert!(types.contains(&Some("chore".to_string())));
}

/// A breaking-change commit is flagged with `is_breaking = true` and
/// `version_bump = Major`.
#[tokio::test]
async fn test_analyze_commits_breaking_change_is_flagged() {
    let sha = "break1".to_string();
    let stub = StubGitHub::new(vec![make_commit(&sha, "feat!: remove legacy API")]);
    let calc = GitHubVersionCalculator::new(stub);

    let result = calc
        .analyze_commits(make_context(), conventional_strategy(), vec![sha])
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert!(result[0].is_breaking);

    use crate::traits::version_calculator::VersionBump;
    assert_eq!(result[0].version_bump, VersionBump::Major);
}

/// When one SHA is not found the whole call returns an error rather than
/// silently dropping the missing commit.
#[tokio::test]
async fn test_analyze_commits_missing_sha_returns_error() {
    let stub = StubGitHub::new(vec![make_commit("exists", "fix: known commit")]);
    let calc = GitHubVersionCalculator::new(stub);

    let err = calc
        .analyze_commits(
            make_context(),
            conventional_strategy(),
            vec!["exists".to_string(), "missing-sha".to_string()],
        )
        .await
        .unwrap_err();

    let msg = err.to_string();
    assert!(
        msg.contains("missing-sha"),
        "error should identify the missing SHA; got: {msg}"
    );
}

/// `get_commit` is called exactly once per SHA — not more, not less.
#[tokio::test]
async fn test_analyze_commits_calls_api_once_per_sha() {
    let n = 10_usize;
    let commits: Vec<GitCommit> = (0..n)
        .map(|i| make_commit(&format!("sha{i:03}"), &format!("fix: patch {i}")))
        .collect();
    let shas: Vec<String> = commits.iter().map(|c| c.sha.clone()).collect();
    let stub = StubGitHub::new(commits);
    let calc = GitHubVersionCalculator::new(stub.clone());

    let result = calc
        .analyze_commits(make_context(), conventional_strategy(), shas)
        .await
        .unwrap();

    assert_eq!(result.len(), n);
    assert_eq!(stub.get_commit_call_count().await, n);
}
