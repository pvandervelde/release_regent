//! GitHub-backed five-level configuration provider.
//!
//! Implements [`ConfigurationProvider`] for the server deployment model described in
//! ADR-007. Configuration is resolved across five levels, in this merge order:
//!
//! 1. Built-in defaults (`ReleaseRegentConfig::default()`)
//! 2. App-level — local `CONFIG_DIR/release-regent.toml` (via [`FileConfigurationProvider`])
//! 3. Global policy — `{org}/.release-regent/global.toml` (metadata repo, GitHub API)
//! 4. Group policy — `{org}/.release-regent/groups/{group}.toml` (metadata repo, conditional)
//! 5. Repository config — `.release-regent.toml` in target repo root (GitHub API)
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

use crate::{
    file_provider::FileConfigurationProvider, formats::parse_config, validation::ConfigValidator,
};

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
    #[allow(dead_code)] // called by get_merged_config (G.3); verified by unit tests
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

/// Merge `incoming` into `base`, keeping locked fields from `base` unchanged.
///
/// For each of the ten lockable fields (see [`LOCKABLE_FIELDS`]):
///
/// - If the field's path is in `locks` **and** `incoming` carries a value that
///   differs from `base`, the `base` value is retained and a `warn!` is emitted
///   carrying the `path`, `locked_value`, and `override_value`.
/// - Otherwise `incoming`'s value is used.
///
/// The `release_pr` and `notifications` sub-configs (and `group` / `locked_fields`)
/// are never lockable and are always taken from `incoming`.
///
/// # Compile-time contract
///
/// The compile-time assertion on [`LOCKABLE_FIELDS`] ensures that this function's
/// per-field match arms stay in sync with the lockable-fields list. Update the
/// count and every match arm together whenever `LOCKABLE_FIELDS` changes.
///
/// # Spec reference
///
/// `docs/specs/interfaces/github_operations_additions.md` §7
#[must_use]
#[allow(clippy::too_many_lines)] // 10 lockable fields × ~6 lines each; helpers would obscure the contract
fn merge_config_with_locks(
    base: ReleaseRegentConfig,
    incoming: ReleaseRegentConfig,
    locks: &ConfigLocks,
) -> ReleaseRegentConfig {
    use release_regent_core::config::{
        BranchConfig, CoreConfig, ErrorHandlingConfig, ReleasesConfig, VersioningConfig,
    };

    // ── versioning.strategy ────────────────────────────────────────────────
    let versioning_strategy = if locks.is_locked("versioning.strategy")
        && incoming.versioning.strategy != base.versioning.strategy
    {
        warn!(
            path = "versioning.strategy",
            locked_value = ?base.versioning.strategy,
            override_value = ?incoming.versioning.strategy,
            "locked field override attempt ignored"
        );
        base.versioning.strategy
    } else {
        incoming.versioning.strategy
    };

    // ── versioning.allow_override ──────────────────────────────────────────
    let versioning_allow_override = if locks.is_locked("versioning.allow_override")
        && incoming.versioning.allow_override != base.versioning.allow_override
    {
        warn!(
            path = "versioning.allow_override",
            locked_value = base.versioning.allow_override,
            override_value = incoming.versioning.allow_override,
            "locked field override attempt ignored"
        );
        base.versioning.allow_override
    } else {
        incoming.versioning.allow_override
    };

    // ── releases.draft ─────────────────────────────────────────────────────
    let releases_draft =
        if locks.is_locked("releases.draft") && incoming.releases.draft != base.releases.draft {
            warn!(
                path = "releases.draft",
                locked_value = base.releases.draft,
                override_value = incoming.releases.draft,
                "locked field override attempt ignored"
            );
            base.releases.draft
        } else {
            incoming.releases.draft
        };

    // ── releases.prerelease ────────────────────────────────────────────────
    let releases_prerelease = if locks.is_locked("releases.prerelease")
        && incoming.releases.prerelease != base.releases.prerelease
    {
        warn!(
            path = "releases.prerelease",
            locked_value = base.releases.prerelease,
            override_value = incoming.releases.prerelease,
            "locked field override attempt ignored"
        );
        base.releases.prerelease
    } else {
        incoming.releases.prerelease
    };

    // ── releases.generate_notes ────────────────────────────────────────────
    let releases_generate_notes = if locks.is_locked("releases.generate_notes")
        && incoming.releases.generate_notes != base.releases.generate_notes
    {
        warn!(
            path = "releases.generate_notes",
            locked_value = base.releases.generate_notes,
            override_value = incoming.releases.generate_notes,
            "locked field override attempt ignored"
        );
        base.releases.generate_notes
    } else {
        incoming.releases.generate_notes
    };

    // ── core.branches.main ─────────────────────────────────────────────────
    let core_branches_main = if locks.is_locked("core.branches.main")
        && incoming.core.branches.main != base.core.branches.main
    {
        warn!(
            path = "core.branches.main",
            locked_value = %base.core.branches.main,
            override_value = %incoming.core.branches.main,
            "locked field override attempt ignored"
        );
        base.core.branches.main
    } else {
        incoming.core.branches.main
    };

    // ── core.version_prefix ────────────────────────────────────────────────
    let core_version_prefix = if locks.is_locked("core.version_prefix")
        && incoming.core.version_prefix != base.core.version_prefix
    {
        warn!(
            path = "core.version_prefix",
            locked_value = %base.core.version_prefix,
            override_value = %incoming.core.version_prefix,
            "locked field override attempt ignored"
        );
        base.core.version_prefix
    } else {
        incoming.core.version_prefix
    };

    // ── error_handling.max_retries ─────────────────────────────────────────
    let error_max_retries = if locks.is_locked("error_handling.max_retries")
        && incoming.error_handling.max_retries != base.error_handling.max_retries
    {
        warn!(
            path = "error_handling.max_retries",
            locked_value = base.error_handling.max_retries,
            override_value = incoming.error_handling.max_retries,
            "locked field override attempt ignored"
        );
        base.error_handling.max_retries
    } else {
        incoming.error_handling.max_retries
    };

    // ── error_handling.backoff_multiplier ──────────────────────────────────
    // Exact float equality is intentional: values come from parsed config text, so
    // identical literals produce identical bit patterns. A difference means a genuine
    // config change, not a rounding artefact.
    #[allow(clippy::float_cmp)]
    let error_backoff_multiplier = if locks.is_locked("error_handling.backoff_multiplier")
        && incoming.error_handling.backoff_multiplier != base.error_handling.backoff_multiplier
    {
        warn!(
            path = "error_handling.backoff_multiplier",
            locked_value = base.error_handling.backoff_multiplier,
            override_value = incoming.error_handling.backoff_multiplier,
            "locked field override attempt ignored"
        );
        base.error_handling.backoff_multiplier
    } else {
        incoming.error_handling.backoff_multiplier
    };

    // ── error_handling.initial_delay_ms ───────────────────────────────────
    let error_initial_delay_ms = if locks.is_locked("error_handling.initial_delay_ms")
        && incoming.error_handling.initial_delay_ms != base.error_handling.initial_delay_ms
    {
        warn!(
            path = "error_handling.initial_delay_ms",
            locked_value = base.error_handling.initial_delay_ms,
            override_value = incoming.error_handling.initial_delay_ms,
            "locked field override attempt ignored"
        );
        base.error_handling.initial_delay_ms
    } else {
        incoming.error_handling.initial_delay_ms
    };

    ReleaseRegentConfig {
        core: CoreConfig {
            version_prefix: core_version_prefix,
            branches: BranchConfig {
                main: core_branches_main,
            },
        },
        // group and locked_fields are metadata fields handled by get_merged_config;
        // always take from incoming.
        group: incoming.group,
        locked_fields: incoming.locked_fields,
        // release_pr and notifications are never lockable; always take from incoming.
        release_pr: incoming.release_pr,
        notifications: incoming.notifications,
        releases: ReleasesConfig {
            draft: releases_draft,
            prerelease: releases_prerelease,
            generate_notes: releases_generate_notes,
        },
        error_handling: ErrorHandlingConfig {
            max_retries: error_max_retries,
            backoff_multiplier: error_backoff_multiplier,
            initial_delay_ms: error_initial_delay_ms,
        },
        versioning: VersioningConfig {
            strategy: versioning_strategy,
            allow_override: versioning_allow_override,
            // excluded_pr_authors is not lockable; always from incoming.
            excluded_pr_authors: incoming.versioning.excluded_pr_authors,
        },
        // changelog is not lockable; always take from incoming.
        changelog: incoming.changelog,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Cache entry helpers
// ─────────────────────────────────────────────────────────────────────────────

/// TTL for global policy cache entries (10 minutes).
#[allow(dead_code)] // used by load_global_policy (G.2)
const GLOBAL_CACHE_TTL: Duration = Duration::from_secs(600);

/// TTL for group and repository config cache entries (5 minutes).
#[allow(dead_code)] // used by load_group_policy / fetch_repo_dotfile (G.2)
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
/// `None` entries (absent files) are cached exactly like present-file entries so
/// that repeated events in the same TTL window do not re-probe the API.
///
/// A parse or validation error at any level evicts the relevant cache entry so
/// that a corrected file is picked up on the next event.
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
    /// `org` → `cached_at`. When the App is absent or the repo doesn't exist the lookup
    /// result is negative-cached for [`REPO_GROUP_CACHE_TTL`] to suppress repeated API
    /// calls and warn-spam on orgs that never install the App on the metadata repo.
    negative_metadata_installation_cache: RwLock<HashMap<String, Instant>>,
    /// `org` → `(config_or_none, cached_at)`. TTL: [`GLOBAL_CACHE_TTL`].
    /// `None` means the global policy file is absent (valid operational state).
    global_cache: RwLock<HashMap<String, (Option<ReleaseRegentConfig>, Instant)>>,
    /// `"{org}/{group}"` → `(config_or_none, cached_at)`. TTL: [`REPO_GROUP_CACHE_TTL`].
    /// `None` means the group policy file is absent.
    group_cache: RwLock<HashMap<String, (Option<ReleaseRegentConfig>, Instant)>>,
    /// `"{owner}/{repo}"` → `(config_or_none, cached_at)`. TTL: [`REPO_GROUP_CACHE_TTL`].
    /// `None` means the repo dotfile is absent.
    repo_cache: RwLock<HashMap<String, (Option<ReleaseRegentConfig>, Instant)>>,
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
            negative_metadata_installation_cache: RwLock::default(),
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
        // Fast path: permanent positive-cache hit.
        {
            let cache = self.metadata_installation_cache.read().await;
            if let Some(&id) = cache.get(org) {
                return Some(id);
            }
        }

        // Fast path: still-live negative-cache hit — suppress repeated API calls.
        {
            let neg = self.negative_metadata_installation_cache.read().await;
            if let Some(cached_at) = neg.get(org) {
                if cached_at.elapsed() < REPO_GROUP_CACHE_TTL {
                    return None;
                }
            }
        }

        // Slow path: look up via GitHub API.
        match self
            .github
            .get_installation_id_for_repo(org, ".release-regent")
            .await
        {
            Ok(id) => {
                self.metadata_installation_cache
                    .write()
                    .await
                    .insert(org.to_string(), id);
                Some(id)
            }
            Err(e) => {
                warn!(
                    org = %org,
                    error = %e,
                    "metadata repository is not accessible; \
                     falling back to app-level config as effective top"
                );
                self.negative_metadata_installation_cache
                    .write()
                    .await
                    .insert(org.to_string(), Instant::now());
                None
            }
        }
    }

    /// Parse raw TOML content and run schema validation.
    ///
    /// Returns `Err(CoreError::Config)` when parsing fails or when the parsed
    /// config does not satisfy the validator's rules.
    fn parse_and_validate_content(
        &self,
        content: &str,
        source_desc: &str,
    ) -> CoreResult<ReleaseRegentConfig> {
        let config = parse_config(content).map_err(|e| {
            release_regent_core::CoreError::config(format!(
                "Failed to parse config at {source_desc}: {e}"
            ))
        })?;

        let validation = self.validator.validate(&config).map_err(|e| {
            release_regent_core::CoreError::config(format!(
                "Validation error for {source_desc}: {e}"
            ))
        })?;

        if !validation.is_valid {
            return Err(release_regent_core::CoreError::config(format!(
                "Config at {source_desc} is invalid: {}",
                validation.errors.join(", ")
            )));
        }

        Ok(config)
    }

    /// Probe the metadata repository for `global.toml` and return the parsed,
    /// validated policy config.
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
        // Cache check.
        {
            let cache = self.global_cache.read().await;
            if let Some((cached, cached_at)) = cache.get(org) {
                if cached_at.elapsed() < GLOBAL_CACHE_TTL {
                    return Ok(cached.clone());
                }
            }
        }

        // TOML-only probe for the metadata repo.
        // "main" is the conventional default branch for the metadata repository.
        const META_BRANCH: &str = "main";
        const META_REPO: &str = ".release-regent";
        const GLOBAL_PROBE: &str = "global.toml";

        match client
            .get_file_content(org, META_REPO, GLOBAL_PROBE, META_BRANCH)
            .await
        {
            Ok(Some(content)) => {
                let source = format!("{org}/{META_REPO}/{GLOBAL_PROBE}");
                match self.parse_and_validate_content(&content, &source) {
                    Ok(config) => {
                        self.global_cache
                            .write()
                            .await
                            .insert(org.to_string(), (Some(config.clone()), Instant::now()));
                        return Ok(Some(config));
                    }
                    Err(e) => {
                        // Evict on parse/validation error so the next event re-probes.
                        self.global_cache.write().await.remove(org);
                        return Err(e);
                    }
                }
            }
            Ok(None) => {} // file absent
            Err(api_err) => {
                // Transient API failure: propagate; caller decides whether to warn-and-skip
                // or hard-fail.
                return Err(api_err);
            }
        }

        // All candidates absent: cache None and return Ok(None).
        self.global_cache
            .write()
            .await
            .insert(org.to_string(), (None, Instant::now()));
        Ok(None)
    }

    /// Probe the metadata repository for `groups/{group}.toml` and return the
    /// parsed, validated group policy config.
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
        let cache_key = format!("{org}/{group}");

        // Cache check.
        {
            let cache = self.group_cache.read().await;
            if let Some((cached, cached_at)) = cache.get(&cache_key) {
                if cached_at.elapsed() < REPO_GROUP_CACHE_TTL {
                    return Ok(cached.clone());
                }
            }
        }

        const META_BRANCH: &str = "main";
        const META_REPO: &str = ".release-regent";
        let probe_path = format!("groups/{group}.toml");

        match client
            .get_file_content(org, META_REPO, &probe_path, META_BRANCH)
            .await
        {
            Ok(Some(content)) => {
                let source = format!("{org}/{META_REPO}/{probe_path}");
                match self.parse_and_validate_content(&content, &source) {
                    Ok(config) => {
                        self.group_cache
                            .write()
                            .await
                            .insert(cache_key, (Some(config.clone()), Instant::now()));
                        return Ok(Some(config));
                    }
                    Err(e) => {
                        self.group_cache.write().await.remove(&cache_key);
                        return Err(e);
                    }
                }
            }
            Ok(None) => {} // file absent
            Err(api_err) => {
                return Err(api_err);
            }
        }

        // All candidates absent.
        self.group_cache
            .write()
            .await
            .insert(cache_key, (None, Instant::now()));
        Ok(None)
    }

    /// Probe the target repository for `.release-regent.toml` and return the
    /// parsed, validated repository dotfile config.
    ///
    /// Returns `Ok(None)` when no dotfile exists (valid operational state).
    /// Returns `Err(CoreError::Config)` when the file exists but is invalid.
    /// Returns `Err(CoreError::GitHub)` for transient API failures — unlike a missing
    /// file, a transient fetch error hard-fails the event (the repo dotfile is the
    /// primary per-repo config source).
    ///
    /// Respects and populates the 300-second `repo_cache` (key: `"{owner}/{repo}"`).
    ///
    /// # Branch assumption
    ///
    /// The cache key is `"{owner}/{repo}"` without the branch component. This is safe
    /// because all production callers pass `default_branch` from the webhook payload,
    /// so every event for a given repo hits the same branch. A future caller that passes
    /// a non-default branch would receive a stale dotfile if another branch's entry is
    /// already in the cache — document any such caller explicitly if one is ever added.
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
        let cache_key = format!("{owner}/{repo}");

        // Cache check.
        {
            let cache = self.repo_cache.read().await;
            if let Some((cached, cached_at)) = cache.get(&cache_key) {
                if cached_at.elapsed() < REPO_GROUP_CACHE_TTL {
                    return Ok(cached.clone());
                }
            }
        }

        // TOML-only probe for the repo dotfile.
        const DOTFILE_PROBE: &str = ".release-regent.toml";

        match client
            .get_file_content(owner, repo, DOTFILE_PROBE, branch)
            .await
        {
            Ok(Some(content)) => {
                let source = format!("{owner}/{repo}/{DOTFILE_PROBE}@{branch}");
                match self.parse_and_validate_content(&content, &source) {
                    Ok(config) => {
                        self.repo_cache
                            .write()
                            .await
                            .insert(cache_key, (Some(config.clone()), Instant::now()));
                        return Ok(Some(config));
                    }
                    Err(e) => {
                        // Evict on error; hard-fail the event.
                        self.repo_cache.write().await.remove(&cache_key);
                        return Err(e);
                    }
                }
            }
            Ok(None) => {} // dotfile absent
            Err(api_err) => {
                // API errors on the target repo dotfile are always hard failures.
                return Err(api_err);
            }
        }

        // All probes returned None: dotfile absent.
        self.repo_cache
            .write()
            .await
            .insert(cache_key, (None, Instant::now()));
        Ok(None)
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
    /// 5. Repository dotfile (`.release-regent.toml` in target repo)
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
        // CLI / no-GitHub fallback — delegate entirely to the file provider.
        if options.installation_id.is_none() {
            return self.inner.get_merged_config(owner, repo, options).await;
        }

        let installation_id = options.installation_id.unwrap();
        let default_branch = options.default_branch.as_deref().unwrap_or("main");

        // Level 1 + 2: Start from app-level config (local disk, always present).
        // Each config level is a complete parsed document whose unspecified fields carry
        // serde defaults, so the app-level config replaces built-in defaults entirely.
        let mut result = self.inner.load_global_config(options.clone()).await?;
        let mut locks = ConfigLocks::default();

        // Levels 3, 4, 5: Metadata repo path vs. no-metadata fallback.
        match self.resolve_metadata_installation(owner).await {
            Some(meta_id) => {
                let meta = self.github.scoped_to(meta_id);

                // Level 3: Global policy.
                let metadata_reachable = match self.load_global_policy(owner, &meta).await {
                    Ok(Some(mut g)) => {
                        if g.group.is_some() {
                            warn!(
                                org = %owner,
                                group = ?g.group,
                                "global policy file contains a `group` field; \
                                 `group` is only meaningful in repository dotfiles and will be ignored"
                            );
                            g.group = None;
                        }
                        locks.extend_from(&g.locked_fields, "global");
                        result = merge_config_with_locks(result, g, &locks);
                        true
                    }
                    Ok(None) => true,
                    Err(e) if e.is_config_error() => return Err(e),
                    Err(e) => {
                        tracing::warn!(
                            org = %owner,
                            error = %e,
                            "Metadata repo unreachable; skipping global AND group levels"
                        );
                        false
                    }
                };

                // Peek repo dotfile with the meta-scoped client (fetched once, reused at level 5).
                let repo_result = self
                    .fetch_repo_dotfile(owner, repo, default_branch, &meta)
                    .await;

                if let Err(ref e) = repo_result {
                    if !e.is_config_error() {
                        tracing::warn!(
                            repo = %format!("{owner}/{repo}"),
                            error = %e,
                            "Transient API error fetching repo dotfile; \
                             group policy will not be applied"
                        );
                    }
                }

                // Level 4: Group policy (only if metadata repo was reachable).
                if metadata_reachable {
                    let group = repo_result
                        .as_ref()
                        .ok()
                        .and_then(Option::as_ref)
                        .and_then(|c: &ReleaseRegentConfig| c.group.clone());

                    if let Some(ref g) = group {
                        match self.load_group_policy(owner, g, &meta).await {
                            Ok(Some(mut gc)) => {
                                if gc.group.is_some() {
                                    warn!(
                                        org = %owner,
                                        group = ?gc.group,
                                        "group policy file contains a `group` field; \
                                         `group` is only meaningful in repository dotfiles and will be ignored"
                                    );
                                    gc.group = None;
                                }
                                locks.extend_from(&gc.locked_fields, &format!("group:{g}"));
                                result = merge_config_with_locks(result, gc, &locks);
                            }
                            Ok(None) => {
                                tracing::warn!(
                                    org = %owner,
                                    group = %g,
                                    "Group '{g}' declared in {owner}/{repo} but \
                                     no group config found; skipping"
                                );
                            }
                            Err(e) if e.is_config_error() => return Err(e),
                            Err(e) => {
                                tracing::warn!(
                                    group = %g,
                                    error = %e,
                                    "Failed to load group policy; skipping group level"
                                );
                            }
                        }
                    }
                }

                // Level 5: Repo dotfile (reusing the already-fetched result).
                match repo_result? {
                    Some(mut rc) => {
                        if !rc.locked_fields.is_empty() {
                            tracing::warn!(
                                repo = %format!("{owner}/{repo}"),
                                "locked_fields in repository dotfile is ignored"
                            );
                            rc.locked_fields.clear();
                        }
                        result = merge_config_with_locks(result, rc, &locks);
                    }
                    None => {}
                }
            }
            None => {
                // Metadata repo absent: use only app-level + repo dotfile.
                warn!(
                    org = %owner,
                    "Metadata repo {owner}/.release-regent not accessible; \
                     using app-level as baseline"
                );
                let scoped = self.github.scoped_to(installation_id);
                if let Some(rc) = self
                    .fetch_repo_dotfile(owner, repo, default_branch, &scoped)
                    .await?
                {
                    result = merge_config_with_locks(result, rc, &locks);
                }
            }
        }

        // group and locked_fields are pipeline-only metadata fields; strip them from
        // the final result so callers never observe values sourced from policy files.
        result.group = None;
        result.locked_fields = Vec::new();

        Ok(result)
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
        if options.installation_id.is_none() {
            return self
                .inner
                .load_repository_config(owner, repo, options)
                .await;
        }

        let installation_id = options.installation_id.unwrap();
        let default_branch = options.default_branch.as_deref().unwrap_or("main");
        let client = self.github.scoped_to(installation_id);

        match self
            .fetch_repo_dotfile(owner, repo, default_branch, &client)
            .await?
        {
            Some(config) => Ok(Some(RepositoryConfig {
                config,
                name: repo.to_string(),
                owner: owner.to_string(),
            })),
            None => Ok(None),
        }
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

#[cfg(test)]
#[path = "github_provider_tests.rs"]
mod tests;
