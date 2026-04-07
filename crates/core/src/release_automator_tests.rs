use super::*;
use crate::{
    traits::{
        event_source::{EventSourceKind, EventType, ProcessingEvent, RepositoryInfo},
        git_operations::{
            GetCommitsOptions, GitCommit, GitOperations, GitRepository, GitTag, ListTagsOptions,
        },
        github_operations::{
            CollaboratorPermission, CreatePullRequestParams, CreateReleaseParams, GitHubOperations,
            GitUser as GitHubUser, Label, PullRequest, PullRequestBranch, Release, Repository, Tag,
            UpdateReleaseParams,
        },
    },
    CoreError, CoreResult,
};
use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::Mutex;

// ─────────────────────────────────────────────────────────────────────────────
// Inline test double
// ─────────────────────────────────────────────────────────────────────────────

/// Configurable mock state for `TestGitHub`.
#[derive(Default)]
struct TestState {
    /// Tags returned by successful `create_tag`.
    created_tags: Vec<(String, String)>,
    /// Whether the next `create_tag` call should return `NotSupported` (tag exists).
    next_create_tag_conflict: bool,
    /// Whether `create_tag` always errors with a GitHub error.
    create_tag_github_error: bool,
    /// Whether `create_release` should return an error.
    create_release_error: bool,
    /// Whether `delete_branch` should return an error.
    delete_branch_error: bool,
    /// Releases stored (keyed by tag name) — returned by `get_release_by_tag`.
    releases_by_tag: std::collections::HashMap<String, Release>,
    /// Recorded `create_release` calls.
    created_releases: Vec<CreateReleaseParams>,
    /// Recorded `delete_branch` calls.
    deleted_branches: Vec<String>,
    /// Sequential release ID counter.
    next_release_id: u64,
}

#[derive(Clone)]
struct TestGitHub {
    state: Arc<Mutex<TestState>>,
}

impl TestGitHub {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(TestState {
                next_release_id: 1,
                ..Default::default()
            })),
        }
    }

    /// Make the next `create_tag` fail with `NotSupported` (tag already exists).
    async fn with_tag_conflict(self) -> Self {
        self.state.lock().await.next_create_tag_conflict = true;
        self
    }

    /// Make `create_tag` always fail with a generic `GitHub` error.
    async fn with_create_tag_github_error(self) -> Self {
        self.state.lock().await.create_tag_github_error = true;
        self
    }

    /// Make `create_release` fail.
    async fn with_create_release_error(self) -> Self {
        self.state.lock().await.create_release_error = true;
        self
    }

    /// Make `delete_branch` fail (should be non-fatal).
    async fn with_delete_branch_error(self) -> Self {
        self.state.lock().await.delete_branch_error = true;
        self
    }

    /// Pre-load a release returned by `get_release_by_tag` for `tag_name`.
    async fn with_release_for_tag(self, tag_name: impl Into<String>, release: Release) -> Self {
        self.state
            .lock()
            .await
            .releases_by_tag
            .insert(tag_name.into(), release);
        self
    }

    async fn created_tags(&self) -> Vec<(String, String)> {
        self.state.lock().await.created_tags.clone()
    }

    async fn created_releases(&self) -> Vec<CreateReleaseParams> {
        self.state.lock().await.created_releases.clone()
    }

    async fn deleted_branches(&self) -> Vec<String> {
        self.state.lock().await.deleted_branches.clone()
    }
}

// ── GitOperations stub ────────────────────────────────────────────────────

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

// ── GitHubOperations impl ─────────────────────────────────────────────────

#[async_trait]
impl GitHubOperations for TestGitHub {
    async fn add_labels(
        &self,
        _owner: &str,
        _repo: &str,
        _issue_number: u64,
        _labels: &[&str],
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
        _issue_number: u64,
        _body: &str,
    ) -> CoreResult<()> {
        Ok(())
    }

    async fn create_pull_request(
        &self,
        owner: &str,
        repo: &str,
        params: CreatePullRequestParams,
    ) -> CoreResult<PullRequest> {
        let now = Utc::now();
        let r = stub_repo(owner, repo);
        Ok(PullRequest {
            number: 1,
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
                repo: r.clone(),
            },
            base: PullRequestBranch {
                ref_name: params.base,
                sha: "def".to_string(),
                repo: r,
            },
        })
    }

    async fn create_release(
        &self,
        _owner: &str,
        _repo: &str,
        params: CreateReleaseParams,
    ) -> CoreResult<Release> {
        let mut st = self.state.lock().await;
        if st.create_release_error {
            return Err(CoreError::network("simulated release creation failure"));
        }
        let id = st.next_release_id;
        st.next_release_id += 1;
        st.created_releases.push(params.clone());
        let now = Utc::now();
        Ok(Release {
            id,
            tag_name: params.tag_name.clone(),
            name: params.name,
            body: params.body,
            draft: params.draft,
            prerelease: params.prerelease,
            created_at: now,
            published_at: Some(now),
            author: stub_git_user(),
            target_commitish: params
                .target_commitish
                .unwrap_or_else(|| params.tag_name.clone()),
        })
    }

    async fn create_tag(
        &self,
        _owner: &str,
        _repo: &str,
        tag_name: &str,
        commit_sha: &str,
        _message: Option<String>,
        _tagger: Option<GitHubUser>,
    ) -> CoreResult<Tag> {
        let mut st = self.state.lock().await;
        if st.create_tag_github_error {
            return Err(CoreError::network("simulated tag creation failure"));
        }
        if st.next_create_tag_conflict {
            st.next_create_tag_conflict = false;
            return Err(CoreError::not_supported(
                "create_tag",
                format!("tag '{tag_name}' already exists"),
            ));
        }
        st.created_tags
            .push((tag_name.to_string(), commit_sha.to_string()));
        let now = Utc::now();
        Ok(Tag {
            name: tag_name.to_string(),
            commit_sha: commit_sha.to_string(),
            message: None,
            tagger: None,
            created_at: Some(now),
        })
    }

    async fn delete_branch(&self, _owner: &str, _repo: &str, branch_name: &str) -> CoreResult<()> {
        let mut st = self.state.lock().await;
        if st.delete_branch_error {
            return Err(CoreError::network("simulated branch deletion failure"));
        }
        st.deleted_branches.push(branch_name.to_string());
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

    async fn get_release_by_tag(
        &self,
        _owner: &str,
        _repo: &str,
        tag: &str,
    ) -> CoreResult<Release> {
        let st = self.state.lock().await;
        st.releases_by_tag
            .get(tag)
            .cloned()
            .ok_or_else(|| CoreError::not_found(format!("release for tag '{tag}'")))
    }

    async fn list_pr_labels(
        &self,
        _owner: &str,
        _repo: &str,
        _issue_number: u64,
    ) -> CoreResult<Vec<Label>> {
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
            user: stub_git_user(),
            head: PullRequestBranch {
                ref_name: "release/v1.0.0".to_string(),
                sha: "abc".to_string(),
                repo: r.clone(),
            },
            base: PullRequestBranch {
                ref_name: "main".to_string(),
                sha: "def".to_string(),
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
}

// ─────────────────────────────────────────────────────────────────────────────
// Test helpers
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

fn stub_git_user() -> GitHubUser {
    GitHubUser {
        name: "test-user".to_string(),
        email: "test@example.com".to_string(),
        login: Some("test-user".to_string()),
    }
}

fn stub_release(id: u64, tag_name: &str, prerelease: bool) -> Release {
    let now = Utc::now();
    Release {
        id,
        tag_name: tag_name.to_string(),
        name: Some(tag_name.to_string()),
        body: Some(String::new()),
        draft: false,
        prerelease,
        created_at: now,
        published_at: Some(now),
        author: stub_git_user(),
        target_commitish: tag_name.to_string(),
    }
}

/// Build a minimal [`ProcessingEvent`] representing a merged release PR.
///
/// `branch` is the head branch of the merged PR (e.g. `"release/v1.2.3"`).
/// `merge_sha` is the merge commit SHA.
/// `body` is the PR body content (may include `## Changelog` section).
fn make_release_pr_event(branch: &str, merge_sha: &str, body: &str) -> ProcessingEvent {
    let payload = serde_json::json!({
        "pull_request": {
            "number": 42,
            "head": {
                "ref": branch,
                "sha": merge_sha
            },
            "merge_commit_sha": merge_sha,
            "body": body
        }
    });
    ProcessingEvent {
        event_id: "evt-001".to_string(),
        correlation_id: "corr-001".to_string(),
        event_type: EventType::ReleasePrMerged,
        repository: RepositoryInfo {
            owner: "testorg".to_string(),
            name: "testrepo".to_string(),
            default_branch: "main".to_string(),
        },
        payload,
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// extract_version_from_branch tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_extract_version_from_branch_valid_stable() {
    let v = extract_version_from_branch("release/v1.2.3", "release").unwrap();
    assert_eq!(v.major, 1);
    assert_eq!(v.minor, 2);
    assert_eq!(v.patch, 3);
    assert!(v.prerelease.is_none());
}

#[test]
fn test_extract_version_from_branch_prerelease() {
    let v = extract_version_from_branch("release/v1.0.0-rc.1", "release").unwrap();
    assert_eq!(v.major, 1);
    assert_eq!(v.minor, 0);
    assert_eq!(v.patch, 0);
    assert_eq!(v.prerelease.as_deref(), Some("rc.1"));
    assert!(v.is_prerelease());
}

#[test]
fn test_extract_version_from_branch_wrong_prefix_returns_error() {
    let err = extract_version_from_branch("feature/my-feature", "release").unwrap_err();
    assert!(matches!(err, CoreError::InvalidInput { .. }));
}

#[test]
fn test_extract_version_from_branch_missing_v_prefix_returns_error() {
    let err = extract_version_from_branch("release/1.2.3", "release").unwrap_err();
    assert!(matches!(err, CoreError::InvalidInput { .. }));
}

#[test]
fn test_extract_version_from_branch_invalid_semver_returns_error() {
    let err = extract_version_from_branch("release/vnot-semver", "release").unwrap_err();
    assert!(matches!(
        err,
        CoreError::InvalidInput { .. } | CoreError::Versioning { .. }
    ));
}

// ─────────────────────────────────────────────────────────────────────────────
// is_release_pr_branch tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_is_release_pr_branch_matching() {
    assert!(is_release_pr_branch("release/v1.2.3", "release"));
    assert!(is_release_pr_branch("release/v0.1.0-alpha.1", "release"));
}

#[test]
fn test_is_release_pr_branch_non_matching() {
    assert!(!is_release_pr_branch("feature/my-feature", "release"));
    assert!(!is_release_pr_branch("fix/bug", "release"));
    assert!(!is_release_pr_branch("release/no-version", "release"));
    assert!(!is_release_pr_branch("main", "release"));
}

// ─────────────────────────────────────────────────────────────────────────────
// ReleaseAutomator::automate tests
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_automate_happy_path_creates_tag_release_and_deletes_branch() {
    let github = TestGitHub::new();
    let config = AutomatorConfig::default();
    let automator = ReleaseAutomator::new(config, &github);

    let event = make_release_pr_event(
        "release/v1.2.3",
        "deadbeef1234567890deadbeef1234567890abcd",
        "## Changelog\n\n- feat: add widget [abc123def456789012345678901234567890abcd]\n",
    );

    let result = automator
        .automate("testorg", "testrepo", &event, "corr-001")
        .await
        .unwrap();

    let AutomatorResult::Created { release } = result;
    assert_eq!(release.tag_name, "v1.2.3");
    assert!(!release.prerelease);

    let tags = github.created_tags().await;
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].0, "v1.2.3");
    assert_eq!(tags[0].1, "deadbeef1234567890deadbeef1234567890abcd");

    let releases = github.created_releases().await;
    assert_eq!(releases.len(), 1);
    assert_eq!(releases[0].tag_name, "v1.2.3");
    assert!(!releases[0].prerelease);

    let branches = github.deleted_branches().await;
    assert_eq!(branches, vec!["release/v1.2.3"]);
}

#[tokio::test]
async fn test_automate_prerelease_version_sets_prerelease_flag() {
    let github = TestGitHub::new();
    let automator = ReleaseAutomator::new(AutomatorConfig::default(), &github);

    let event = make_release_pr_event(
        "release/v1.0.0-rc.1",
        "deadbeef1234567890deadbeef1234567890abcd",
        "## Changelog\n\n- fix: prep rc [abc123def456789012345678901234567890abcd]\n",
    );

    let result = automator
        .automate("testorg", "testrepo", &event, "corr-001")
        .await
        .unwrap();

    let AutomatorResult::Created { release } = result;
    assert!(release.prerelease, "Expected prerelease flag to be set");
    assert_eq!(release.tag_name, "v1.0.0-rc.1");

    let releases = github.created_releases().await;
    assert_eq!(releases.len(), 1);
    assert!(releases[0].prerelease);
}

#[tokio::test]
async fn test_automate_stable_version_does_not_set_prerelease_flag() {
    let github = TestGitHub::new();
    let automator = ReleaseAutomator::new(AutomatorConfig::default(), &github);

    let event = make_release_pr_event(
        "release/v2.0.0",
        "deadbeef1234567890deadbeef1234567890abcd",
        "",
    );

    let result = automator
        .automate("testorg", "testrepo", &event, "corr-001")
        .await
        .unwrap();

    let AutomatorResult::Created { release } = result;
    assert!(
        !release.prerelease,
        "Stable version must not set prerelease"
    );
}

#[tokio::test]
async fn test_automate_idempotent_tag_and_release_both_exist() {
    let existing_release = stub_release(42, "v1.2.3", false);
    let github = TestGitHub::new()
        .with_tag_conflict()
        .await
        .with_release_for_tag("v1.2.3", existing_release.clone())
        .await;
    let automator = ReleaseAutomator::new(AutomatorConfig::default(), &github);

    let event = make_release_pr_event(
        "release/v1.2.3",
        "deadbeef1234567890deadbeef1234567890abcd",
        "",
    );

    let result = automator
        .automate("testorg", "testrepo", &event, "corr-001")
        .await
        .unwrap();

    let AutomatorResult::Created { release } = result;
    assert_eq!(
        release.id, 42,
        "Expected the pre-existing release to be returned"
    );

    // No new releases created and no branch deleted (already complete).
    assert!(
        github.created_releases().await.is_empty(),
        "No new release should be created when both tag and release exist"
    );
    assert!(
        github.deleted_branches().await.is_empty(),
        "Branch should not be deleted when idempotently returning existing release"
    );
}

#[tokio::test]
async fn test_automate_tag_exists_but_release_missing_resumes() {
    // Tag exists but release does not — automator should create the release.
    let github = TestGitHub::new().with_tag_conflict().await;
    let automator = ReleaseAutomator::new(AutomatorConfig::default(), &github);

    let event = make_release_pr_event(
        "release/v1.2.3",
        "deadbeef1234567890deadbeef1234567890abcd",
        "## Changelog\n\n- feat: resumed [abc123def456789012345678901234567890abcd]\n",
    );

    let result = automator
        .automate("testorg", "testrepo", &event, "corr-001")
        .await
        .unwrap();

    let AutomatorResult::Created { release } = result;
    assert_eq!(release.tag_name, "v1.2.3");

    let releases = github.created_releases().await;
    assert_eq!(
        releases.len(),
        1,
        "Release should be created from the existing tag"
    );

    // Branch should still be cleaned up.
    let branches = github.deleted_branches().await;
    assert_eq!(branches, vec!["release/v1.2.3"]);
}

#[tokio::test]
async fn test_automate_github_api_failure_on_tag_creation_propagates() {
    let github = TestGitHub::new().with_create_tag_github_error().await;
    let automator = ReleaseAutomator::new(AutomatorConfig::default(), &github);

    let event = make_release_pr_event(
        "release/v1.2.3",
        "deadbeef1234567890deadbeef1234567890abcd",
        "",
    );

    let err = automator
        .automate("testorg", "testrepo", &event, "corr-001")
        .await
        .unwrap_err();

    assert!(
        matches!(err, CoreError::Network { .. }),
        "Expected Network error from create_tag failure, got: {err:?}"
    );
}

#[tokio::test]
async fn test_automate_github_api_failure_on_release_creation_propagates() {
    let github = TestGitHub::new().with_create_release_error().await;
    let automator = ReleaseAutomator::new(AutomatorConfig::default(), &github);

    let event = make_release_pr_event(
        "release/v1.2.3",
        "deadbeef1234567890deadbeef1234567890abcd",
        "",
    );

    let err = automator
        .automate("testorg", "testrepo", &event, "corr-001")
        .await
        .unwrap_err();

    assert!(
        matches!(err, CoreError::Network { .. }),
        "Expected Network error from create_release failure, got: {err:?}"
    );
}

#[tokio::test]
async fn test_automate_branch_delete_failure_is_nonfatal() {
    let github = TestGitHub::new().with_delete_branch_error().await;
    let automator = ReleaseAutomator::new(AutomatorConfig::default(), &github);

    let event = make_release_pr_event(
        "release/v1.2.3",
        "deadbeef1234567890deadbeef1234567890abcd",
        "",
    );

    // Should succeed even though delete_branch fails.
    let result = automator
        .automate("testorg", "testrepo", &event, "corr-001")
        .await
        .unwrap();

    let AutomatorResult::Created { release } = result;
    assert_eq!(release.tag_name, "v1.2.3");

    // Release was still created despite branch deletion failure.
    assert_eq!(github.created_releases().await.len(), 1);
}

#[tokio::test]
async fn test_automate_missing_head_ref_returns_invalid_input() {
    let github = TestGitHub::new();
    let automator = ReleaseAutomator::new(AutomatorConfig::default(), &github);

    // Payload without pull_request.head.ref
    let event = ProcessingEvent {
        event_id: "evt-bad".to_string(),
        correlation_id: "corr-bad".to_string(),
        event_type: EventType::ReleasePrMerged,
        repository: RepositoryInfo {
            owner: "org".to_string(),
            name: "repo".to_string(),
            default_branch: "main".to_string(),
        },
        payload: serde_json::json!({ "pull_request": { "number": 1 } }),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
    };

    let err = automator
        .automate("org", "repo", &event, "corr-bad")
        .await
        .unwrap_err();
    assert!(
        matches!(err, CoreError::InvalidInput { .. }),
        "Expected InvalidInput for missing head.ref, got: {err:?}"
    );
}

#[tokio::test]
async fn test_automate_missing_merge_sha_returns_invalid_input() {
    let github = TestGitHub::new();
    let automator = ReleaseAutomator::new(AutomatorConfig::default(), &github);

    // Payload has head.ref but no merge_commit_sha or head.sha
    let event = ProcessingEvent {
        event_id: "evt-bad".to_string(),
        correlation_id: "corr-bad".to_string(),
        event_type: EventType::ReleasePrMerged,
        repository: RepositoryInfo {
            owner: "org".to_string(),
            name: "repo".to_string(),
            default_branch: "main".to_string(),
        },
        payload: serde_json::json!({
            "pull_request": {
                "head": { "ref": "release/v1.0.0" }
            }
        }),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
    };

    let err = automator
        .automate("org", "repo", &event, "corr-bad")
        .await
        .unwrap_err();
    assert!(
        matches!(err, CoreError::InvalidInput { .. }),
        "Expected InvalidInput for missing both SHAs, got: {err:?}"
    );
}

#[tokio::test]
async fn test_automate_invalid_branch_version_returns_error() {
    let github = TestGitHub::new();
    let automator = ReleaseAutomator::new(AutomatorConfig::default(), &github);

    // Branch starts with release/v but has invalid semver
    let event = make_release_pr_event(
        "release/vnot-valid",
        "deadbeef1234567890deadbeef1234567890abcd",
        "",
    );

    let err = automator
        .automate("testorg", "testrepo", &event, "corr-001")
        .await
        .unwrap_err();
    // The error comes from VersionCalculator::parse_version which returns Versioning or InvalidInput.
    assert!(
        matches!(
            err,
            CoreError::InvalidInput { .. } | CoreError::Versioning { .. }
        ),
        "Expected parse error for invalid semver branch, got: {err:?}"
    );
}

#[tokio::test]
async fn test_automate_changelog_extracted_from_pr_body() {
    let github = TestGitHub::new();
    let automator = ReleaseAutomator::new(AutomatorConfig::default(), &github);

    let body = "## Changelog\n\n- feat: add widget [abc123def456789012345678901234567890abcd]\n- fix: bug [bcd234ef0123456789012345678901234567890ef]\n\n## Notes\n\nSee wiki.";
    let event = make_release_pr_event(
        "release/v1.0.0",
        "deadbeef1234567890deadbeef1234567890abcd",
        body,
    );

    automator
        .automate("testorg", "testrepo", &event, "corr-001")
        .await
        .unwrap();

    let releases = github.created_releases().await;
    assert_eq!(releases.len(), 1);
    let release_body = releases[0].body.as_deref().unwrap_or("");
    assert!(
        release_body.contains("add widget"),
        "Release body should contain changelog entry"
    );
    assert!(
        !release_body.contains("See wiki"),
        "Release body should not contain content after changelog section"
    );
}

#[tokio::test]
async fn test_automate_fallback_sha_used_when_merge_commit_sha_absent() {
    let github = TestGitHub::new();
    let automator = ReleaseAutomator::new(AutomatorConfig::default(), &github);

    // Payload uses head.sha as fallback (no merge_commit_sha field).
    let event = ProcessingEvent {
        event_id: "evt-001".to_string(),
        correlation_id: "corr-001".to_string(),
        event_type: EventType::ReleasePrMerged,
        repository: RepositoryInfo {
            owner: "testorg".to_string(),
            name: "testrepo".to_string(),
            default_branch: "main".to_string(),
        },
        payload: serde_json::json!({
            "pull_request": {
                "head": {
                    "ref": "release/v1.5.0",
                    "sha": "cafebabe1234567890cafebabe1234567890abcd"
                },
                "body": ""
            }
        }),
        received_at: Utc::now(),
        source: EventSourceKind::Webhook,
    };

    automator
        .automate("testorg", "testrepo", &event, "corr-001")
        .await
        .unwrap();

    let tags = github.created_tags().await;
    assert_eq!(tags.len(), 1);
    assert_eq!(
        tags[0].1, "cafebabe1234567890cafebabe1234567890abcd",
        "Should use head.sha when merge_commit_sha is absent"
    );
}
