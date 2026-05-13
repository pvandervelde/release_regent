# ADR-007: Enterprise Configuration Hierarchy with Metadata Repository

Status: Accepted
Date: 2026-05-09
Owners: ReleaseRegent team

## Context

Release Regent needs a flexible configuration system that supports both simple single-repo
deployments and large enterprise deployments. A naive two-level model (app-level local
config → repo dotfile) is insufficient for enterprise deployments where:

- A platform team needs to enforce organisation-wide policies (e.g. versioning strategy,
  release controls) that individual repository owners cannot override.
- Policies need to apply to **groups** of related repositories (e.g. all backend services,
  all mobile apps) without requiring each repository to duplicate the same configuration.
- Configuration governance must follow the same git-based audit trail as code changes,
  not live in a deployment container.
- A single Release Regent instance serves hundreds of repositories across an organisation,
  requiring centralised policy management with selective override permissions.

A two-level model (app-level local config → repo dotfile) does not support policy
enforcement or group-level defaults.

## Decision

Extend the configuration hierarchy to five levels, resolved in this order (each level
merges over the previous for unlocked fields only):

```
Built-in defaults
      ↓
App-level config     CONFIG_DIR/release-regent.toml  (local disk — bootstrap + fallback)
      ↓
Global policy        {org}/.release-regent/global.toml  (metadata repo)
      ↓
Group policy         {org}/.release-regent/groups/{group}.toml  (metadata repo)
      ↓
Repository config    {repo-root}/.release-regent.toml  (target repository)
```

### Metadata repository

All server-side levels above the repository dotfile are stored in a **metadata
repository** named `.release-regent` within the same GitHub organisation (or user
account). This repository is auto-discovered: for an event from `myorg/some-repo`, the
metadata repository is `myorg/.release-regent`.

```
myorg/.release-regent/
  global.toml          ← org-wide policy defaults and field locks
  groups/
    platform.toml      ← policy for repos that declare group = "platform"
    mobile.toml
    data.toml
```

The GitHub App must be installed on the metadata repository for global and group levels
to be active. If the App is not installed (or the repository does not exist), the system
silently falls back to the app-level config as the top of the hierarchy.

### Group membership

Repositories self-declare their group in the repo dotfile via a top-level `group` field:

```toml
# .release-regent.toml in myorg/platform-api
group = "platform"

[release_pr]
title_template = "chore(release): ${version} [platform-api]"
```

The `group` field is added to `ReleaseRegentConfig`. It is only meaningful in repository
dotfiles; if present in global or group config files it is ignored with a `warn!`.

### Per-field locks

Global and group config files may include a `locked_fields` array. Each entry is a
dotted field path. Lower levels cannot override locked fields.

```toml
# global.toml
locked_fields = ["versioning.strategy", "versioning.allow_override"]

[versioning]
strategy = "conventional"
allow_override = false
```

```toml
# groups/platform.toml
# Adds to global's locks; cannot remove globally-locked fields
locked_fields = ["releases.draft"]

[releases]
draft = true
```

**Lockable fields** (only these are valid in `locked_fields`):

| Path | Description |
|---|---|
| `versioning.strategy` | Versioning algorithm (conventional / external) |
| `versioning.allow_override` | Whether PR comment override commands are permitted |
| `releases.draft` | Whether GitHub releases are created as drafts |
| `releases.prerelease` | Whether GitHub releases are marked pre-release |
| `releases.generate_notes` | Whether GitHub auto-generates release notes |
| `core.branches.main` | Name of the default/main branch |
| `core.version_prefix` | Prefix prepended to version tags |
| `error_handling.max_retries` | Maximum retry count |
| `error_handling.backoff_multiplier` | Exponential backoff multiplier |
| `error_handling.initial_delay_ms` | Initial retry delay |

**Never lockable** — if listed in `locked_fields`, silently ignored with `warn!`:

- All `release_pr.*` fields (title template, body template, manifest files, auto-detect)
- All `notifications.*` fields

`locked_fields` in repository dotfiles is silently ignored with `warn!`.

### Lock accumulation rule

Locks accumulate downward through the hierarchy; they can never be removed by a lower
level. If a group config attempts to include in its `locked_fields` a field that global
already locked, the entry is a no-op (the field stays locked) and a `warn!` is emitted.
Repository dotfiles cannot modify locks in any way.

### Lock conflict handling

When a lower-level config specifies a value for a field that is locked by a higher level,
the provider **silently uses the locked value** and emits a `warn!`. The event is **not**
failed. This prevents immediate workflow disruption for repositories that have a
pre-existing dotfile when a new lock is added centrally — they can clean up the dotfile
at their own pace.

### Fallback behaviour

| Condition | Behaviour |
|---|---|
| App not installed on metadata repo / metadata repo absent | Fall back: skip global and group levels; emit `warn!`; app-level is the effective top |
| Metadata repo accessible but `global.toml` absent | Skip global level; continue |
| Metadata repo accessible, `global.toml` present but invalid | **Hard fail** all events for this org |
| Group declared in repo dotfile, group file absent in metadata repo | Skip group level; emit `warn!`; continue |
| Group file present but invalid | **Hard fail** events for repos in that group |
| Repository dotfile absent | Skip repo level; use merged result so far |
| Repository dotfile present but invalid | **Hard fail** that event only |
| Repository dotfile API error (503, network timeout) | **Hard fail** that event only — unlike a transient error on the metadata repo, the repo dotfile is the primary per-repository config source; transient failures are not silently skipped |

The asymmetry between "absent = skip silently" and "present but invalid = hard fail" is
intentional: absence is a valid operational state (not yet configured), while an invalid
file is a configuration error that must be explicitly corrected.

### Caching

Three independent in-memory caches with different TTLs:

| Level | Cache key | TTL |
|---|---|---|
| Global policy | `{org}` | 600 seconds (10 min) |
| Group policy | `{org}/{group}` | 300 seconds (5 min) |
| Repository config | `{owner}/{repo}` | 300 seconds (5 min) |

The metadata repo installation ID is cached permanently for the process lifetime
(installation IDs are stable for the lifetime of a GitHub App installation).

A parse or validation error at any level evicts the cache entry for that level so that a
corrected file is picked up on the next event.

## Consequences

**Enables:**

- Platform teams enforce non-overridable policies (versioning strategy, release controls)
  centrally, without touching individual repository config files.
- Group-level defaults eliminate per-repository boilerplate for common settings (draft
  releases for incubation services, specific branch naming, etc.).
- All configuration changes at every level are under git-based change management and
  audit within the metadata repository.
- Compatible with single-repository or simple deployments — metadata repository is
  optional; app-level config is the fallback.
- Backwards compatible: existing deployments with only a local `CONFIG_DIR/release-regent.toml`
  continue to work—older YAML-format deployments must migrate to TOML.

**Forbids:**

- Lower configuration levels from overriding locked policy fields.
- Locking template and notification fields (always user-customisable at repo level).
- Repository dotfiles from specifying `locked_fields` (ignored with `warn!`).
- Release Regent from validating the authorship of changes to the metadata repository —
  this is delegated to branch protection rules and CODEOWNERS on the metadata repository.

**Trade-offs:**

- Webhook events that trigger config loading may now make up to three additional GitHub
  API calls (global, group, repo dotfile) before the main operation begins. Caching
  reduces this to zero for the common case within a TTL window.
- An invalid `global.toml` in the metadata repository fails **all** events for that
  organisation — high blast radius but deliberate, as policy errors must be fixed promptly.
- The metadata repository naming convention (`{org}/.release-regent`) is fixed; custom
  names are not supported.

## Alternatives considered

### Option A: Mapping file in metadata repo for group membership

A `teams/members.toml` would centrally map repositories to groups. **Why not**: Requires
a central update for every new repository; doesn't scale and violates the self-describing
repository principle.

### Option B: GitHub Teams API for group membership

Map GitHub Team membership to config groups. **Why not**: GitHub Teams are designed for
access control, not config inheritance. One repo can belong to multiple teams (ambiguous
group resolution). Requires additional `teams:read` scope. Fragile.

### Option C: Separate metadata repository per group

Each group operates its own metadata repository. **Why not**: No single authoritative place
for org-wide policy. Installation management complexity grows with the number of groups.

### Option D: Environment variable injection per group or org

Pass group/org policy via env vars at server startup. **Why not**: Doesn't scale past a
handful of settings. Not auditable. Requires a server restart for every policy change.

## Implementation notes

### New fields on `ReleaseRegentConfig`

```rust
/// Repository group name. Only meaningful in repository dotfiles.
/// If present in global or group config files, ignored with warn!.
/// When set, the provider fetches {org}/.release-regent/groups/{group}.toml.
#[serde(default)]
pub group: Option<String>,

/// Fields that cannot be overridden at lower configuration levels.
/// Only valid in global policy and group policy files.
/// If present in repository dotfiles, ignored with warn!.
///
/// Valid lockable paths: see [ADR-007] for the complete list.
#[serde(default)]
pub locked_fields: Vec<String>,
```

### Merge algorithm (pseudocode inside `get_merged_config`)

```
result ← ReleaseRegentConfig::default()
locks  ← HashSet::new()

// 1. App-level (local disk, always required)
app_config ← FileConfigurationProvider::load_global_config()
result ← merge(result, app_config)   // no locks applied yet

// 2. Metadata repo levels
metadata_installation ← resolve_metadata_installation(org)
if metadata_installation is Some(id):
    scoped ← github.scoped_to(id)

    // 2a. Global policy
    metadata_reachable ← true
    global_raw ← fetch_file(scoped, org, ".release-regent", "global.toml")
    if global_raw is Ok(Some(content)):
        global_config ← parse_and_validate(content)   // Err → hard fail
        locks.extend(validate_lockable(global_config.locked_fields))
        result ← merge_with_locks(result, global_config, locks)
    // if Ok(None): global absent; metadata reachable; continue to group level
    if global_raw is Err(API):
        warn!("Metadata repo unreachable; skipping global AND group levels")
        metadata_reachable ← false

    // 2b. Group policy (need group name from repo dotfile peek)
    // Only attempted when metadata repo was reachable (global returned Ok)
    if metadata_reachable:
        group_name ← peek_group_field(owner, repo, branch, scoped)  // may return None
        if group_name is Some(name):
            group_raw ← fetch_file(scoped, org, ".release-regent", "groups/{name}.toml")
            if group_raw is Ok(Some(content)):
                group_config ← parse_and_validate(content)  // Err → hard fail
                new_locks ← validate_lockable(group_config.locked_fields)
                // Locks already in `locks` remain; new_locks adds more
                locks.extend(new_locks)
                result ← merge_with_locks(result, group_config, locks)
            if group_raw is Ok(None):
                warn!("Group '{name}' declared in {owner}/{repo} but no group config found")
                // skip group level, continue

else:
    warn!("Metadata repo {org}/.release-regent not accessible; using app-level as baseline")

// 3. Repository dotfile
repo_config ← fetch_repo_dotfile(owner, repo, branch)
if repo_config is Ok(Some(config)):
    if config.locked_fields is not empty: warn! and clear locked_fields
    result ← merge_with_locks(result, config, locks)
if repo_config is Ok(None): skip
if repo_config is Err: hard fail

return result
```

### `merge_with_locks` semantics

For each field in `incoming`:

- If the field's dotted path is in `locks` and `incoming.field ≠ result.field`:
  emit `warn!("Field {path} is locked by higher-level policy; ignoring override from {level}")`;
  keep `result.field` (the locked value).
- Otherwise: `result.field ← incoming.field`.

### Group field peek

To resolve the group name before applying the group policy, the provider fetches the repo
dotfile once early in the merge pipeline, extracts only the `group` field, then applies
the full repo dotfile last. The fetched content is cached so it is not downloaded twice.

### Server environment variable changes

No new environment variables are required. The metadata repository is auto-discovered
from each webhook event's `repository.owner` field.

### Lockable field validation

Non-lockable fields (templates, notifications) found in `locked_fields` are silently
dropped after a `warn!`. The provider must maintain a static allowlist of lockable paths
and filter `locked_fields` against it before adding to the accumulated lock set.

## References

- [FR-6: Configuration Management](../specs/requirements/functional-requirements.md)
- [US-6, US-9: Configuration Management](../specs/requirements/user-stories.md)
- [Interface spec — GitHubConfigurationProvider](../specs/interfaces/github_operations_additions.md)
