---
title: Configuration file reference
description: Complete reference for all options in the Release Regent configuration file (YAML or TOML)
---

# Configuration file reference

Release Regent is configured through a file at the root of each repository. All settings are
optional — the tool works with sensible defaults if the file is absent.

## Supported file names and formats

Both YAML and TOML are supported. Release Regent searches for configuration files in the
following order:

| File name | Format |
| :--- | :--- |
| `.release-regent.yml` | YAML |
| `.release-regent.yaml` | YAML |
| `release-regent.yml` | YAML |
| `release-regent.yaml` | YAML |
| `.release-regent.toml` | TOML |
| `release-regent.toml` | TOML |

**`rr init` creates `.release-regent.yml` (YAML) by default.** You can rename it or convert it
to TOML at any time — the format is determined by the file extension.

## File structure

=== "YAML"

    ```yaml
    core:
      # Version prefix and branch settings

    versioning:
      # How versions are calculated

    release_pr:
      # How release PRs are created and what they contain

    releases:
      # How GitHub releases are published

    error_handling:
      # Retry behaviour

    notifications:
      # Error notification settings
    ```

=== "TOML"

    ```toml
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

!!! note "Examples in this document"
    All option examples below are shown in YAML, which `rr init` produces by default.
    TOML equivalents use the same key names; YAML maps (`key: value`) become `[section]`
    headers and YAML sequences become `[[section]]` arrays.

---

## `core` — core settings

### `core.version_prefix`

**Type**: string
**Default**: `"v"`

Prefix prepended to version numbers in Git tags and release PR titles.

```yaml
core:
  version_prefix: "v"           # Tags like v1.2.3
  # version_prefix: ""          # Tags like 1.2.3
  # version_prefix: "release-"  # Tags like release-1.2.3
```

### `core.branches.main`

**Type**: string
**Default**: `"main"`

The default branch of the repository. Release Regent targets this branch when creating release
PRs and reading commit history.

```yaml
core:
  branches:
    main: "main"
    # main: "master"
```

---

## `versioning` — version calculation

### `versioning.strategy`

**Type**: string or object
**Default**: `"conventional"`

How the next version is calculated.

| Value | Behaviour |
| :--- | :--- |
| `"conventional"` | Analyse commit messages using the [Conventional Commits](conventional-commits.md) standard |
| `!external` | Delegate to an external command (see below) |

```yaml
versioning:
  strategy: "conventional"
```

#### External strategy

When `strategy` is `!external`, the following fields are required or optional:

```yaml
versioning:
  strategy: !external
    command: "./scripts/calculate-version.sh"
    env_vars: {}          # Optional: extra environment variables passed to the command
    timeout_ms: 30000     # Optional: max execution time in milliseconds (default 30 000)
```

### `versioning.allow_override`

**Type**: boolean
**Default**: `true`

Whether contributors can override the calculated version bump using
[PR comment commands](pr-commands.md) (e.g. `!set-version`).

```yaml
versioning:
  allow_override: true
```

### `versioning.excluded_pr_authors`

**Type**: list of strings
**Default**: `[]`

PR author logins that Release Regent silently ignores. PRs opened by a login in this list do
not receive a projected-version comment and are skipped during the post-merge refresh. Useful
for bot accounts that open dependency-update PRs.

```yaml
versioning:
  excluded_pr_authors:
    - "dependabot[bot]"
    - "renovate[bot]"
```

---

## `release_pr` — release pull requests

### `release_pr.title_template`

**Type**: string
**Default**: `"chore(release): ${version}"`

Template for the release PR title. Use `${version}` as the placeholder — both `${variable}`
and `{variable}` syntax are accepted.

```yaml
release_pr:
  title_template: "chore(release): ${version}"
  # title_template: "Release ${version}"
  # title_template: "Prepare release ${version}"
```

### `release_pr.body_template`

**Type**: string
**Default**: `"## Changelog\n\n${changelog}"`

Template for the release PR body. Use `${changelog}` to insert the generated changelog.

```yaml
release_pr:
  body_template: |
    ## Changelog

    ${changelog}
```

### `release_pr.draft`

**Type**: boolean
**Default**: `false`

Whether to create release PRs as GitHub draft PRs.

```yaml
release_pr:
  draft: false
  # draft: true  # Require manual "Ready for review" before merging
```

### `release_pr.auto_detect_manifests`

**Type**: boolean
**Default**: `true`

When `true`, Release Regent automatically detects and updates the version field in
`Cargo.toml`, `package.json`, `pyproject.toml`, and `composer.json` at the repository root.

Files listed in `manifest_files` are always processed regardless of this setting.

```yaml
release_pr:
  auto_detect_manifests: true
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

```yaml
release_pr:
  manifest_files:
    - path: "Cargo.toml"
      format: "toml"
      version_key: "package.version"
    - path: "package.json"
      format: "json"
      version_key: "version"
    - path: "pyproject.toml"
      format: "toml"
      version_key: "tool.poetry.version"
    - path: "VERSION"
      format: "plain_text"
      version_key: "^([0-9]+\\.[0-9]+\\.[0-9]+)$"
```

See [Update manifest files](../how-to/configuration/update-manifest-files.md) for detailed
format guidance.

---

## `releases` — GitHub releases

### `releases.draft`

**Type**: boolean
**Default**: `false`

Publish releases as drafts (not publicly visible until manually published in the GitHub UI).

```yaml
releases:
  draft: false
```

### `releases.prerelease`

**Type**: boolean
**Default**: `false`

Mark releases as pre-releases in the GitHub UI.

```yaml
releases:
  prerelease: false
```

### `releases.generate_notes`

**Type**: boolean
**Default**: `true`

When `true`, GitHub auto-generates release notes from merged PRs in addition to the changelog
body. These notes appear in the GitHub release alongside the release PR body content.

```yaml
releases:
  generate_notes: true
```

---

## `error_handling` — retry behaviour

### `error_handling.max_retries`

**Type**: integer
**Default**: `5`

Maximum number of retries for transient GitHub API failures.

```yaml
error_handling:
  max_retries: 5
```

### `error_handling.backoff_multiplier`

**Type**: float
**Default**: `2.0`

Multiplier applied to the delay after each failed attempt (exponential back-off).

```yaml
error_handling:
  backoff_multiplier: 2.0
```

### `error_handling.initial_delay_ms`

**Type**: integer (milliseconds)
**Default**: `1000`

Delay before the first retry.

```yaml
error_handling:
  initial_delay_ms: 1000
```

---

## `notifications` — error notifications

### `notifications.enabled`

**Type**: boolean
**Default**: `true`

Whether to send notifications when Release Regent encounters an error.

```yaml
notifications:
  enabled: true
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

```yaml
notifications:
  strategy: "github_issue"
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

```yaml
notifications:
  strategy: "github_issue"
  github_issue:
    labels:
      - "release-regent"
      - "bug"
    assignees: []
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

```yaml
notifications:
  strategy: "webhook"
  webhook:
    url: "https://hooks.example.com/release-regent"
    headers:
      Authorization: "Bearer mytoken"
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

```yaml
notifications:
  strategy: "slack"
  slack:
    webhook_url: "https://hooks.slack.com/services/T00/B00/xxx"
    channel: "#releases"
```

---

## Complete example

```yaml
core:
  version_prefix: "v"
  branches:
    main: "main"

versioning:
  strategy: "conventional"
  allow_override: true
  excluded_pr_authors:
    - "dependabot[bot]"
    - "renovate[bot]"

release_pr:
  title_template: "chore(release): ${version}"
  body_template: |
    ## Changelog

    ${changelog}
  draft: false
  auto_detect_manifests: true
  manifest_files: []

releases:
  draft: false
  prerelease: false
  generate_notes: true

error_handling:
  max_retries: 5
  backoff_multiplier: 2.0
  initial_delay_ms: 1000

notifications:
  enabled: true
  strategy: "github_issue"
  github_issue:
    labels:
      - "release-regent"
      - "bug"
    assignees: []
```