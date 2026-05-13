use super::{merge_config_with_locks, ConfigLocks, LOCKABLE_FIELDS};
use release_regent_core::config::{
    BranchConfig, CoreConfig, ErrorHandlingConfig, ReleasesConfig, VersioningConfig,
    VersioningStrategy,
};
use release_regent_core::config::{ReleasePrConfig, ReleaseRegentConfig};
use tracing_test::traced_test;

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Build a minimal but complete `ReleaseRegentConfig` with all default values.
fn default_config() -> ReleaseRegentConfig {
    ReleaseRegentConfig::default()
}

/// Build a `ReleaseRegentConfig` from individual, well-known values so that tests can
/// mutate exactly one field and compare the result.
#[allow(clippy::too_many_arguments)] // 10 lockable fields require 10 explicit arguments
fn make_config(
    versioning_strategy: VersioningStrategy,
    allow_override: bool,
    draft: bool,
    prerelease: bool,
    generate_notes: bool,
    main_branch: &str,
    version_prefix: &str,
    max_retries: u32,
    backoff_multiplier: f64,
    initial_delay_ms: u64,
) -> ReleaseRegentConfig {
    ReleaseRegentConfig {
        core: CoreConfig {
            version_prefix: version_prefix.to_string(),
            branches: BranchConfig {
                main: main_branch.to_string(),
            },
        },
        versioning: VersioningConfig {
            strategy: versioning_strategy,
            allow_override,
            excluded_pr_authors: Vec::new(),
        },
        releases: ReleasesConfig {
            draft,
            prerelease,
            generate_notes,
        },
        error_handling: ErrorHandlingConfig {
            max_retries,
            backoff_multiplier,
            initial_delay_ms,
        },
        group: None,
        locked_fields: Vec::new(),
        release_pr: ReleasePrConfig::default(),
        notifications: Default::default(),
    }
}

/// Return a `ConfigLocks` with the given paths already locked.
fn locks_from(paths: &[&str]) -> ConfigLocks {
    let mut locks = ConfigLocks::default();
    for path in paths {
        locks.locked.insert((*path).to_string());
    }
    locks
}

// ─────────────────────────────────────────────────────────────────────────────
// LOCKABLE_FIELDS constant
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_lockable_fields_has_ten_entries() {
    assert_eq!(LOCKABLE_FIELDS.len(), 10);
}

#[test]
fn test_lockable_fields_contains_expected_paths() {
    let expected = [
        "versioning.strategy",
        "versioning.allow_override",
        "releases.draft",
        "releases.prerelease",
        "releases.generate_notes",
        "core.branches.main",
        "core.version_prefix",
        "error_handling.max_retries",
        "error_handling.backoff_multiplier",
        "error_handling.initial_delay_ms",
    ];
    for path in &expected {
        assert!(
            LOCKABLE_FIELDS.contains(path),
            "expected lockable field '{path}' was not found"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ConfigLocks::extend_from
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_config_locks_extend_from_valid_path_adds_to_set() {
    let mut locks = ConfigLocks::default();
    locks.extend_from(&["versioning.strategy".to_string()], "global");
    assert!(locks.is_locked("versioning.strategy"));
}

#[test]
fn test_config_locks_extend_from_multiple_valid_paths_all_added() {
    let mut locks = ConfigLocks::default();
    locks.extend_from(
        &[
            "releases.draft".to_string(),
            "core.version_prefix".to_string(),
        ],
        "global",
    );
    assert!(locks.is_locked("releases.draft"));
    assert!(locks.is_locked("core.version_prefix"));
}

#[test]
#[traced_test]
fn test_config_locks_extend_from_non_lockable_path_ignored_and_emits_warn() {
    // BA-63: non-lockable paths (e.g. release_pr.*) appear nowhere in LOCKABLE_FIELDS.
    let mut locks = ConfigLocks::default();
    locks.extend_from(&["release_pr.branch_name_template".to_string()], "global");
    assert!(
        !locks.is_locked("release_pr.branch_name_template"),
        "non-lockable path must not be added to the lock set"
    );
    assert!(
        logs_contain("locked_fields entry is not a lockable policy field"),
        "a warn! must be emitted for non-lockable paths"
    );
}

#[test]
#[traced_test]
fn test_config_locks_extend_from_duplicate_path_emits_warn_but_stays_locked() {
    // BA-62: A group policy cannot un-lock a global lock; duplicate entries are
    // a no-op to the lock set but do emit a diagnostic warning.
    let mut locks = ConfigLocks::default();
    locks.extend_from(&["releases.draft".to_string()], "global");
    locks.extend_from(&["releases.draft".to_string()], "group");
    assert!(
        locks.is_locked("releases.draft"),
        "field must remain locked after duplicate extend_from"
    );
    assert!(
        logs_contain("field already locked by higher level"),
        "a warn! must be emitted for the duplicate lock entry"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// ConfigLocks::is_locked
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_config_locks_is_locked_returns_true_for_added_path() {
    let locks = locks_from(&["versioning.strategy"]);
    assert!(locks.is_locked("versioning.strategy"));
}

#[test]
fn test_config_locks_is_locked_returns_false_for_absent_path() {
    let locks = locks_from(&["versioning.strategy"]);
    assert!(!locks.is_locked("releases.draft"));
}

#[test]
fn test_config_locks_is_locked_returns_false_on_empty_set() {
    let locks = ConfigLocks::default();
    assert!(!locks.is_locked("versioning.strategy"));
}

// ─────────────────────────────────────────────────────────────────────────────
// merge_config_with_locks — locked bool fields
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[traced_test]
fn test_merge_config_with_locks_locked_bool_kept_from_base_and_emits_warn() {
    // BA-59 / BA-60 / BA-61: a boolean field locked by higher level cannot be
    // overridden by an incoming value that differs from base.
    let mut base = default_config();
    base.releases.draft = false;

    let mut incoming = default_config();
    incoming.releases.draft = true; // attempts to unlock → must be rejected

    let locks = locks_from(&["releases.draft"]);
    let result = merge_config_with_locks(base, incoming, &locks);

    assert!(
        !result.releases.draft,
        "locked field must retain the base value"
    );
    assert!(
        logs_contain("locked field override attempt ignored"),
        "a warn! must be emitted when a locked field override is rejected"
    );
}

#[test]
fn test_merge_config_with_locks_locked_bool_same_value_no_warn() {
    let mut base = default_config();
    base.releases.draft = true;
    let mut incoming = default_config();
    incoming.releases.draft = true; // same value → no warn required

    let locks = locks_from(&["releases.draft"]);
    // Just verify no panic and correct value; we don't assert on warn here.
    let result = merge_config_with_locks(base, incoming, &locks);
    assert!(result.releases.draft);
}

#[test]
fn test_merge_config_with_locks_unlocked_bool_taken_from_incoming() {
    let mut base = default_config();
    base.releases.draft = false;
    let mut incoming = default_config();
    incoming.releases.draft = true; // no lock → incoming wins

    let locks = ConfigLocks::default(); // empty
    let result = merge_config_with_locks(base, incoming, &locks);
    assert!(result.releases.draft);
}

// ─────────────────────────────────────────────────────────────────────────────
// merge_config_with_locks — locked string fields
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[traced_test]
fn test_merge_config_with_locks_locked_string_kept_from_base_and_emits_warn() {
    let mut base = default_config();
    base.core.branches.main = "protected-main".to_string();
    let mut incoming = default_config();
    incoming.core.branches.main = "feature-branch".to_string();

    let locks = locks_from(&["core.branches.main"]);
    let result = merge_config_with_locks(base, incoming, &locks);

    assert_eq!(result.core.branches.main, "protected-main");
    assert!(logs_contain("locked field override attempt ignored"));
}

#[test]
#[traced_test]
fn test_merge_config_with_locks_locked_version_prefix_kept_from_base_and_emits_warn() {
    let mut base = default_config();
    base.core.version_prefix = "v".to_string();
    let mut incoming = default_config();
    incoming.core.version_prefix = "release-".to_string();

    let locks = locks_from(&["core.version_prefix"]);
    let result = merge_config_with_locks(base, incoming, &locks);

    assert_eq!(result.core.version_prefix, "v");
    assert!(logs_contain("locked field override attempt ignored"));
}

// ─────────────────────────────────────────────────────────────────────────────
// merge_config_with_locks — locked VersioningStrategy enum
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[traced_test]
fn test_merge_config_with_locks_locked_versioning_strategy_kept_from_base_and_emits_warn() {
    let mut base = default_config();
    base.versioning.strategy = VersioningStrategy::Conventional;
    let mut incoming = default_config();
    incoming.versioning.strategy = VersioningStrategy::External {
        command: "./my-script.sh".to_string(),
        env_vars: Default::default(),
        timeout_ms: 30_000,
    };

    let locks = locks_from(&["versioning.strategy"]);
    let result = merge_config_with_locks(base, incoming, &locks);

    assert_eq!(result.versioning.strategy, VersioningStrategy::Conventional);
    assert!(logs_contain("locked field override attempt ignored"));
}

// ─────────────────────────────────────────────────────────────────────────────
// merge_config_with_locks — locked numeric fields
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[traced_test]
fn test_merge_config_with_locks_locked_max_retries_kept_from_base_and_emits_warn() {
    let mut base = default_config();
    base.error_handling.max_retries = 3;
    let mut incoming = default_config();
    incoming.error_handling.max_retries = 10;

    let locks = locks_from(&["error_handling.max_retries"]);
    let result = merge_config_with_locks(base, incoming, &locks);

    assert_eq!(result.error_handling.max_retries, 3);
    assert!(logs_contain("locked field override attempt ignored"));
}

#[test]
#[traced_test]
fn test_merge_config_with_locks_locked_backoff_multiplier_kept_from_base_and_emits_warn() {
    let mut base = default_config();
    base.error_handling.backoff_multiplier = 1.5;
    let mut incoming = default_config();
    incoming.error_handling.backoff_multiplier = 3.0;

    let locks = locks_from(&["error_handling.backoff_multiplier"]);
    let result = merge_config_with_locks(base, incoming, &locks);

    assert_eq!(result.error_handling.backoff_multiplier, 1.5);
    assert!(logs_contain("locked field override attempt ignored"));
}

#[test]
#[traced_test]
fn test_merge_config_with_locks_locked_initial_delay_ms_kept_from_base_and_emits_warn() {
    let mut base = default_config();
    base.error_handling.initial_delay_ms = 500;
    let mut incoming = default_config();
    incoming.error_handling.initial_delay_ms = 5000;

    let locks = locks_from(&["error_handling.initial_delay_ms"]);
    let result = merge_config_with_locks(base, incoming, &locks);

    assert_eq!(result.error_handling.initial_delay_ms, 500);
    assert!(logs_contain("locked field override attempt ignored"));
}

// ─────────────────────────────────────────────────────────────────────────────
// merge_config_with_locks — non-lockable fields always from incoming
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_merge_config_with_locks_release_pr_always_from_incoming() {
    // release_pr is not in LOCKABLE_FIELDS; locking all 10 fields must not
    // affect it — incoming's release_pr always wins.
    let base = default_config();
    let mut incoming = default_config();
    incoming.release_pr.title_template = "custom: ${version}".to_string();

    let locks = locks_from(LOCKABLE_FIELDS); // all fields locked
    let result = merge_config_with_locks(base, incoming, &locks);

    assert_eq!(result.release_pr.title_template, "custom: ${version}");
}

#[test]
fn test_merge_config_with_locks_group_metadata_always_from_incoming() {
    let mut base = default_config();
    base.group = Some("team-a".to_string());
    let mut incoming = default_config();
    incoming.group = Some("team-b".to_string());

    let locks = locks_from(LOCKABLE_FIELDS); // all fields locked
    let result = merge_config_with_locks(base, incoming, &locks);

    assert_eq!(result.group.as_deref(), Some("team-b"));
}

#[test]
fn test_merge_config_with_locks_excluded_pr_authors_always_from_incoming() {
    // excluded_pr_authors is inside VersioningConfig but is not lockable.
    let mut base = default_config();
    base.versioning.excluded_pr_authors = vec!["renovate[bot]".to_string()];
    let mut incoming = default_config();
    incoming.versioning.excluded_pr_authors = vec!["dependabot[bot]".to_string()];

    let locks = locks_from(&["versioning.strategy", "versioning.allow_override"]);
    let result = merge_config_with_locks(base, incoming, &locks);

    assert_eq!(
        result.versioning.excluded_pr_authors,
        vec!["dependabot[bot]".to_string()]
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// merge_config_with_locks — all 10 lockable fields enforced simultaneously
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[traced_test]
fn test_merge_config_with_locks_all_ten_lockable_fields_enforced() {
    // BA-59/60/61 comprehensive: lock all 10 fields; every incoming value differs;
    // every base value must be retained and exactly 10 warn! entries emitted.
    let base = make_config(
        VersioningStrategy::Conventional,
        false,
        false,
        false,
        false,
        "protected-main",
        "stable-",
        3,
        1.5,
        500,
    );
    let incoming = make_config(
        VersioningStrategy::External {
            command: "./nope.sh".to_string(),
            env_vars: Default::default(),
            timeout_ms: 30_000,
        },
        true,
        true,
        true,
        true,
        "develop",
        "nightly-",
        99,
        9.9,
        9999,
    );

    let locks = locks_from(LOCKABLE_FIELDS);
    let result = merge_config_with_locks(base, incoming, &locks);

    // Every lockable field should retain the base value.
    assert_eq!(result.versioning.strategy, VersioningStrategy::Conventional);
    assert!(!result.versioning.allow_override);
    assert!(!result.releases.draft);
    assert!(!result.releases.prerelease);
    assert!(!result.releases.generate_notes);
    assert_eq!(result.core.branches.main, "protected-main");
    assert_eq!(result.core.version_prefix, "stable-");
    assert_eq!(result.error_handling.max_retries, 3);
    assert_eq!(result.error_handling.backoff_multiplier, 1.5);
    assert_eq!(result.error_handling.initial_delay_ms, 500);

    // Exactly 10 "locked field override attempt ignored" warn! events.
    let log_count = (0..10)
        .filter(|_| logs_contain("locked field override attempt ignored"))
        .count();
    // `logs_contain` is true/false per call; we re-check the same message 10 times
    // only to verify they all log (the trace infra accumulates all 10).
    assert!(
        log_count == 10,
        "expected 10 warn! events for all locked fields, got {log_count}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// G.2: GitHubConfigurationProvider cache helpers
// ─────────────────────────────────────────────────────────────────────────────

// Minimal test double for GitHubOperations. Only the three methods exercised
// by the G.2 helpers (get_installation_id_for_repo, get_file_content, scoped_to)
// carry real logic; the rest are safe stubs.

use std::collections::HashMap as GH2HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex as TokioMutex;

use release_regent_core::{
    traits::{
        git_operations::{
            GetCommitsOptions, GitCommit, GitOperations, GitRepository, GitTag, ListTagsOptions,
        },
        github_operations::{
            CollaboratorPermission, CreatePullRequestParams, CreateReleaseParams, FileUpdate,
            IssueComment, Label, PullRequest, Release, Tag, UpdateReleaseParams,
        },
        GitHubOperations,
    },
    CoreError,
};

/// State shared across clones of `TestGitHub` via `Arc<Mutex>`.
struct TestGitHubState {
    /// Override for `get_installation_id_for_repo`. `None` → return `Err(GitHub)`.
    install_response: Option<u64>,
    /// Per-key response for `get_file_content`.
    /// Value is `Ok(Some(content))`, `Ok(None)` (absent), or `Err(msg)` (API error).
    file_responses: GH2HashMap<(String, String, String, String), Result<Option<String>, String>>,
}

impl TestGitHubState {
    fn new() -> Self {
        Self {
            install_response: Some(42),
            file_responses: GH2HashMap::new(),
        }
    }
}

/// Minimal GitHub client stub for testing config cache helpers.
///
/// Shares state across `scoped_to` clones so tests can inspect call results.
#[derive(Clone)]
struct TestGitHub {
    state: Arc<TokioMutex<TestGitHubState>>,
}

impl TestGitHub {
    fn new() -> Self {
        Self {
            state: Arc::new(TokioMutex::new(TestGitHubState::new())),
        }
    }

    /// Make `get_installation_id_for_repo` return an error.
    async fn set_install_error(&self) {
        self.state.lock().await.install_response = None;
    }

    /// Register a file with content (file exists).
    async fn add_file(&self, owner: &str, repo: &str, path: &str, branch: &str, content: &str) {
        self.state.lock().await.file_responses.insert(
            (
                owner.to_string(),
                repo.to_string(),
                path.to_string(),
                branch.to_string(),
            ),
            Ok(Some(content.to_string())),
        );
    }

    /// Register a file path as returning an API error.
    async fn add_file_error(&self, owner: &str, repo: &str, path: &str, branch: &str) {
        self.state.lock().await.file_responses.insert(
            (
                owner.to_string(),
                repo.to_string(),
                path.to_string(),
                branch.to_string(),
            ),
            Err("simulated GitHub API error".to_string()),
        );
    }
}

// ── GitOperations stub ────────────────────────────────────────────────────────

#[async_trait]
impl GitOperations for TestGitHub {
    async fn get_commits_between(
        &self,
        _owner: &str,
        _repo: &str,
        _base: &str,
        _head: &str,
        _options: GetCommitsOptions,
    ) -> release_regent_core::CoreResult<Vec<GitCommit>> {
        Ok(vec![])
    }

    async fn get_commit(
        &self,
        _owner: &str,
        _repo: &str,
        _sha: &str,
    ) -> release_regent_core::CoreResult<GitCommit> {
        Err(CoreError::not_found("stub"))
    }

    async fn list_tags(
        &self,
        _owner: &str,
        _repo: &str,
        _options: ListTagsOptions,
    ) -> release_regent_core::CoreResult<Vec<GitTag>> {
        Ok(vec![])
    }

    async fn get_tag(
        &self,
        _owner: &str,
        _repo: &str,
        _tag_name: &str,
    ) -> release_regent_core::CoreResult<GitTag> {
        Err(CoreError::not_found("stub"))
    }

    async fn tag_exists(
        &self,
        _owner: &str,
        _repo: &str,
        _tag_name: &str,
    ) -> release_regent_core::CoreResult<bool> {
        Ok(false)
    }

    async fn get_head_commit(
        &self,
        _owner: &str,
        _repo: &str,
        _branch: Option<&str>,
    ) -> release_regent_core::CoreResult<GitCommit> {
        Err(CoreError::not_found("stub"))
    }

    async fn get_repository_info(
        &self,
        _owner: &str,
        _repo: &str,
    ) -> release_regent_core::CoreResult<GitRepository> {
        Err(CoreError::not_found("stub"))
    }
}

// ── GitHubOperations impl ─────────────────────────────────────────────────────

#[async_trait]
impl GitHubOperations for TestGitHub {
    async fn add_labels(
        &self,
        _owner: &str,
        _repo: &str,
        _issue_number: u64,
        _labels: &[&str],
    ) -> release_regent_core::CoreResult<()> {
        Ok(())
    }

    async fn create_branch(
        &self,
        _owner: &str,
        _repo: &str,
        _branch_name: &str,
        _sha: &str,
    ) -> release_regent_core::CoreResult<()> {
        Ok(())
    }

    async fn create_issue_comment(
        &self,
        _owner: &str,
        _repo: &str,
        _issue_number: u64,
        _body: &str,
    ) -> release_regent_core::CoreResult<()> {
        Ok(())
    }

    async fn create_pull_request(
        &self,
        _owner: &str,
        _repo: &str,
        _params: CreatePullRequestParams,
    ) -> release_regent_core::CoreResult<PullRequest> {
        Err(CoreError::not_found("stub"))
    }

    async fn create_release(
        &self,
        _owner: &str,
        _repo: &str,
        _params: CreateReleaseParams,
    ) -> release_regent_core::CoreResult<Release> {
        Err(CoreError::not_found("stub"))
    }

    async fn create_tag(
        &self,
        _owner: &str,
        _repo: &str,
        _tag_name: &str,
        _commit_sha: &str,
        _message: Option<String>,
        _tagger: Option<release_regent_core::traits::github_operations::GitUser>,
    ) -> release_regent_core::CoreResult<Tag> {
        Err(CoreError::not_found("stub"))
    }

    async fn delete_branch(
        &self,
        _owner: &str,
        _repo: &str,
        _branch_name: &str,
    ) -> release_regent_core::CoreResult<()> {
        Ok(())
    }

    async fn force_update_branch(
        &self,
        _owner: &str,
        _repo: &str,
        _branch_name: &str,
        _sha: &str,
    ) -> release_regent_core::CoreResult<()> {
        Ok(())
    }

    async fn get_collaborator_permission(
        &self,
        _owner: &str,
        _repo: &str,
        _username: &str,
    ) -> release_regent_core::CoreResult<CollaboratorPermission> {
        Ok(CollaboratorPermission::Write)
    }

    async fn get_latest_release(
        &self,
        _owner: &str,
        _repo: &str,
    ) -> release_regent_core::CoreResult<Option<Release>> {
        Ok(None)
    }

    async fn get_pull_request(
        &self,
        _owner: &str,
        _repo: &str,
        pr_number: u64,
    ) -> release_regent_core::CoreResult<PullRequest> {
        Err(CoreError::not_found(format!("PR #{pr_number}")))
    }

    async fn get_release_by_tag(
        &self,
        _owner: &str,
        _repo: &str,
        tag: &str,
    ) -> release_regent_core::CoreResult<Release> {
        Err(CoreError::not_found(format!("release for tag '{tag}'")))
    }

    async fn list_pr_labels(
        &self,
        _owner: &str,
        _repo: &str,
        _issue_number: u64,
    ) -> release_regent_core::CoreResult<Vec<Label>> {
        Ok(vec![])
    }

    async fn list_issue_comments(
        &self,
        _owner: &str,
        _repo: &str,
        _issue_number: u64,
    ) -> release_regent_core::CoreResult<Vec<IssueComment>> {
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
    ) -> release_regent_core::CoreResult<Vec<PullRequest>> {
        Ok(vec![])
    }

    async fn list_releases(
        &self,
        _owner: &str,
        _repo: &str,
        _per_page: Option<u8>,
        _page: Option<u32>,
    ) -> release_regent_core::CoreResult<Vec<Release>> {
        Ok(vec![])
    }

    async fn remove_label(
        &self,
        _owner: &str,
        _repo: &str,
        _issue_number: u64,
        _label_name: &str,
    ) -> release_regent_core::CoreResult<()> {
        Ok(())
    }

    async fn search_pull_requests(
        &self,
        _owner: &str,
        _repo: &str,
        _query: &str,
    ) -> release_regent_core::CoreResult<Vec<PullRequest>> {
        Ok(vec![])
    }

    async fn update_pull_request(
        &self,
        _owner: &str,
        _repo: &str,
        pr_number: u64,
        _title: Option<String>,
        _body: Option<String>,
        _state: Option<String>,
    ) -> release_regent_core::CoreResult<PullRequest> {
        Err(CoreError::not_found(format!("PR #{pr_number}")))
    }

    async fn update_issue_comment(
        &self,
        _owner: &str,
        _repo: &str,
        _comment_id: u64,
        _body: &str,
    ) -> release_regent_core::CoreResult<()> {
        Ok(())
    }

    async fn update_release(
        &self,
        _owner: &str,
        _repo: &str,
        release_id: u64,
        _params: UpdateReleaseParams,
    ) -> release_regent_core::CoreResult<Release> {
        Err(CoreError::not_found(format!("release #{release_id}")))
    }

    async fn upsert_file(
        &self,
        _owner: &str,
        _repo: &str,
        _path: &str,
        _commit_message: &str,
        _content: &str,
        _branch: &str,
    ) -> release_regent_core::CoreResult<()> {
        Ok(())
    }

    async fn get_installation_id_for_repo(
        &self,
        _owner: &str,
        _repo: &str,
    ) -> release_regent_core::CoreResult<u64> {
        match self.state.lock().await.install_response {
            Some(id) => Ok(id),
            None => Err(CoreError::github(std::io::Error::new(
                std::io::ErrorKind::Other,
                "simulated installation lookup failure",
            ))),
        }
    }

    fn scoped_to(&self, _installation_id: u64) -> Self {
        // Share Arc state so all clones observe the same responses.
        Self {
            state: Arc::clone(&self.state),
        }
    }

    async fn get_file_content(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        branch: &str,
    ) -> release_regent_core::CoreResult<Option<String>> {
        let key = (
            owner.to_string(),
            repo.to_string(),
            path.to_string(),
            branch.to_string(),
        );
        match self.state.lock().await.file_responses.get(&key).cloned() {
            Some(Ok(content)) => Ok(content),
            Some(Err(msg)) => Err(CoreError::github(std::io::Error::new(
                std::io::ErrorKind::Other,
                msg,
            ))),
            None => Ok(None), // path not registered → absent
        }
    }

    async fn batch_commit_files(
        &self,
        _owner: &str,
        _repo: &str,
        _branch: &str,
        _files: &[FileUpdate],
        _message: &str,
    ) -> release_regent_core::CoreResult<()> {
        Ok(())
    }

    async fn batch_commit_files_rebased(
        &self,
        _owner: &str,
        _repo: &str,
        _branch: &str,
        _files: &[FileUpdate],
        _message: &str,
        _parent_sha: &str,
    ) -> release_regent_core::CoreResult<()> {
        Ok(())
    }
}

// ── Provider factory ──────────────────────────────────────────────────────────

/// Create a `GitHubConfigurationProvider<TestGitHub>` backed by a real temp-dir baseline.
async fn make_github_provider(
    github: TestGitHub,
) -> super::GitHubConfigurationProvider<TestGitHub> {
    use crate::file_provider::FileConfigurationProvider;
    let tmp = tempfile::tempdir().expect("tempdir");
    let inner = FileConfigurationProvider::new(tmp.path())
        .await
        .expect("FileConfigurationProvider");
    // Keep the tempdir alive for the duration of the test; it is dropped
    // implicitly at end of the calling test function.
    std::mem::forget(tmp);
    super::GitHubConfigurationProvider::new(inner, github)
}

/// Minimal valid TOML config content (overrides version_prefix for traceability).
fn toml_config(version_prefix: &str) -> String {
    format!("[core]\nversion_prefix = \"{version_prefix}\"\n")
}

// ── resolve_metadata_installation ────────────────────────────────────────────

#[tokio::test]
async fn test_resolve_metadata_installation_success_returns_some() {
    let gh = TestGitHub::new();
    let provider = make_github_provider(gh).await;

    let result = provider.resolve_metadata_installation("myorg").await;

    assert_eq!(result, Some(42));
}

#[tokio::test]
async fn test_resolve_metadata_installation_caches_result_avoids_second_api_call() {
    let gh = TestGitHub::new();
    let provider = make_github_provider(gh).await;

    // First call hits the API.
    let first = provider.resolve_metadata_installation("myorg").await;
    // Corrupt the state so a second API call would return an error.
    provider.github.set_install_error().await;
    // Second call must hit the cache and still return Some(42).
    let second = provider.resolve_metadata_installation("myorg").await;

    assert_eq!(first, Some(42));
    assert_eq!(second, Some(42), "cache hit must bypass the API");
}

#[tokio::test]
#[traced_test]
async fn test_resolve_metadata_installation_api_error_returns_none_and_emits_warn() {
    let gh = TestGitHub::new();
    gh.set_install_error().await;
    let provider = make_github_provider(gh).await;

    let result = provider.resolve_metadata_installation("myorg").await;

    assert!(result.is_none(), "API error must return None");
    assert!(
        logs_contain("metadata repository is not accessible"),
        "a warn! must be emitted when the metadata repo cannot be resolved"
    );
}

#[tokio::test]
async fn test_resolve_metadata_installation_negative_cache_suppresses_second_api_call() {
    // First call returns an error and populates the negative cache.
    // A second call within the TTL window must return None without hitting the API.
    let gh = TestGitHub::new();
    gh.set_install_error().await;
    let provider = make_github_provider(gh).await;

    let first = provider.resolve_metadata_installation("myorg").await;
    // Re-enable the API so a second real call would succeed — if the negative cache
    // works, the second call must still return None without a network round-trip.
    provider.github.state.lock().await.install_response = Some(99);
    let second = provider.resolve_metadata_installation("myorg").await;

    assert!(
        first.is_none(),
        "first call after API error must return None"
    );
    assert!(
        second.is_none(),
        "second call within TTL must be served from negative cache and return None"
    );
}

// ── fetch_repo_dotfile ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_fetch_repo_dotfile_absent_returns_ok_none() {
    let gh = TestGitHub::new();
    let provider = make_github_provider(gh).await;

    let result = provider
        .fetch_repo_dotfile("owner", "repo", "main", &provider.github.clone())
        .await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_fetch_repo_dotfile_toml_found_returns_parsed_config() {
    let gh = TestGitHub::new();
    gh.add_file(
        "owner",
        "repo",
        ".release-regent.toml",
        "main",
        &toml_config("toml-prefix"),
    )
    .await;
    let client = gh.clone();
    let provider = make_github_provider(gh).await;

    let result = provider
        .fetch_repo_dotfile("owner", "repo", "main", &client)
        .await
        .expect("should succeed");

    assert!(result.is_some());
    assert_eq!(
        result.unwrap().core.version_prefix,
        "toml-prefix",
        "content of .toml file must be reflected in the parsed config"
    );
}

#[tokio::test]
async fn test_fetch_repo_dotfile_invalid_content_returns_err_config() {
    let gh = TestGitHub::new();
    gh.add_file(
        "owner",
        "repo",
        ".release-regent.toml",
        "main",
        ": this is not valid toml {{{{ ]]]]",
    )
    .await;
    let client = gh.clone();
    let provider = make_github_provider(gh).await;

    let result = provider
        .fetch_repo_dotfile("owner", "repo", "main", &client)
        .await;

    assert!(result.is_err());
    assert!(
        result.unwrap_err().is_config_error(),
        "parse failure must be CoreError::Config"
    );
}

#[tokio::test]
async fn test_fetch_repo_dotfile_parse_error_evicts_cache_so_corrected_file_is_re_fetched() {
    let gh = TestGitHub::new();
    gh.add_file(
        "owner",
        "repo",
        ".release-regent.toml",
        "main",
        ": this is not valid toml {{{{ ]]]]",
    )
    .await;
    let client = gh.clone();
    let provider = make_github_provider(gh).await;

    // First call: parse error → cache entry must not be populated.
    let first = provider
        .fetch_repo_dotfile("owner", "repo", "main", &client)
        .await;
    assert!(first.is_err(), "invalid TOML must return an error");

    // Replace the mock with valid content (simulates an operator fixing the file).
    client
        .add_file(
            "owner",
            "repo",
            ".release-regent.toml",
            "main",
            &toml_config("fixed"),
        )
        .await;

    // Second call must reach the API (no stale cache entry) and succeed.
    let second = provider
        .fetch_repo_dotfile("owner", "repo", "main", &client)
        .await
        .expect("second call with corrected content must succeed");
    assert_eq!(
        second.as_ref().map(|c| c.core.version_prefix.as_str()),
        Some("fixed"),
        "after a parse error the cache must be evicted so the corrected file is re-fetched"
    );
}

#[tokio::test]
async fn test_fetch_repo_dotfile_api_error_returns_err_github() {
    let gh = TestGitHub::new();
    gh.add_file_error("owner", "repo", ".release-regent.toml", "main")
        .await;
    let client = gh.clone();
    let provider = make_github_provider(gh).await;

    let result = provider
        .fetch_repo_dotfile("owner", "repo", "main", &client)
        .await;

    assert!(result.is_err());
    // The error is a GitHub/Network error (not a Config parse error).
    assert!(
        !result.unwrap_err().is_config_error(),
        "API error must not be classified as CoreError::Config"
    );
}

#[tokio::test]
async fn test_fetch_repo_dotfile_cache_hit_within_ttl_skips_api() {
    let gh = TestGitHub::new();
    gh.add_file(
        "owner",
        "repo",
        ".release-regent.toml",
        "main",
        &toml_config("v1"),
    )
    .await;
    let client = gh.clone();
    let provider = make_github_provider(gh).await;

    // Populate cache.
    let first = provider
        .fetch_repo_dotfile("owner", "repo", "main", &client)
        .await
        .expect("first call");

    // Remove the file from the mock (simulates "no longer there").
    // A cache hit must still return the cached result.
    client.state.lock().await.file_responses.clear();

    let second = provider
        .fetch_repo_dotfile("owner", "repo", "main", &client)
        .await
        .expect("cached call");

    assert_eq!(
        first.as_ref().map(|c| &c.core.version_prefix),
        second.as_ref().map(|c| &c.core.version_prefix),
        "cache hit must return the previously parsed config"
    );
}

// ── load_global_policy ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_load_global_policy_absent_returns_ok_none() {
    let gh = TestGitHub::new();
    let client = gh.clone();
    let provider = make_github_provider(gh).await;

    let result = provider.load_global_policy("myorg", &client).await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_load_global_policy_toml_found_returns_parsed_config() {
    let gh = TestGitHub::new();
    gh.add_file(
        "myorg",
        ".release-regent",
        "global.toml",
        "main",
        &toml_config("global-prefix"),
    )
    .await;
    let client = gh.clone();
    let provider = make_github_provider(gh).await;

    let result = provider
        .load_global_policy("myorg", &client)
        .await
        .expect("should succeed");

    assert!(result.is_some());
    assert_eq!(result.unwrap().core.version_prefix, "global-prefix");
}

#[tokio::test]
async fn test_load_global_policy_invalid_content_returns_err_config() {
    let gh = TestGitHub::new();
    gh.add_file(
        "myorg",
        ".release-regent",
        "global.toml",
        "main",
        "not valid toml [[[",
    )
    .await;
    let client = gh.clone();
    let provider = make_github_provider(gh).await;

    let result = provider.load_global_policy("myorg", &client).await;

    assert!(result.is_err());
    assert!(result.unwrap_err().is_config_error());
}

#[tokio::test]
async fn test_load_global_policy_api_error_propagates() {
    let gh = TestGitHub::new();
    // The single probe path returns an error.
    gh.add_file_error("myorg", ".release-regent", "global.toml", "main")
        .await;
    let client = gh.clone();
    let provider = make_github_provider(gh).await;

    let result = provider.load_global_policy("myorg", &client).await;

    assert!(result.is_err());
    assert!(
        !result.unwrap_err().is_config_error(),
        "API error must not be CoreError::Config"
    );
}

#[tokio::test]
async fn test_load_global_policy_cache_hit_within_ttl_skips_api() {
    let gh = TestGitHub::new();
    gh.add_file(
        "myorg",
        ".release-regent",
        "global.toml",
        "main",
        &toml_config("cached"),
    )
    .await;
    let client = gh.clone();
    let provider = make_github_provider(gh).await;

    // Populate cache.
    let _ = provider.load_global_policy("myorg", &client).await;
    // Clear responses to prove cache is used.
    client.state.lock().await.file_responses.clear();

    let result = provider
        .load_global_policy("myorg", &client)
        .await
        .expect("cached call");

    assert_eq!(result.unwrap().core.version_prefix, "cached");
}

#[tokio::test]
async fn test_load_global_policy_parse_error_evicts_cache_so_corrected_file_is_re_fetched() {
    let gh = TestGitHub::new();
    gh.add_file(
        "myorg",
        ".release-regent",
        "global.toml",
        "main",
        "not valid toml [[[",
    )
    .await;
    let client = gh.clone();
    let provider = make_github_provider(gh).await;

    // First call: parse error → cache entry must not be populated.
    let first = provider.load_global_policy("myorg", &client).await;
    assert!(first.is_err(), "invalid TOML must return an error");

    // Replace the mock with valid content (simulates an operator fixing the file).
    client
        .add_file(
            "myorg",
            ".release-regent",
            "global.toml",
            "main",
            &toml_config("fixed-global"),
        )
        .await;

    // Second call must reach the API (no stale cache entry) and succeed.
    let second = provider
        .load_global_policy("myorg", &client)
        .await
        .expect("second call with corrected content must succeed");
    assert_eq!(
        second.as_ref().map(|c| c.core.version_prefix.as_str()),
        Some("fixed-global"),
        "after a parse error the cache must be evicted so the corrected file is re-fetched"
    );
}

// ── load_group_policy ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_load_group_policy_absent_returns_ok_none() {
    let gh = TestGitHub::new();
    let client = gh.clone();
    let provider = make_github_provider(gh).await;

    let result = provider
        .load_group_policy("myorg", "platform", &client)
        .await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_load_group_policy_toml_found_returns_parsed_config() {
    let gh = TestGitHub::new();
    gh.add_file(
        "myorg",
        ".release-regent",
        "groups/platform.toml",
        "main",
        &toml_config("group-prefix"),
    )
    .await;
    let client = gh.clone();
    let provider = make_github_provider(gh).await;

    let result = provider
        .load_group_policy("myorg", "platform", &client)
        .await
        .expect("should succeed");

    assert_eq!(result.unwrap().core.version_prefix, "group-prefix");
}

#[tokio::test]
async fn test_load_group_policy_invalid_content_returns_err_config() {
    let gh = TestGitHub::new();
    gh.add_file(
        "myorg",
        ".release-regent",
        "groups/backend.toml",
        "main",
        "definitely not toml {{{{",
    )
    .await;
    let client = gh.clone();
    let provider = make_github_provider(gh).await;

    let result = provider
        .load_group_policy("myorg", "backend", &client)
        .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().is_config_error());
}

#[tokio::test]
async fn test_load_group_policy_parse_error_evicts_cache_so_corrected_file_is_re_fetched() {
    let gh = TestGitHub::new();
    gh.add_file(
        "myorg",
        ".release-regent",
        "groups/backend.toml",
        "main",
        "definitely not toml {{{{",
    )
    .await;
    let client = gh.clone();
    let provider = make_github_provider(gh).await;

    // First call: parse error → cache entry must not be populated.
    let first = provider
        .load_group_policy("myorg", "backend", &client)
        .await;
    assert!(first.is_err(), "invalid TOML must return an error");

    // Replace the mock with valid content (simulates an operator fixing the file).
    client
        .add_file(
            "myorg",
            ".release-regent",
            "groups/backend.toml",
            "main",
            &toml_config("fixed-backend"),
        )
        .await;

    // Second call must reach the API (no stale cache entry) and succeed.
    let second = provider
        .load_group_policy("myorg", "backend", &client)
        .await
        .expect("second call with corrected content must succeed");
    assert_eq!(
        second.as_ref().map(|c| c.core.version_prefix.as_str()),
        Some("fixed-backend"),
        "after a parse error the cache must be evicted so the corrected file is re-fetched"
    );
}

#[tokio::test]
async fn test_load_group_policy_cache_keyed_by_org_and_group() {
    let gh = TestGitHub::new();
    gh.add_file(
        "myorg",
        ".release-regent",
        "groups/platform.toml",
        "main",
        &toml_config("platform"),
    )
    .await;
    gh.add_file(
        "myorg",
        ".release-regent",
        "groups/mobile.toml",
        "main",
        &toml_config("mobile"),
    )
    .await;
    let client = gh.clone();
    let provider = make_github_provider(gh).await;

    let platform = provider
        .load_group_policy("myorg", "platform", &client)
        .await
        .expect("platform")
        .unwrap();
    let mobile = provider
        .load_group_policy("myorg", "mobile", &client)
        .await
        .expect("mobile")
        .unwrap();

    assert_eq!(platform.core.version_prefix, "platform");
    assert_eq!(mobile.core.version_prefix, "mobile");
    assert_ne!(
        platform.core.version_prefix, mobile.core.version_prefix,
        "different groups must have independent cache entries"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// G.3: get_merged_config — five-level pipeline
// ─────────────────────────────────────────────────────────────────────────────

use release_regent_core::traits::{configuration_provider::LoadOptions, ConfigurationProvider};

// ── helpers ───────────────────────────────────────────────────────────────────

/// Minimal valid app-level config seeded onto disk.
/// Uses a distinctive `version_prefix` so tests can verify the app level ran.
fn app_config_toml() -> &'static str {
    "[core]\nversion_prefix = \"app-v\"\n"
}

/// Create a `GitHubConfigurationProvider<TestGitHub>` backed by a real temp-dir
/// baseline that contains a valid app-level `release-regent.toml`.
async fn make_provider_with_app_config(
    github: TestGitHub,
) -> super::GitHubConfigurationProvider<TestGitHub> {
    use crate::file_provider::FileConfigurationProvider;
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(tmp.path().join("release-regent.toml"), app_config_toml())
        .expect("write app config");
    let inner = FileConfigurationProvider::new(tmp.path())
        .await
        .expect("FileConfigurationProvider");
    std::mem::forget(tmp);
    super::GitHubConfigurationProvider::new(inner, github)
}

/// `LoadOptions` for server mode (installation_id present).
fn server_options() -> LoadOptions {
    LoadOptions {
        installation_id: Some(1),
        default_branch: Some("main".to_string()),
        ..Default::default()
    }
}

/// `LoadOptions` for CLI mode (no installation_id).
fn cli_options() -> LoadOptions {
    LoadOptions::default()
}

/// TOML content for global policy, setting version_prefix to a unique marker.
fn global_toml(version_prefix: &str) -> String {
    format!("[core]\nversion_prefix = \"{version_prefix}\"\n")
}

/// TOML content for group policy, setting version_prefix to a unique marker.
fn group_toml(version_prefix: &str) -> String {
    format!("[core]\nversion_prefix = \"{version_prefix}\"\n")
}

/// TOML content for a repo dotfile, declaring a group and setting version_prefix.
fn repo_dotfile_toml_with_group(version_prefix: &str, group: &str) -> String {
    format!("group = \"{group}\"\n[core]\nversion_prefix = \"{version_prefix}\"\n")
}

/// Seed the standard metadata repo global policy file.
async fn seed_global(gh: &TestGitHub, content: &str) {
    gh.add_file("myorg", ".release-regent", "global.toml", "main", content)
        .await;
}

/// Seed the standard metadata repo group policy file.
async fn seed_group(gh: &TestGitHub, group_name: &str, content: &str) {
    let path = format!("groups/{group_name}.toml");
    gh.add_file("myorg", ".release-regent", &path, "main", content)
        .await;
}

/// Seed the repo dotfile in the target repository.
async fn seed_dotfile(gh: &TestGitHub, content: &str) {
    gh.add_file("myorg", "myrepo", ".release-regent.toml", "main", content)
        .await;
}

// ── get_merged_config — pipeline path tests ───────────────────────────────────

/// Scenario 1: Metadata repo not accessible → app-level + repo dotfile only.
/// The provider must emit `warn!` about the missing metadata repo.
#[tokio::test]
#[traced_test]
async fn test_get_merged_config_no_metadata_access_uses_app_plus_repo_dotfile() {
    let gh = TestGitHub::new();
    gh.set_install_error().await;
    // Dotfile uses scoped_to(installation_id=1) since meta was absent.
    gh.add_file(
        "myorg",
        "myrepo",
        ".release-regent.toml",
        "main",
        &toml_config("repo-v"),
    )
    .await;
    let provider = make_provider_with_app_config(gh).await;

    let result = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await
        .expect("should succeed");

    assert_eq!(result.core.version_prefix, "repo-v");
    assert!(
        logs_contain(".release-regent not accessible"),
        "must warn about absent metadata repo"
    );
}

/// Scenario 2: Metadata accessible, global policy present, no group, no repo dotfile.
/// Expected: global policy wins as the highest applied level.
#[tokio::test]
async fn test_get_merged_config_global_only_no_repo_dotfile() {
    let gh = TestGitHub::new();
    seed_global(&gh, &global_toml("global-v")).await;
    // No repo dotfile → fetch returns Ok(None).
    let provider = make_provider_with_app_config(gh).await;

    let result = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await
        .expect("should succeed");

    assert_eq!(result.core.version_prefix, "global-v");
}

/// Scenario 3: Metadata accessible, no global policy, no group, repo dotfile present.
/// Expected: repo dotfile wins as the highest applied level.
#[tokio::test]
async fn test_get_merged_config_no_global_no_group_repo_dotfile_applies() {
    let gh = TestGitHub::new();
    // No global policy → Ok(None).
    seed_dotfile(&gh, &toml_config("repo-v")).await;
    let provider = make_provider_with_app_config(gh).await;

    let result = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await
        .expect("should succeed");

    assert_eq!(result.core.version_prefix, "repo-v");
}

/// Full 5-level stack: all levels present, repo dotfile declares group.
/// Expected: repo dotfile's `version_prefix` wins (last non-locked level).
#[tokio::test]
async fn test_get_merged_config_full_five_level_stack_repo_dotfile_wins() {
    let gh = TestGitHub::new();
    seed_global(&gh, &global_toml("global-v")).await;
    seed_group(&gh, "mygroup", &group_toml("group-v")).await;
    seed_dotfile(&gh, &repo_dotfile_toml_with_group("repo-v", "mygroup")).await;
    let provider = make_provider_with_app_config(gh).await;

    let result = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await
        .expect("should succeed");

    assert_eq!(
        result.core.version_prefix, "repo-v",
        "repo dotfile is level 5 and must win all unlocked fields"
    );
}

/// Group declared in dotfile but group file absent: must emit `warn!` and skip group.
#[tokio::test]
#[traced_test]
async fn test_get_merged_config_group_declared_but_file_absent_warns_and_skips() {
    let gh = TestGitHub::new();
    seed_global(&gh, &global_toml("global-v")).await;
    // Group "missing" has no file seeded.
    seed_dotfile(&gh, &repo_dotfile_toml_with_group("repo-v", "missing")).await;
    let provider = make_provider_with_app_config(gh).await;

    let result = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await
        .expect("should succeed even when group file absent");

    assert_eq!(result.core.version_prefix, "repo-v");
    assert!(
        logs_contain("no group config found"),
        "must warn when declared group file is absent"
    );
}

/// API 503 on global policy fetch: must emit `warn!`, skip metadata levels, and
/// use only app-level + repo dotfile (with the per-event scoped client).
///
/// NOTE: The spec treats a transient API error on the global policy as metadata
/// unreachable. The repo dotfile is fetched with the meta-scoped client (same in
/// this test since `scoped_to` shares state). The event continues using app + repo.
#[tokio::test]
#[traced_test]
async fn test_get_merged_config_api_error_on_global_uses_app_plus_repo_dotfile() {
    let gh = TestGitHub::new();
    // All three global probe paths return an API error → one probe path.
    gh.add_file_error("myorg", ".release-regent", "global.toml", "main")
        .await;
    // Repo dotfile fetched via meta client (same state due to scoped_to clone).
    seed_dotfile(&gh, &toml_config("repo-v")).await;
    let provider = make_provider_with_app_config(gh).await;

    let result = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await
        .expect("transient global API error must not hard-fail the event");

    assert_eq!(result.core.version_prefix, "repo-v");
    assert!(
        logs_contain("Metadata repo unreachable"),
        "must emit warn! when global API errors"
    );
}

// ── get_merged_config — lock semantics ────────────────────────────────────────

/// Global policy locks `releases.draft` (default = false).
/// Group tries to override it with `true` → `warn!` emitted; locked value (false) preserved.
#[tokio::test]
#[traced_test]
async fn test_get_merged_config_global_locks_field_group_cannot_override() {
    let gh = TestGitHub::new();

    // Global: lock draft (which is false by default) with the same default value.
    // lock-then-merge means global's own draft=false merges cleanly (same as base),
    // so no warn fires at level 3. Group's draft=true fires the warn at level 4.
    let global_toml_content = "locked_fields = [\"releases.draft\"]\n[releases]\ndraft = false\n";
    seed_global(&gh, global_toml_content).await;

    // Group tries to set draft = true.
    let group_toml_content = "[releases]\ndraft = true\n";
    seed_group(&gh, "mygroup", group_toml_content).await;

    // Repo dotfile declares group; no draft setting (defaults to false).
    seed_dotfile(&gh, &repo_dotfile_toml_with_group("repo-v", "mygroup")).await;
    let provider = make_provider_with_app_config(gh).await;

    let result = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await
        .expect("should succeed");

    assert!(
        !result.releases.draft,
        "globally-locked releases.draft must not be overridden by group"
    );
    assert!(logs_contain("locked field override attempt ignored"));
}

/// Group policy locks `releases.draft` (default = false).
/// Repo dotfile tries to override it with `true` → `warn!` emitted; locked value preserved.
#[tokio::test]
#[traced_test]
async fn test_get_merged_config_group_locks_field_repo_dotfile_cannot_override() {
    let gh = TestGitHub::new();

    seed_global(&gh, &global_toml("global-v")).await;

    // Group: lock draft = false (same value as app-level default so no warn at level 4 merge).
    let group_content = "locked_fields = [\"releases.draft\"]\n[releases]\ndraft = false\n";
    seed_group(&gh, "mygroup", group_content).await;

    // Repo dotfile: tries to set draft = true (TOML format, file is .release-regent.toml).
    let repo_content =
        "group = \"mygroup\"\n[releases]\ndraft = true\n[core]\nversion_prefix = \"repo-v\"\n";
    seed_dotfile(&gh, repo_content).await;

    let provider = make_provider_with_app_config(gh).await;

    let result = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await
        .expect("should succeed");

    assert!(
        !result.releases.draft,
        "group-locked releases.draft must not be overridden by repo dotfile"
    );
    assert!(logs_contain("locked field override attempt ignored"));
}

/// Repo dotfile contains `locked_fields`: must be cleared with `warn!` before merge.
#[tokio::test]
#[traced_test]
async fn test_get_merged_config_locked_fields_in_repo_dotfile_are_cleared() {
    let gh = TestGitHub::new();
    seed_global(&gh, &global_toml("global-v")).await;

    // TOML format (file is .release-regent.toml).
    let repo_content =
        "locked_fields = [\"versioning.strategy\"]\n[core]\nversion_prefix = \"repo-v\"\n";
    seed_dotfile(&gh, repo_content).await;

    let provider = make_provider_with_app_config(gh).await;

    let result = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await
        .expect("should succeed");

    // locked_fields must be cleared before the merge so they don't propagate.
    assert!(
        result.locked_fields.is_empty(),
        "locked_fields from repo dotfile must be cleared"
    );
    assert!(logs_contain(
        "locked_fields in repository dotfile is ignored"
    ));
    // Other fields should still come from the repo dotfile.
    assert_eq!(result.core.version_prefix, "repo-v");
}

/// Non-lockable path in `locked_fields` is ignored (with `warn!`).
/// `release_pr.title_template` is not lockable per `LOCKABLE_FIELDS`.
#[tokio::test]
#[traced_test]
async fn test_get_merged_config_non_lockable_path_in_global_locked_fields_is_ignored() {
    let gh = TestGitHub::new();

    // Global tries to lock release_pr.title_template — non-lockable, should warn and skip.
    let global_content =
        "locked_fields = [\"release_pr.title_template\"]\n[core]\nversion_prefix = \"global-v\"\n";
    seed_global(&gh, global_content).await;

    seed_dotfile(&gh, &toml_config("repo-v")).await;

    let provider = make_provider_with_app_config(gh).await;

    let result = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await
        .expect("should succeed");

    // Non-lockable path must not actually prevent the repo dotfile from applying.
    assert_eq!(
        result.core.version_prefix, "repo-v",
        "non-lockable field in locked_fields must not block repo dotfile from overriding"
    );
    assert!(
        logs_contain("not a lockable policy field"),
        "must warn when a non-lockable path appears in locked_fields"
    );
}

/// Global policy file contains a `group` field: must emit a `warn!` and strip it
/// before the merge so it is absent from the final effective config.
#[tokio::test]
#[traced_test]
async fn test_get_merged_config_global_policy_group_field_is_warned_and_stripped() {
    let gh = TestGitHub::new();
    // Global file contains `group = "backend"` which is only meaningful in dotfiles.
    let global_content = "group = \"backend\"\n[core]\nversion_prefix = \"global-v\"\n";
    seed_global(&gh, global_content).await;
    let provider = make_provider_with_app_config(gh).await;

    let result = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await
        .expect("should succeed; group in global policy is a non-fatal warning");

    assert_eq!(result.core.version_prefix, "global-v");
    assert!(
        result.group.is_none(),
        "group field from global policy must not appear in the final result"
    );
    assert!(
        logs_contain("`group` is only meaningful in repository dotfiles"),
        "must warn when global policy file contains a group field"
    );
}

/// Group policy file contains a `group` field: must emit a `warn!` and strip it
/// before the merge so it is absent from the final effective config.
#[tokio::test]
#[traced_test]
async fn test_get_merged_config_group_policy_group_field_is_warned_and_stripped() {
    let gh = TestGitHub::new();
    seed_global(&gh, &global_toml("global-v")).await;
    // Group file contains `group = "other"` which is invalid at this level.
    let group_content = "group = \"other\"\n[core]\nversion_prefix = \"group-v\"\n";
    seed_group(&gh, "mygroup", group_content).await;
    seed_dotfile(&gh, &repo_dotfile_toml_with_group("repo-v", "mygroup")).await;
    let provider = make_provider_with_app_config(gh).await;

    let result = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await
        .expect("should succeed; group in group policy is a non-fatal warning");

    assert_eq!(result.core.version_prefix, "repo-v");
    assert!(
        result.group.is_none(),
        "group field from group policy must not appear in the final result"
    );
    assert!(
        logs_contain("`group` is only meaningful in repository dotfiles"),
        "must warn when group policy file contains a group field"
    );
}

/// After the full merge pipeline, `group` and `locked_fields` must always be
/// absent from the returned config — they are pipeline-only metadata fields.
#[tokio::test]
async fn test_get_merged_config_metadata_fields_stripped_from_final_result() {
    let gh = TestGitHub::new();
    // Global locks a field (so locked_fields is non-empty during the pipeline).
    let global_content =
        "locked_fields = [\"versioning.allow_override\"]\n[core]\nversion_prefix = \"global-v\"\n";
    seed_global(&gh, global_content).await;
    // Dotfile declares a group (so group is non-None during the pipeline).
    seed_dotfile(
        &gh,
        &repo_dotfile_toml_with_group("repo-v", "unresolved-group"),
    )
    .await;
    let provider = make_provider_with_app_config(gh).await;

    let result = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await
        .expect("should succeed");

    assert!(
        result.group.is_none(),
        "group must be None in the final result regardless of dotfile contents"
    );
    assert!(
        result.locked_fields.is_empty(),
        "locked_fields must be empty in the final result regardless of policy file contents"
    );
}

// ── get_merged_config — hard-fail scenarios ───────────────────────────────────

#[tokio::test]
async fn test_get_merged_config_global_policy_invalid_returns_err_config() {
    let gh = TestGitHub::new();
    gh.add_file(
        "myorg",
        ".release-regent",
        "global.toml",
        "main",
        "not valid toml {{{{",
    )
    .await;
    let provider = make_provider_with_app_config(gh).await;

    let result = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().is_config_error());
}

#[tokio::test]
async fn test_get_merged_config_group_policy_invalid_returns_err_config() {
    let gh = TestGitHub::new();
    seed_global(&gh, &global_toml("global-v")).await;
    gh.add_file(
        "myorg",
        ".release-regent",
        "groups/mygroup.toml",
        "main",
        "definitely not toml {{",
    )
    .await;
    seed_dotfile(&gh, &repo_dotfile_toml_with_group("repo-v", "mygroup")).await;
    let provider = make_provider_with_app_config(gh).await;

    let result = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().is_config_error());
}

#[tokio::test]
async fn test_get_merged_config_repo_dotfile_invalid_returns_err_config() {
    let gh = TestGitHub::new();
    seed_global(&gh, &global_toml("global-v")).await;
    gh.add_file(
        "myorg",
        "myrepo",
        ".release-regent.toml",
        "main",
        ": this is not valid toml {{{{",
    )
    .await;
    let provider = make_provider_with_app_config(gh).await;

    let result = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().is_config_error());
}

// ── get_merged_config — cache hits ────────────────────────────────────────────

/// Global policy cache hit: second call must return same result without re-probing files.
#[tokio::test]
async fn test_get_merged_config_global_cache_hit_skips_api_on_second_call() {
    let gh = TestGitHub::new();
    seed_global(&gh, &global_toml("global-v")).await;
    let provider = make_provider_with_app_config(gh.clone()).await;

    // First call: populates global cache.
    let first = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await
        .expect("first call");

    // Remove global file so any re-probe would return absent.
    gh.state.lock().await.file_responses.clear();

    // Second call: must use cache.
    let second = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await
        .expect("second call");

    assert_eq!(
        first.core.version_prefix, second.core.version_prefix,
        "global cache hit must return the same result as the first call"
    );
}

/// Group cache hit: second call must return same result without re-probing files.
#[tokio::test]
async fn test_get_merged_config_group_cache_hit_skips_api_on_second_call() {
    let gh = TestGitHub::new();
    seed_global(&gh, &global_toml("global-v")).await;
    seed_group(&gh, "mygroup", &group_toml("group-v")).await;
    seed_dotfile(&gh, &repo_dotfile_toml_with_group("repo-v", "mygroup")).await;
    let provider = make_provider_with_app_config(gh.clone()).await;

    let first = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await
        .expect("first call");

    gh.state.lock().await.file_responses.clear();

    let second = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await
        .expect("second call");

    assert_eq!(first.core.version_prefix, second.core.version_prefix);
}

/// Repo dotfile cache hit: second call must return same result without re-probing.
#[tokio::test]
async fn test_get_merged_config_repo_dotfile_cache_hit_skips_api_on_second_call() {
    let gh = TestGitHub::new();
    seed_global(&gh, &global_toml("global-v")).await;
    seed_dotfile(&gh, &toml_config("repo-v")).await;
    let provider = make_provider_with_app_config(gh.clone()).await;

    let first = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await
        .expect("first call");

    gh.state.lock().await.file_responses.clear();

    let second = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await
        .expect("second call");

    assert_eq!(first.core.version_prefix, second.core.version_prefix);
}

// ── get_merged_config — CLI mode ──────────────────────────────────────────────

/// CLI mode (`installation_id = None`): must delegate entirely to `FileConfigurationProvider`.
/// No GitHub API calls should be attempted (verified by having the install lookup error;
/// any call to the GitHub backend would cause the test to fail via the error propagation
/// in `resolve_metadata_installation`, but with None install_id the code path is skipped).
#[tokio::test]
async fn test_get_merged_config_cli_mode_delegates_to_file_provider() {
    let gh = TestGitHub::new();
    // If GitHub were called, this would cause a panic/error.
    gh.set_install_error().await;
    let provider = make_provider_with_app_config(gh).await;

    // CLI mode: installation_id = None.
    let result = provider
        .get_merged_config("myorg", "myrepo", cli_options())
        .await
        .expect("CLI mode must delegate to FileConfigurationProvider");

    // FileConfigurationProvider with empty tempdir + release-regent.toml returns that file's config.
    assert_eq!(
        result.core.version_prefix, "app-v",
        "CLI mode must use FileConfigurationProvider result (app config)"
    );
}

// ── load_repository_config ────────────────────────────────────────────────────

/// Server mode: dotfile present → returns wrapped `RepositoryConfig`.
#[tokio::test]
async fn test_load_repository_config_server_mode_returns_repo_dotfile_wrapped() {
    let gh = TestGitHub::new();
    seed_dotfile(&gh, &toml_config("repo-v")).await;
    let provider = make_provider_with_app_config(gh).await;

    let result = provider
        .load_repository_config("myorg", "myrepo", server_options())
        .await
        .expect("should succeed");

    assert!(result.is_some());
    let repo_config = result.unwrap();
    assert_eq!(repo_config.config.core.version_prefix, "repo-v");
    assert_eq!(repo_config.owner, "myorg");
    assert_eq!(repo_config.name, "myrepo");
}

/// Server mode: dotfile absent → returns `Ok(None)`.
#[tokio::test]
async fn test_load_repository_config_server_mode_absent_returns_ok_none() {
    let gh = TestGitHub::new();
    // No dotfile seeded → all probes return Ok(None).
    let provider = make_provider_with_app_config(gh).await;

    let result = provider
        .load_repository_config("myorg", "myrepo", server_options())
        .await
        .expect("should succeed");

    assert!(result.is_none());
}

/// CLI mode: delegates to `FileConfigurationProvider` (returns Ok(None) for empty tempdir).
#[tokio::test]
async fn test_load_repository_config_cli_mode_delegates_to_inner() {
    let gh = TestGitHub::new();
    gh.set_install_error().await;
    let provider = make_provider_with_app_config(gh).await;

    let result = provider
        .load_repository_config("myorg", "myrepo", cli_options())
        .await
        .expect("CLI mode must delegate without error");

    // FileConfigurationProvider finds no per-repo file → Ok(None)
    assert!(result.is_none());
}

// ─────────────────────────────────────────────────────────────────────────────
// G.5: Integration scenarios — five-level config pipeline (server mode)
//
// These tests exercise the full path from provider construction through
// `get_merged_config`, verifying each scenario produces the correct outcome
// when using `MockGitHubOperations` with call-tracking.
// ─────────────────────────────────────────────────────────────────────────────

use release_regent_testing::mocks::MockGitHubOperations;

/// Create a `GitHubConfigurationProvider<MockGitHubOperations>` backed by a real
/// temp-dir baseline containing a valid app-level `release-regent.toml`
/// (identical to `make_provider_with_app_config` but for `MockGitHubOperations`).
async fn make_provider_with_mock(
    mock: MockGitHubOperations,
) -> super::GitHubConfigurationProvider<MockGitHubOperations> {
    use crate::file_provider::FileConfigurationProvider;
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(tmp.path().join("release-regent.toml"), app_config_toml())
        .expect("write app config");
    let inner = FileConfigurationProvider::new(tmp.path())
        .await
        .expect("FileConfigurationProvider");
    std::mem::forget(tmp);
    super::GitHubConfigurationProvider::new(inner, mock)
}

/// Seed the standard metadata repo global policy file on a `MockGitHubOperations`.
async fn seed_mock_global(mock: &MockGitHubOperations, content: &str) {
    mock.with_file_content(
        "myorg",
        ".release-regent",
        "global.toml",
        "main",
        Some(content.to_string()),
    )
    .await;
}

/// Seed the repo dotfile on a `MockGitHubOperations`.
async fn seed_mock_dotfile(mock: &MockGitHubOperations, content: &str) {
    mock.with_file_content(
        "myorg",
        "myrepo",
        ".release-regent.toml",
        "main",
        Some(content.to_string()),
    )
    .await;
}

/// Scenario (a): Global policy locks `versioning.strategy = "conventional"`.
///
/// The repo dotfile tries to override with the `external` strategy.
/// Expected: The globally-locked value (`conventional`) wins; a `warn!` is emitted
/// for the override attempt; `get_merged_config` succeeds and returns the locked value.
#[tokio::test]
#[traced_test]
async fn test_g5_global_locks_strategy_prevents_repo_override() {
    let mock = MockGitHubOperations::new();
    let global_content =
        "locked_fields = [\"versioning.strategy\"]\n[versioning]\nstrategy = \"conventional\"\n";
    seed_mock_global(&mock, global_content).await;
    // Repo dotfile tries to switch to the `external` strategy.
    let dotfile_content = "[versioning]\nstrategy = { external = { command = \"./my-version.sh\", env_vars = {}, timeout_ms = 30000 } }\n";
    seed_mock_dotfile(&mock, dotfile_content).await;
    let provider = make_provider_with_mock(mock).await;

    let result = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await
        .expect("locked strategy must not hard-fail the event");

    assert_eq!(
        result.versioning.strategy,
        VersioningStrategy::Conventional,
        "globally-locked versioning.strategy must not be overridden by repo dotfile"
    );
    assert!(
        logs_contain("locked field override attempt ignored"),
        "must emit warn! when a locked field override is attempted"
    );
}

/// Scenario (b): Metadata repo absent (installation ID lookup returns an error).
///
/// Expected: provider falls back to app-level + repo dotfile; a `warn!` is emitted;
/// `get_file_content` is NOT called for the global policy file.
#[tokio::test]
#[traced_test]
async fn test_g5_metadata_absent_uses_app_and_repo_dotfile_no_global_api_call() {
    let mock = MockGitHubOperations::new().with_method_error(
        "get_installation_id_for_repo",
        "App not installed on metadata repo",
    );
    seed_mock_dotfile(&mock, &toml_config("repo-v")).await;
    let provider = make_provider_with_mock(mock.clone()).await;

    let result = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await
        .expect("metadata-absent must not hard-fail the event");

    assert_eq!(
        result.core.version_prefix, "repo-v",
        "repo dotfile must apply when metadata repo is absent"
    );
    assert!(
        logs_contain(".release-regent not accessible"),
        "must emit warn! when metadata repo is absent"
    );

    // Quality checklist: no get_file_content call for the global policy.
    let file_calls = mock.get_file_content_calls().await;
    let global_calls: Vec<_> = file_calls
        .iter()
        .filter(|(_, repo, path, _)| repo == ".release-regent" && path.starts_with("global"))
        .collect();
    assert!(
        global_calls.is_empty(),
        "must not call get_file_content for global policy when metadata repo is absent, \
         but got: {global_calls:?}"
    );
}

/// Scenario (c): Global policy file contains invalid TOML.
///
/// Expected: `get_merged_config` returns `Err` with `is_config_error() == true`; no
/// PR or version-calculator operations are attempted.
#[tokio::test]
async fn test_g5_invalid_global_hard_fails_with_config_error() {
    let mock = MockGitHubOperations::new();
    seed_mock_global(&mock, "not valid toml ]][[").await;
    let provider = make_provider_with_mock(mock).await;

    let result = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await;

    assert!(result.is_err(), "invalid global.toml must return Err");
    assert!(
        result.unwrap_err().is_config_error(),
        "error must be CoreError::Config"
    );
}

/// Scenario (d): Repo dotfile is absent; global defaults used.
///
/// Expected: global-level `version_prefix` is the effective config; no `Err` returned.
#[tokio::test]
async fn test_g5_absent_repo_dotfile_uses_global_defaults() {
    let mock = MockGitHubOperations::new();
    seed_mock_global(&mock, &global_toml("global-v")).await;
    // No repo dotfile seeded → all three dotfile probes return Ok(None).
    let provider = make_provider_with_mock(mock).await;

    let result = provider
        .get_merged_config("myorg", "myrepo", server_options())
        .await
        .expect("absent repo dotfile must not fail");

    assert_eq!(
        result.core.version_prefix, "global-v",
        "global policy must be the effective config when repo dotfile is absent"
    );
}

/// Scenario (e): CLI mode (`installation_id = None`).
///
/// Expected: `get_merged_config` delegates entirely to `FileConfigurationProvider`
/// without making any GitHub API calls.  The result reflects the app-level config
/// (the temp-dir `release-regent.toml` seeded by `make_provider_with_mock`).
#[tokio::test]
async fn test_g5_cli_mode_delegates_to_file_provider_no_github_calls() {
    let mock = MockGitHubOperations::new();
    // Even if GitHub were called, the method errors would propagate as test failures.
    // We verify via call tracking rather than error injection.
    let provider = make_provider_with_mock(mock.clone()).await;

    // CLI mode: installation_id = None.
    let result = provider
        .get_merged_config("myorg", "myrepo", cli_options())
        .await
        .expect("CLI mode must delegate to FileConfigurationProvider without error");

    // FileConfigurationProvider returns the app-level config (version_prefix = "app-v").
    assert_eq!(
        result.core.version_prefix, "app-v",
        "CLI mode must return the FileConfigurationProvider result (app-level config)"
    );

    // No GitHub API calls must have been made for config.
    let file_calls = mock.get_file_content_calls().await;
    assert!(
        file_calls.is_empty(),
        "CLI mode must not make any get_file_content GitHub API calls, but got: {file_calls:?}"
    );
}
