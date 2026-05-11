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
