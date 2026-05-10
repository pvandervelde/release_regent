//! GitHub-backed five-level configuration provider.
//!
//! Implements [`ConfigurationProvider`] for the server deployment model described in
//! ADR-007. Configuration is resolved across five levels, in this merge order:
//!
//! 1. Built-in defaults (`ReleaseRegentConfig::default()`)
//! 2. App-level — local `CONFIG_DIR/release-regent.yml` (via [`FileConfigurationProvider`])
//! 3. Global policy — `{org}/.release-regent/global.toml` (metadata repo, GitHub API)
//! 4. Group policy — `{org}/.release-regent/groups/{group}.toml` (metadata repo, conditional)
//! 5. Repository config — `.release-regent.yml` in target repo root (GitHub API)
//!
//! Global and group policy files may lock specific fields so that lower levels cannot
//! override them. See [`ConfigLocks`] and [`LOCKABLE_FIELDS`] for lock semantics, and
//! [`merge_config_with_locks`] for the per-field enforcement contract.
//!
//! # Spec reference
//!
//! `docs/specs/interfaces/github_operations_additions.md` — second section
//! (`GitHubConfigurationProvider`)
//!
//! # ADR
//!
//! `docs/adr/ADR-007-enterprise-config-hierarchy.md`

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::warn;

use release_regent_core::{
    config::ReleaseRegentConfig,
    traits::{
        configuration_provider::{
            ConfigurationSource, LoadOptions, RepositoryConfig, ValidationResult,
        },
        ConfigurationProvider, GitHubOperations,
    },
    CoreResult,
};

use crate::{file_provider::FileConfigurationProvider, validation::ConfigValidator};

// ─────────────────────────────────────────────────────────────────────────────
// Lockable field registry
// ─────────────────────────────────────────────────────────────────────────────

/// Dotted field paths that may be locked by global or group policy files.
///
/// Non-policy paths (e.g. `release_pr.*`, `notifications.*`) are **never** lockable.
/// If they appear in a `locked_fields` list they are silently dropped with `warn!`.
///
/// # ADR reference
///
/// See the lockable-fields table in ADR-007's "Per-field locks" section.
pub const LOCKABLE_FIELDS: &[&str] = &[
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

// Compile-time guard: update this count and `merge_config_with_locks` together.
const _: () = assert!(
    LOCKABLE_FIELDS.len() == 10,
    "Update merge_config_with_locks field checks when adding or removing lockable fields",
);

// ─────────────────────────────────────────────────────────────────────────────
// ConfigLocks — accumulated lock set through the merge pipeline
// ─────────────────────────────────────────────────────────────────────────────

/// Accumulated set of locked field paths built up during the config merge pipeline.
///
/// Locks flow strictly downward: once a field is locked by a higher level it cannot
/// be unlocked by any lower level.
#[derive(Debug, Default)]
struct ConfigLocks {
    locked: HashSet<String>,
}

impl ConfigLocks {
    /// Extend the lock set from a `locked_fields` list read from a policy file.
    ///
    /// Non-lockable paths are dropped with a `warn!`.
    /// Paths that are already locked are a no-op with a `warn!` (duplicate entry).
    fn extend_from(&mut self, locked_fields: &[String], source_level: &str) {
        for path in locked_fields {
            if !LOCKABLE_FIELDS.contains(&path.as_str()) {
                warn!(
                    path = %path,
                    level = %source_level,
                    "locked_fields entry is not a lockable policy field; ignoring"
                );
                continue;
            }
            if self.locked.contains(path.as_str()) {
                warn!(
                    path = %path,
                    level = %source_level,
                    "field already locked by higher level; ignoring duplicate lock entry"
                );
            } else {
                self.locked.insert(path.clone());
            }
        }
    }

    fn is_locked(&self, path: &str) -> bool {
        self.locked.contains(path)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Merge helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Unconditionally merge `incoming` over `base` (no lock checks).
///
/// Used for the app-level baseline where no locks are active yet.
///
/// # Spec reference
///
/// `docs/specs/interfaces/github_operations_additions.md` §7
pub fn merge_config(
    base: ReleaseRegentConfig,
    incoming: ReleaseRegentConfig,
) -> ReleaseRegentConfig {
    unimplemented!("See docs/specs/interfaces/github_operations_additions.md §7 (merge_config)");
    // Suppress unused-variable warnings in stub
    #[allow(clippy::drop_non_drop)]
    let _ = (base, incoming);
}

/// Merge `incoming` into `base`, keeping locked fields from `base` unchanged.
///
/// For each locked field where `incoming` differs from `base`:
/// - The `base` (locked) value is kept.
/// - A `warn!` is emitted identifying the field path, locked value, and override value.
///
/// Template (`release_pr.*`) and notification (`notifications.*`) fields are always
/// taken from `incoming` because they are never lockable.
///
/// # Compile-time contract
///
/// The compile-time assertion on [`LOCKABLE_FIELDS`] ensures that this function's
/// per-field checks stay in sync with the lockable-fields list. Update the count and
/// the match arms together whenever `LOCKABLE_FIELDS` changes.
///
/// # Spec reference
///
/// `docs/specs/interfaces/github_operations_additions.md` §7
pub fn merge_config_with_locks(
    base: ReleaseRegentConfig,
    incoming: ReleaseRegentConfig,
    locks: &ConfigLocks,
) -> ReleaseRegentConfig {
    unimplemented!(
        "See docs/specs/interfaces/github_operations_additions.md §7 (merge_config_with_locks)"
    );
    // Suppress unused-variable warnings in stub
    #[allow(clippy::drop_non_drop)]
    let _ = (base, incoming, locks);
}

// ─────────────────────────────────────────────────────────────────────────────
// Cache entry helpers
// ─────────────────────────────────────────────────────────────────────────────

/// TTL for global policy cache entries (10 minutes).
const GLOBAL_CACHE_TTL: Duration = Duration::from_secs(600);

/// TTL for group and repository config cache entries (5 minutes).
const REPO_GROUP_CACHE_TTL: Duration = Duration::from_secs(300);

// ─────────────────────────────────────────────────────────────────────────────
// GitHubConfigurationProvider
// ─────────────────────────────────────────────────────────────────────────────

/// Five-level configuration provider backed by a GitHub metadata repository.
///
/// Resolves configuration for a target repository across five merge levels (see module
/// doc for the full merge order). Levels 3–5 require GitHub API access; level 2 (app-
/// level baseline) is always read from local disk via the inner
/// [`FileConfigurationProvider`].
///
/// # Caching
///
/// Four independent in-memory caches reduce GitHub API calls on hot paths:
///
/// | Cache | Key | TTL |
/// |-------|-----|-----|
/// | `metadata_installation_cache` | `org` | permanent |
/// | `global_cache` | `org` | 600 s |
/// | `group_cache` | `"{org}/{group}"` | 300 s |
/// | `repo_cache` | `"{owner}/{repo}"` | 300 s |
///
/// A parse or validation error at any level evicts the relevant cache entry immediately
/// so that a corrected file is picked up on the next event.
///
/// # Spec reference
///
/// `docs/specs/interfaces/github_operations_additions.md` §4
pub struct GitHubConfigurationProvider<G>
where
    G: GitHubOperations + Clone + Send + Sync,
{
    /// App-level baseline provider; also used as direct delegate in CLI mode.
    inner: FileConfigurationProvider,
    /// Unscoped GitHub client. Each level-specific call uses `scoped_to(id)`.
    github: G,
    /// `org` → installation ID for `{org}/.release-regent`. Never evicted.
    metadata_installation_cache: RwLock<HashMap<String, u64>>,
    /// `org` → `(config, cached_at)`. TTL: [`GLOBAL_CACHE_TTL`].
    global_cache: RwLock<HashMap<String, (ReleaseRegentConfig, Instant)>>,
    /// `"{org}/{group}"` → `(config, cached_at)`. TTL: [`REPO_GROUP_CACHE_TTL`].
    group_cache: RwLock<HashMap<String, (ReleaseRegentConfig, Instant)>>,
    /// `"{owner}/{repo}"` → `(config, cached_at)`. TTL: [`REPO_GROUP_CACHE_TTL`].
    repo_cache: RwLock<HashMap<String, (ReleaseRegentConfig, Instant)>>,
    /// Schema validator for each parsed config level.
    validator: ConfigValidator,
}

impl<G: GitHubOperations + Clone + Send + Sync> GitHubConfigurationProvider<G> {
    /// Create a new provider wrapping a local-file baseline and a GitHub client.
    ///
    /// All four caches start empty; no network calls are made at construction time.
    ///
    /// # Parameters
    ///
    /// - `inner` — local-disk provider used for the app-level baseline (level 2) and as
    ///   the complete fallback in CLI mode (`installation_id = None`).
    /// - `github` — unscoped GitHub client. For each event the provider creates a scoped
    ///   clone via [`GitHubOperations::scoped_to`].
    pub fn new(inner: FileConfigurationProvider, github: G) -> Self {
        Self {
            inner,
            github,
            metadata_installation_cache: RwLock::default(),
            global_cache: RwLock::default(),
            group_cache: RwLock::default(),
            repo_cache: RwLock::default(),
            validator: ConfigValidator::new(),
        }
    }

    /// Look up (and permanently cache) the GitHub App installation ID for the metadata
    /// repository `{org}/.release-regent`.
    ///
    /// Returns `None` when the App is not installed on the metadata repo or when the
    /// metadata repo does not exist. This is a valid operational state: the system falls
    /// back to app-level config as the effective top. A `warn!` is emitted on first miss.
    ///
    /// # Spec reference
    ///
    /// `docs/specs/interfaces/github_operations_additions.md` §6 — `resolve_metadata_installation`
    async fn resolve_metadata_installation(&self, org: &str) -> Option<u64> {
        unimplemented!(
            "See docs/specs/interfaces/github_operations_additions.md §6 \
             (resolve_metadata_installation)"
        );
        #[allow(clippy::drop_non_drop)]
        let _ = org;
    }

    /// Probe the metadata repository for `global.toml` (then `.yml`, then `.yaml`) and
    /// return the parsed, validated policy config.
    ///
    /// Returns `Ok(None)` when no global policy file exists (absent is not an error).
    /// Returns `Err(CoreError::Config)` when the file exists but is invalid.
    /// Returns `Err(CoreError::GitHub)` for transient API failures.
    ///
    /// Respects and populates the 600-second `global_cache`. A parse/validation error
    /// evicts any existing cache entry so that a corrected file is picked up next time.
    ///
    /// # Spec reference
    ///
    /// `docs/specs/interfaces/github_operations_additions.md` §6 — `load_global_policy`
    async fn load_global_policy(
        &self,
        org: &str,
        client: &G,
    ) -> CoreResult<Option<ReleaseRegentConfig>> {
        unimplemented!(
            "See docs/specs/interfaces/github_operations_additions.md §6 (load_global_policy)"
        );
        #[allow(clippy::drop_non_drop)]
        let _ = (org, client);
    }

    /// Probe the metadata repository for `groups/{group}.toml` (then `.yml`, `.yaml`) and
    /// return the parsed, validated group policy config.
    ///
    /// Returns `Ok(None)` when no group policy file exists (absent is not an error).
    /// Returns `Err(CoreError::Config)` when the file exists but cannot be parsed or
    /// validated. Returns `Err(CoreError::GitHub)` for transient API failures.
    ///
    /// Respects and populates the 300-second `group_cache` (key: `"{org}/{group}"`).
    ///
    /// # Spec reference
    ///
    /// `docs/specs/interfaces/github_operations_additions.md` §6 — `load_group_policy`
    async fn load_group_policy(
        &self,
        org: &str,
        group: &str,
        client: &G,
    ) -> CoreResult<Option<ReleaseRegentConfig>> {
        unimplemented!(
            "See docs/specs/interfaces/github_operations_additions.md §6 (load_group_policy)"
        );
        #[allow(clippy::drop_non_drop)]
        let _ = (org, group, client);
    }

    /// Probe the target repository for `.release-regent.yml` (then `.yaml`, then `.toml`)
    /// and return the parsed, validated repository dotfile config.
    ///
    /// Probe order is YAML-first (`.yml` → `.yaml` → `.toml`) to match existing
    /// installations. This is intentionally asymmetric with the metadata repo helpers
    /// which probe TOML-first; see the spec note on probe-order asymmetry.
    ///
    /// Returns `Ok(None)` when no dotfile exists (valid operational state).
    /// Returns `Err(CoreError::Config)` when the file exists but is invalid.
    /// Returns `Err(CoreError::GitHub)` for transient API failures — unlike a missing
    /// file, a transient fetch error hard-fails the event (the repo dotfile is the
    /// primary per-repo config source).
    ///
    /// Respects and populates the 300-second `repo_cache` (key: `"{owner}/{repo}"`).
    ///
    /// # Spec reference
    ///
    /// `docs/specs/interfaces/github_operations_additions.md` §6 — `fetch_repo_dotfile`
    async fn fetch_repo_dotfile(
        &self,
        owner: &str,
        repo: &str,
        branch: &str,
        client: &G,
    ) -> CoreResult<Option<ReleaseRegentConfig>> {
        unimplemented!(
            "See docs/specs/interfaces/github_operations_additions.md §6 (fetch_repo_dotfile)"
        );
        #[allow(clippy::drop_non_drop)]
        let _ = (owner, repo, branch, client);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ConfigurationProvider implementation
// ─────────────────────────────────────────────────────────────────────────────

#[async_trait]
impl<G> ConfigurationProvider for GitHubConfigurationProvider<G>
where
    G: GitHubOperations + Clone + Send + Sync,
{
    /// Resolve the effective configuration for a repository using the full five-level
    /// merge pipeline.
    ///
    /// When `options.installation_id` is `None` (CLI mode) the call is delegated
    /// entirely to the inner [`FileConfigurationProvider`] and no GitHub API calls are
    /// made.
    ///
    /// # Merge order
    ///
    /// 1. Built-in defaults
    /// 2. App-level (local disk — always present)
    /// 3. Global policy (`{org}/.release-regent/global.toml`)
    /// 4. Group policy (`{org}/.release-regent/groups/{group}.toml`)
    /// 5. Repository dotfile (`.release-regent.yml` in target repo)
    ///
    /// Each level may only override unlocked fields. Per-field lock semantics are
    /// documented in [`merge_config_with_locks`] and ADR-007.
    ///
    /// # Errors
    ///
    /// - `CoreError::Config` — a policy or dotfile exists but is invalid (hard fail)
    /// - `CoreError::GitHub` — unrecoverable API failure on the target repo dotfile
    ///
    /// Transient API failures on the metadata repo levels produce `warn!` entries and
    /// those levels are skipped rather than hard-failing the event.
    ///
    /// # Spec reference
    ///
    /// `docs/specs/interfaces/github_operations_additions.md` §5
    #[allow(clippy::too_many_lines)] // Five-level pipeline is inherently long; sub-helpers keep it manageable
    async fn get_merged_config(
        &self,
        owner: &str,
        repo: &str,
        options: LoadOptions,
    ) -> CoreResult<ReleaseRegentConfig> {
        unimplemented!(
            "See docs/specs/interfaces/github_operations_additions.md §5 (get_merged_config)"
        );
        #[allow(clippy::drop_non_drop)]
        let _ = (owner, repo, options);
    }

    /// Load configuration for a specific repository.
    ///
    /// In server mode (`options.installation_id = Some(_)`) this fetches only the
    /// repository dotfile level (no global/group merge). Delegates to
    /// [`fetch_repo_dotfile`] and wraps the result in [`RepositoryConfig`].
    ///
    /// In CLI mode (`installation_id = None`) delegates entirely to
    /// [`FileConfigurationProvider::load_repository_config`].
    ///
    /// # Spec reference
    ///
    /// `docs/specs/interfaces/github_operations_additions.md` §5
    async fn load_repository_config(
        &self,
        owner: &str,
        repo: &str,
        options: LoadOptions,
    ) -> CoreResult<Option<RepositoryConfig>> {
        unimplemented!(
            "See docs/specs/interfaces/github_operations_additions.md §5 (load_repository_config)"
        );
        #[allow(clippy::drop_non_drop)]
        let _ = (owner, repo, options);
    }

    /// Load the app-level global config from local disk.
    ///
    /// Always delegates to the inner [`FileConfigurationProvider`]; this level
    /// does not interact with GitHub.
    async fn load_global_config(&self, options: LoadOptions) -> CoreResult<ReleaseRegentConfig> {
        self.inner.load_global_config(options).await
    }

    async fn validate_config(&self, config: &ReleaseRegentConfig) -> CoreResult<ValidationResult> {
        self.inner.validate_config(config).await
    }

    async fn save_config(
        &self,
        config: &ReleaseRegentConfig,
        owner: Option<&str>,
        repo: Option<&str>,
        global: bool,
    ) -> CoreResult<()> {
        self.inner.save_config(config, owner, repo, global).await
    }

    async fn list_repository_configs(
        &self,
        options: LoadOptions,
    ) -> CoreResult<Vec<RepositoryConfig>> {
        self.inner.list_repository_configs(options).await
    }

    async fn get_config_source(
        &self,
        owner: Option<&str>,
        repo: Option<&str>,
    ) -> CoreResult<ConfigurationSource> {
        self.inner.get_config_source(owner, repo).await
    }

    async fn reload_config(&self, owner: Option<&str>, repo: Option<&str>) -> CoreResult<()> {
        self.inner.reload_config(owner, repo).await
    }

    async fn config_exists(&self, owner: Option<&str>, repo: Option<&str>) -> CoreResult<bool> {
        self.inner.config_exists(owner, repo).await
    }

    fn supported_formats(&self) -> Vec<String> {
        self.inner.supported_formats()
    }

    async fn get_default_config(&self) -> CoreResult<ReleaseRegentConfig> {
        self.inner.get_default_config().await
    }
}
