---
title: Configuration file reference
description: Complete reference for all options in the Release Regent configuration file
---

# Configuration file reference

Release Regent is configured through a TOML file at the root of each repository. All settings are
optional — the tool works with sensible defaults if the file is absent.

## Supported file names

Release Regent uses configuration files in two different ways depending on how you deploy it.

### Local file discovery (CLI and app-level config)

When the CLI or the server reads configuration from the local file system, it searches for
files in the following order inside `CONFIG_DIR` (or the current directory):

| File name | Format |
| :--- | :--- |
| `release-regent.toml` | TOML |
| `release_regent.toml` | TOML |
| `config.toml` | TOML |

**`rr init` creates `release-regent.toml` by default.**

### Repository dotfile (server, fetched via GitHub API)

When the server processes a webhook event, it fetches the per-repository dotfile from the
target repository over the GitHub API. The server probes exactly one path:

| File name | Format |
| :--- | :--- |
| `.release-regent.toml` (leading dot) | TOML |

This filename with a leading dot is the convention for repository-level dotfiles fetched
from GitHub. It is **not** part of the local file discovery list above.

!!! note "Migrating from YAML"
    Previous versions of Release Regent also accepted `.release-regent.yml` and related YAML
    file names. YAML support has been removed. If your repository uses a `.release-regent.yml`
    file, rename it to `.release-regent.toml` (keeping the leading dot — this is the
    GitHub-fetched dotfile) and convert the contents to TOML syntax before upgrading.
    See [Migrating from YAML configuration](../how-to/configuration/migrate-from-yaml.md)
    for step-by-step instructions.

## File structure

```toml
# group = "name"       # Optional: group policy membership (repo dotfile only)
# locked_fields = []   # Optional: field locks (global.toml / group files only)

[core]
# Version prefix and branch settings

[versioning]
# How versions are calculated

[release_pr]
# How release PRs are created and what they contain

[releases]
# How GitHub releases are published

[error_handling]
# Retry behaviour

[notifications]
# Error notification settings
```

## `core` — core settings

### `core.version_prefix`

**Type**: string
**Default**: `"v"`

Prefix prepended to version numbers in Git tags and release PR titles.

```toml
[core]
version_prefix = "v"           # Tags like v1.2.3
# version_prefix = ""          # Tags like 1.2.3
# version_prefix = "release-"  # Tags like release-1.2.3
```

### `core.branches.main`

**Type**: string
**Default**: `"main"`

The default branch of the repository. Release Regent targets this branch when creating release
PRs and reading commit history.

```toml
[core.branches]
main = "main"
# main = "master"
```

---

## `group` — group membership

**Type**: string
**Default**: *(absent)*

Declares the [configuration group](../explanation/configuration-hierarchy.md#group-membership)
this repository belongs to. When set, Release Regent fetches
`{org}/.release-regent/groups/{group}.toml` from the metadata repository and merges it as an
additional policy layer above the global policy.

This field is meaningful **only** in repository dotfiles. If it appears in `global.toml` or a
group policy file it is silently ignored with a `warn!` log entry.

```toml
group = "backend"
```

See [Set up the metadata repository](../how-to/setup/metadata-repository.md) for how platform
teams create group policy files.

---

## `versioning` — version calculation

### `versioning.strategy`

**Type**: string or object
**Default**: `"conventional"`

How the next version is calculated.

| Value | Behaviour |
| :--- | :--- |
| `"conventional"` | Analyse commit messages using the [Conventional Commits](conventional-commits.md) standard |
| `external` (object — see below) | Delegate to an external command |

```toml
[versioning]
strategy = "conventional"
```

#### External strategy

```toml
[versioning.strategy.external]
command = "./scripts/calculate-version.sh"
env_vars = {}           # Optional: extra environment variables passed to the command
timeout_ms = 30000      # Optional: max execution time in milliseconds (default 30 000)
```

### `versioning.allow_override`

**Type**: boolean
**Default**: `true`

Whether contributors can override the calculated version bump using
[PR comment commands](pr-commands.md) (e.g. `!set-version`).

```toml
[versioning]
allow_override = true
```

### `versioning.excluded_pr_authors`

**Type**: list of strings
**Default**: `[]`

PR author logins that Release Regent silently ignores. PRs opened by a login in this list do
not receive a projected-version comment and are skipped during the post-merge refresh. Useful
for bot accounts that open dependency-update PRs.

```toml
[versioning]
excluded_pr_authors = ["dependabot[bot]", "renovate[bot]"]
```

---

## `release_pr` — release pull requests

### `release_pr.title_template`

**Type**: string
**Default**: `"chore(release): ${version}"`

Template for the release PR title. Use `${version}` as the placeholder — both `${variable}`
and `{variable}` syntax are accepted.

```toml
[release_pr]
title_template = "chore(release): ${version}"
# title_template = "Release ${version}"
# title_template = "Prepare release ${version}"
```

### `release_pr.body_template`

**Type**: string
**Default**: `"## Changelog\n\n${changelog}"`

Template for the release PR body. Use `${changelog}` to insert the generated changelog.

```toml
[release_pr]
body_template = """
## Changelog

${changelog}
"""
```

### `release_pr.draft`

**Type**: boolean
**Default**: `false`

Whether to create release PRs as GitHub draft PRs.

```toml
[release_pr]
draft = false
# draft = true  # Require manual "Ready for review" before merging
```

### `release_pr.auto_detect_manifests`

**Type**: boolean
**Default**: `true`

When `true`, Release Regent automatically detects and updates the version field in
`Cargo.toml`, `package.json`, `pyproject.toml`, and `composer.json` at the repository root.

Files listed in `manifest_files` are always processed regardless of this setting.

```toml
[release_pr]
auto_detect_manifests = true
```

### `release_pr.manifest_files`

**Type**: list of objects
**Default**: `[]`

Explicit list of version manifest files to update when creating the release branch. Each entry
has three required fields:

| Field | Description |
| :--- | :--- |
| `path` | Repository-relative path to the file |
| `format` | File format: `"toml"`, `"json"`, or `"plain_text"` |
| `version_key` | Location of the version field (see table below) |

**`version_key` by format**:

| Format | `version_key` meaning | Example |
| :--- | :--- | :--- |
| `"toml"` | Dot-separated table path | `"package.version"` |
| `"json"` | Top-level key | `"version"` |
| `"plain_text"` | Regex with one capture group matching the current version | `"^version = \"(.+)\"$"` |

```toml
[[release_pr.manifest_files]]
path = "Cargo.toml"
format = "toml"
version_key = "package.version"

[[release_pr.manifest_files]]
path = "package.json"
format = "json"
version_key = "version"

[[release_pr.manifest_files]]
path = "pyproject.toml"
format = "toml"
version_key = "tool.poetry.version"

[[release_pr.manifest_files]]
path = "VERSION"
format = "plain_text"
version_key = "^([0-9]+\\.[0-9]+\\.[0-9]+)$"
```

See [Update manifest files](../how-to/configuration/update-manifest-files.md) for detailed
format guidance.

---

## `releases` — GitHub releases

### `releases.draft`

**Type**: boolean
**Default**: `false`

Publish releases as drafts (not publicly visible until manually published in the GitHub UI).

```toml
[releases]
draft = false
```

### `releases.prerelease`

**Type**: boolean
**Default**: `false`

Mark releases as pre-releases in the GitHub UI.

```toml
[releases]
prerelease = false
```

### `releases.generate_notes`

**Type**: boolean
**Default**: `true`

When `true`, GitHub auto-generates release notes from merged PRs in addition to the changelog
body. These notes appear in the GitHub release alongside the release PR body content.

```toml
[releases]
generate_notes = true
```

---

## `error_handling` — retry behaviour

### `error_handling.max_retries`

**Type**: integer
**Default**: `5`

Maximum number of retries for transient GitHub API failures.

```toml
[error_handling]
max_retries = 5
```

### `error_handling.backoff_multiplier`

**Type**: float
**Default**: `2.0`

Multiplier applied to the delay after each failed attempt (exponential back-off).

```toml
[error_handling]
backoff_multiplier = 2.0
```

### `error_handling.initial_delay_ms`

**Type**: integer (milliseconds)
**Default**: `1000`

Delay before the first retry.

```toml
[error_handling]
initial_delay_ms = 1000
```

---

## `notifications` — error notifications

### `notifications.enabled`

**Type**: boolean
**Default**: `true`

Whether to send notifications when Release Regent encounters an error.

```toml
[notifications]
enabled = true
```

### `notifications.strategy`

**Type**: string
**Default**: `"github_issue"`

How errors are reported.

| Value | Behaviour |
| :--- | :--- |
| `"github_issue"` | Open a GitHub issue in the repository (default) |
| `"webhook"` | POST to an HTTP endpoint |
| `"slack"` | Send a Slack message |
| `"none"` | Do not send notifications |

```toml
[notifications]
strategy = "github_issue"
```

### `notifications.github_issue`

Settings used when `strategy` is `"github_issue"`.

#### `notifications.github_issue.labels`

**Type**: list of strings
**Default**: `["release-regent", "bug"]`

Labels applied to newly created error issues.

#### `notifications.github_issue.assignees`

**Type**: list of strings
**Default**: `[]`

GitHub usernames to assign to newly created error issues.

```toml
[notifications]
strategy = "github_issue"

[notifications.github_issue]
labels = ["release-regent", "bug"]
assignees = []
```

### `notifications.webhook`

Settings used when `strategy` is `"webhook"`.

#### `notifications.webhook.url`

**Type**: string (**required** when strategy is `"webhook"`)

HTTP endpoint to POST the error payload to.

#### `notifications.webhook.headers`

**Type**: object (string → string)
**Default**: `{}`

Additional HTTP headers included in the POST request.

```toml
[notifications]
strategy = "webhook"

[notifications.webhook]
url = "https://hooks.example.com/release-regent"

[notifications.webhook.headers]
Authorization = "Bearer mytoken"
```

### `notifications.slack`

Settings used when `strategy` is `"slack"`.

#### `notifications.slack.webhook_url`

**Type**: string (**required** when strategy is `"slack"`)

Slack incoming webhook URL.

#### `notifications.slack.channel`

**Type**: string
**Default**: the channel configured in the Slack webhook

Override the target Slack channel.

```toml
[notifications]
strategy = "slack"

[notifications.slack]
webhook_url = "https://hooks.slack.com/services/T00/B00/xxx"
channel = "#releases"
```

---

## `locked_fields` — policy locks

**Type**: list of strings
**Default**: `[]`
**Valid in**: `global.toml` and group policy files in the metadata repository only

A list of dotted field paths that lower configuration levels cannot override. Repository
dotfiles cannot set this field — if present, the field is silently ignored with a `warn!`.

```toml
# global.toml — lock versioning strategy and PR overrides org-wide
locked_fields = ["versioning.strategy", "versioning.allow_override"]

[versioning]
strategy = "conventional"
allow_override = false
```

The following fields may be locked:

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

All `release_pr.*` and `notifications.*` fields are never lockable.

For the full rules on lock accumulation and conflict handling, see
[Configuration hierarchy — per-field locks](../explanation/configuration-hierarchy.md#per-field-locks).

---

## Complete example

```toml
[core]
version_prefix = "v"

[core.branches]
main = "main"

[versioning]
strategy = "conventional"
allow_override = true
excluded_pr_authors = ["dependabot[bot]", "renovate[bot]"]

[release_pr]
title_template = "chore(release): ${version}"
body_template = """
## Changelog

${changelog}
"""
draft = false
auto_detect_manifests = true

[releases]
draft = false
prerelease = false
generate_notes = true

[error_handling]
max_retries = 5
backoff_multiplier = 2.0
initial_delay_ms = 1000

[notifications]
enabled = true
strategy = "github_issue"

[notifications.github_issue]
labels = ["release-regent", "bug"]
assignees = []
```
