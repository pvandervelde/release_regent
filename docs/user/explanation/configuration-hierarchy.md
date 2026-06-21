---
title: Configuration hierarchy
description: How Release Regent merges five configuration levels, how the per-org metadata
  repository works, and how platform teams can enforce organisation-wide policy
---

# Configuration hierarchy

Release Regent resolves configuration by merging up to five sources in a fixed order. Each
level can override unlocked fields from the level above. Understanding the hierarchy helps you
decide where to place a setting and how policy enforcement works across a large organisation.

## The five levels

```
Built-in defaults
      ↓
App-level config         CONFIG_DIR/release-regent.toml  (local disk)
      ↓
Global policy            {org}/.release-regent/global.toml  (metadata repo)
      ↓
Group policy             {org}/.release-regent/groups/{group}.toml  (metadata repo)
      ↓
Repository config        .release-regent.toml  (target repository root)
```

| Level | Source | Required? |
| :--- | :--- | :--- |
| Built-in defaults | Hard-coded in the binary | Always present |
| App-level | `CONFIG_DIR/release-regent.toml` on local disk | Required |
| Global policy | `{org}/.release-regent/global.toml` — metadata repository | Optional |
| Group policy | `{org}/.release-regent/groups/{group}.toml` — metadata repository | Optional |
| Repository config | `.release-regent.toml` in target repository root | Optional |

A missing level is silently skipped. A present but invalid level (parse or schema error) is a
hard failure for that event.

### Built-in defaults

`ReleaseRegentConfig::default()` provides sensible baseline values for every field. You never
need to configure these explicitly unless you want to change them.

### App-level config

The operator places a `release-regent.toml` file in `CONFIG_DIR` (set via environment
variable). This is the only level that is always active and serves as both the bootstrap
configuration for the server process and the fallback if no metadata repository exists.

### Global policy (metadata repository)

An org-wide `global.toml` stored in the metadata repository (see below) provides defaults
and policy enforcement that apply to every repository in the organisation. Platform teams
use this level to set non-negotiable settings and field locks without modifying individual
repository dotfiles.

### Group policy (metadata repository)

A group config file in the metadata repository applies defaults and locks to a subset of
repositories that self-declare membership by setting `group = "name"` in their dotfile. This
is useful for common settings that apply to a class of repositories (for example, all backend
services or all mobile apps) without requiring each repository to duplicate the same config.

### Repository config

The `.release-regent.toml` file at the root of each repository is the per-repo level.
Repository owners control all unlocked fields here. See the
[Configuration file reference](../reference/configuration.md) for the full list of available
settings.

---

## The metadata repository

The metadata repository is a GitHub repository named `.release-regent` within the same
organisation (or user account) as the repositories Release Regent manages.

For an event from `myorg/platform-api`, Release Regent auto-discovers the metadata
repository at `myorg/.release-regent`.

### Layout

```
myorg/.release-regent/
  global.toml          ← org-wide defaults and field locks
  groups/
    backend.toml       ← applied to repos that declare group = "backend"
    mobile.toml
    data-platform.toml
```

### How the GitHub App finds it

Release Regent uses the GitHub App installation API to discover whether the App is installed
on `{org}/.release-regent`. The installation ID is cached permanently for the lifetime of the
process (installation IDs are stable for the lifetime of a GitHub App installation).

The GitHub App must be installed **at the organisation level** (not scoped to individual
repositories) so that the installation can read any repository within the org. If the
installation is repository-scoped, policy file fetches will fail with `403`.

### Fallback when the metadata repository is absent

If the metadata repository does not exist, the App is not installed on it, or it is
temporarily unreachable due to a network error, Release Regent:

1. Emits a `warn!` log identifying the org and the reason.
2. Skips the global and group levels.
3. Uses the app-level config as the effective top of the hierarchy.
4. Continues processing the event normally.

The event is **not** failed. This means existing deployments without a metadata repository
continue to work without any changes.

---

## Group membership

A repository declares its group in its dotfile using a top-level `group` field:

```toml
# .release-regent.toml in myorg/platform-api
group = "backend"

[versioning]
strategy = "conventional"
```

When Release Regent processes an event for this repository, it:

1. Reads the `group` field from the repository dotfile.
2. Fetches `{org}/.release-regent/groups/backend.toml` from the metadata repository.
3. Merges the group policy over the global policy for unlocked fields.
4. Merges the repository dotfile over the group policy for unlocked fields.

If the group file does not exist in the metadata repository, Release Regent emits a `warn!`
and skips the group level. This is not an error — it is common during initial rollout.

The `group` field is meaningful **only** in repository dotfiles. If it appears in
`global.toml` or a group policy file, it is silently ignored with a `warn!`.

---

## Per-field locks

Global and group policy files may include a `locked_fields` array. Each entry is a dotted
field path. Lower levels cannot override a locked field.

```toml
# global.toml — enforce conventional commits and disable overrides org-wide
locked_fields = ["versioning.strategy", "versioning.allow_override"]

[versioning]
strategy = "conventional"
allow_override = false
```

```toml
# groups/backend.toml — additionally lock draft releases for backend services
locked_fields = ["releases.draft"]

[releases]
draft = false
```

### Lockable fields

Only the following fields may appear in `locked_fields`:

| Field path | Description |
| :--- | :--- |
| `versioning.strategy` | Versioning algorithm |
| `versioning.allow_override` | Whether PR comment override commands are permitted |
| `releases.draft` | Whether GitHub releases are created as drafts |
| `releases.prerelease` | Whether GitHub releases are marked pre-release |
| `releases.generate_notes` | Whether GitHub auto-generates release notes |
| `core.branches.main` | Name of the default/main branch |
| `core.version_prefix` | Prefix prepended to version tags |
| `error_handling.max_retries` | Maximum retry count |
| `error_handling.backoff_multiplier` | Exponential backoff multiplier |
| `error_handling.initial_delay_ms` | Initial retry delay |

The following fields are **never lockable** — they are always user-customisable at the
repository level:

- All `release_pr.*` fields (title template, body template, manifest files, auto-detect)
- All `notifications.*` fields

### Lock accumulation

Locks accumulate downward through the hierarchy. A group policy can add new locks on top of
global locks, but cannot remove a lock that global already set. Repository dotfiles cannot
add or remove locks.

### Lock conflict handling

When a lower-level config supplies a value for a locked field, Release Regent:

1. Keeps the locked (higher-level) value.
2. Emits a `warn!` identifying the field, the locked value, and the attempted override.
3. Continues processing the event normally — it is **not** failed.

This prevents immediate workflow disruption when a new lock is added centrally. Repository
owners can clean up their dotfiles at their own pace.

---

## Caching

Release Regent caches configuration to reduce GitHub API calls for high-traffic
organisations:

| Level | Cache key | TTL |
| :--- | :--- | :--- |
| Global policy | Organisation name | 600 seconds (10 min) |
| Group policy | `{org}/{group}` | 300 seconds (5 min) |
| Repository config | `{owner}/{repo}` | 300 seconds (5 min) |
| Metadata installation ID | Organisation name | Permanent (process lifetime) |

A parse or validation error at any level immediately evicts the corresponding cache entry.
The corrected file is picked up on the next event without a server restart.

---

## Failure behaviour summary

| Condition | Behaviour |
| :--- | :--- |
| Metadata repository absent or App not installed | Warn and skip global + group levels; continue |
| Metadata repository unreachable (transient network error) | Warn and skip global + group levels; continue |
| `global.toml` absent | Skip global level; continue |
| `global.toml` present but invalid | Hard-fail all events for this org |
| Group declared but group file absent | Warn and skip group level; continue |
| Group file present but invalid | Hard-fail events for repos in that group |
| Repository dotfile absent | Skip repo level; continue |
| Repository dotfile present but invalid | Hard-fail that event only |

The asymmetry between "absent → skip silently" and "present but invalid → hard fail" is
intentional: absence is a valid operational state (not yet configured), while an invalid file
is a configuration error that must be fixed.

---

## See also

- [Set up the metadata repository](../how-to/setup/metadata-repository.md) — step-by-step
  instructions for platform teams
- [Configuration file reference](../reference/configuration.md) — all settings available in a
  repository dotfile
- [ADR-007: Enterprise configuration hierarchy](../../adr/ADR-007-enterprise-config-hierarchy.md)
  — the architectural decision record that introduced this design
