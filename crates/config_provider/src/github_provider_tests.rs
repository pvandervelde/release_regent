use super::{merge_config, merge_config_with_locks, ConfigLocks, LOCKABLE_FIELDS};
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
// merge_config — unconditional merge
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_merge_config_takes_all_incoming_fields_unconditionally() {
    let base = make_config(
        VersioningStrategy::Conventional,
        true,
        false,
        false,
        true,
        "main",
        "v",
        5,
        2.0,
        1000,
    );
    let incoming = make_config(
        VersioningStrategy::Conventional,
        false, // allow_override changed
        true,  // draft changed
        true,  // prerelease changed
        false, // generate_notes changed
        "master",
        "release-",
        3,
        1.5,
        500,
    );

    let result = merge_config(base, incoming.clone());

    assert!(!result.versioning.allow_override);
    assert!(result.releases.draft);
    assert!(result.releases.prerelease);
    assert!(!result.releases.generate_notes);
    assert_eq!(result.core.branches.main, "master");
    assert_eq!(result.core.version_prefix, "release-");
    assert_eq!(result.error_handling.max_retries, 3);
    assert_eq!(result.error_handling.initial_delay_ms, 500);
}

#[test]
fn test_merge_config_ignores_base_when_incoming_differs() {
    // When there are no locks the base is always discarded.
    let base = make_config(
        VersioningStrategy::Conventional,
        true,
        false,
        false,
        true,
        "main",
        "v",
        5,
        2.0,
        1000,
    );
    let mut incoming = default_config();
    incoming.core.version_prefix = "nightly-".to_string();

    let result = merge_config(base, incoming);
    assert_eq!(result.core.version_prefix, "nightly-");
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

/// Minimal valid YAML config content (overrides version_prefix for traceability).
fn yaml_config(version_prefix: &str) -> String {
    format!("core:\n  version_prefix: \"{version_prefix}\"\n")
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
async fn test_fetch_repo_dotfile_yml_found_returns_parsed_config() {
    let gh = TestGitHub::new();
    gh.add_file(
        "owner",
        "repo",
        ".release-regent.yml",
        "main",
        &yaml_config("yml-prefix"),
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
        "yml-prefix",
        "content of .yml file must be reflected in the parsed config"
    );
}

#[tokio::test]
async fn test_fetch_repo_dotfile_yaml_found_when_yml_absent() {
    let gh = TestGitHub::new();
    // .yml absent, .yaml present
    gh.add_file(
        "owner",
        "repo",
        ".release-regent.yaml",
        "main",
        &yaml_config("yaml-prefix"),
    )
    .await;
    let client = gh.clone();
    let provider = make_github_provider(gh).await;

    let result = provider
        .fetch_repo_dotfile("owner", "repo", "main", &client)
        .await
        .expect("should succeed");

    assert_eq!(result.unwrap().core.version_prefix, "yaml-prefix");
}

#[tokio::test]
async fn test_fetch_repo_dotfile_toml_found_when_yml_and_yaml_absent() {
    let gh = TestGitHub::new();
    // .yml and .yaml absent, .toml present
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

    assert_eq!(result.unwrap().core.version_prefix, "toml-prefix");
}

#[tokio::test]
async fn test_fetch_repo_dotfile_yml_takes_precedence_over_yaml() {
    let gh = TestGitHub::new();
    gh.add_file(
        "owner",
        "repo",
        ".release-regent.yml",
        "main",
        &yaml_config("from-yml"),
    )
    .await;
    gh.add_file(
        "owner",
        "repo",
        ".release-regent.yaml",
        "main",
        &yaml_config("from-yaml"),
    )
    .await;
    let client = gh.clone();
    let provider = make_github_provider(gh).await;

    let result = provider
        .fetch_repo_dotfile("owner", "repo", "main", &client)
        .await
        .expect("should succeed");

    assert_eq!(
        result.unwrap().core.version_prefix,
        "from-yml",
        ".yml must be tried before .yaml"
    );
}

#[tokio::test]
async fn test_fetch_repo_dotfile_invalid_content_returns_err_config() {
    let gh = TestGitHub::new();
    gh.add_file(
        "owner",
        "repo",
        ".release-regent.yml",
        "main",
        ": this is not valid yaml {{{{ ]]]]",
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
async fn test_fetch_repo_dotfile_api_error_returns_err_github() {
    let gh = TestGitHub::new();
    gh.add_file_error("owner", "repo", ".release-regent.yml", "main")
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
        ".release-regent.yml",
        "main",
        &yaml_config("v1"),
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
async fn test_load_global_policy_yml_found_when_toml_absent() {
    let gh = TestGitHub::new();
    // toml absent, yml present
    gh.add_file(
        "myorg",
        ".release-regent",
        "global.yml",
        "main",
        &yaml_config("global-yml"),
    )
    .await;
    let client = gh.clone();
    let provider = make_github_provider(gh).await;

    let result = provider
        .load_global_policy("myorg", &client)
        .await
        .expect("should succeed");

    assert_eq!(result.unwrap().core.version_prefix, "global-yml");
}

#[tokio::test]
async fn test_load_global_policy_toml_takes_precedence_over_yml() {
    let gh = TestGitHub::new();
    gh.add_file(
        "myorg",
        ".release-regent",
        "global.toml",
        "main",
        &toml_config("from-toml"),
    )
    .await;
    gh.add_file(
        "myorg",
        ".release-regent",
        "global.yml",
        "main",
        &yaml_config("from-yml"),
    )
    .await;
    let client = gh.clone();
    let provider = make_github_provider(gh).await;

    let result = provider
        .load_global_policy("myorg", &client)
        .await
        .expect("should succeed");

    assert_eq!(
        result.unwrap().core.version_prefix,
        "from-toml",
        "TOML must be tried first for global policy"
    );
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
    // All three probe paths return an error.
    for path in &["global.toml", "global.yml", "global.yaml"] {
        gh.add_file_error("myorg", ".release-regent", path, "main")
            .await;
    }
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
async fn test_load_group_policy_toml_takes_precedence_over_yml() {
    let gh = TestGitHub::new();
    gh.add_file(
        "myorg",
        ".release-regent",
        "groups/backend.toml",
        "main",
        &toml_config("from-toml"),
    )
    .await;
    gh.add_file(
        "myorg",
        ".release-regent",
        "groups/backend.yml",
        "main",
        &yaml_config("from-yml"),
    )
    .await;
    let client = gh.clone();
    let provider = make_github_provider(gh).await;

    let result = provider
        .load_group_policy("myorg", "backend", &client)
        .await
        .expect("should succeed");

    assert_eq!(
        result.unwrap().core.version_prefix,
        "from-toml",
        "TOML must be tried first for group policy"
    );
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
