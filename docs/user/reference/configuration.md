---
title: Configuration file reference
description: Complete reference for all options in .release-regent.toml
---

# Configuration file reference

Release Regent is configured through a `.release-regent.toml` file at the root of each
repository. All settings are optional — the tool works with sensible defaults if the file is
absent.

## File structure

```toml
[versioning]
# How versions are calculated and formatted

[release_pr]
# How release PRs are created and what they contain

[changelog]
# How changelogs are generated

[[changelog.commit_parsers]]
# Rules for commit classification (repeatable)
```

---

## Versioning configuration

### `versioning.prefix`

**Type**: string
**Default**: `"v"`

Prefix added to Git tags and version displays.

```toml
[versioning]
prefix = "v"           # Tags like v1.2.3
# prefix = ""          # Tags like 1.2.3
# prefix = "release-"  # Tags like release-1.2.3
```

### `versioning.allow_prerelease`

**Type**: boolean
**Default**: `true`

Whether to allow pre-release version identifiers (`-alpha.1`, `-beta.2`, `-rc.1`).

```toml
[versioning]
allow_prerelease = true   # Supports v1.2.3-beta.1
# allow_prerelease = false # Only stable versions
```

### `versioning.initial_version`

**Type**: string
**Default**: `"0.1.0"`

Version to use when the repository has no previous releases.

```toml
[versioning]
initial_version = "0.1.0"
```

### `versioning.allow_override`

**Type**: boolean
**Default**: `false`

Whether to allow
[PR comment commands](pr-commands.md) (`!set-version`, `!release`) to override the calculated
version.

```toml
[versioning]
allow_override = true
```

---

## Release PR configuration

### `release_pr.title_template`

**Type**: string
**Default**: `"chore(release): prepare version {version}"`

Template for the release PR title.

**Available variables**: `{version}`, `{version_tag}`, `{date}`

Both `{variable}` and `${variable}` syntax are supported.

```toml
[release_pr]
title_template = "chore(release): prepare version {version}"
# title_template = "Release {version_tag}"
# title_template = "Prepare release {version} ({date})"
```

### `release_pr.body_template`

**Type**: string
**Default**: A standard template showing version, changelog, commit count, and date

Template for the release PR body.

**Available variables**:

| Variable | Description |
| :--- | :--- |
| `{version}` | Semantic version, e.g. `1.2.3` |
| `{version_tag}` | Version with prefix, e.g. `v1.2.3` |
| `{changelog}` | Generated changelog content |
| `{commit_count}` | Commits since last release |
| `{date}` | Current date in ISO 8601 format |

```toml
[release_pr]
body_template = """
## Release {version}

{changelog}

---
{commit_count} commits · {date}
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

### `release_pr.auto_merge`

**Type**: boolean
**Default**: `false`

Whether to enable GitHub auto-merge on release PRs. Requires the repository to have auto-merge
enabled and the required status checks to pass.

```toml
[release_pr]
auto_merge = false
```

### `release_pr.auto_detect_manifests`

**Type**: boolean
**Default**: `true`

When `true`, Release Regent automatically detects and updates version fields in
`Cargo.toml`, `package.json`, `pyproject.toml`, and `composer.json` at the repository root.

Files listed in `manifest_files` are always processed regardless of this setting.

```toml
[release_pr]
auto_detect_manifests = true
```

### `release_pr.manifest_files`

**Type**: array of inline tables
**Default**: `[]`

Explicit list of manifest files to update. Each entry has three fields:

| Field | Description |
| :--- | :--- |
| `path` | Repository-relative path to the file |
| `format` | File format: `"toml"`, `"json"`, or `"plain_text"` |
| `version_key` | Where to find the version: dot-separated TOML path, JSON key, or a regex |

```toml
[release_pr]
manifest_files = [
  { path = "Cargo.toml",        format = "toml",       version_key = "package.version"    },
  { path = "package.json",      format = "json",       version_key = "version"            },
  { path = "pyproject.toml",    format = "toml",       version_key = "project.version"    },
]
```

See [Update manifest files](../how-to/configuration/update-manifest-files.md) for format
details and examples.

---

## Changelog configuration

### `changelog.include_authors`

**Type**: boolean
**Default**: `true`

Include commit author names in changelog entries.

### `changelog.include_commit_links`

**Type**: boolean
**Default**: `true`

Include linked commit SHAs in changelog entries.

### `changelog.include_pr_links`

**Type**: boolean
**Default**: `true`

Include linked PR numbers in changelog entries when detectable.

### `changelog.group_by`

**Type**: string
**Default**: `"type"`
**Options**: `"type"`, `"scope"`, `"none"`

How to group commits in the changelog body.

```toml
[changelog]
group_by = "type"
```

### `changelog.sort_commits`

**Type**: string
**Default**: `"date"`
**Options**: `"date"`, `"type"`, `"scope"`

How to sort commits within each group.

### `changelog.commit_types`

**Type**: table of string → string
**Default**: Standard conventional commit type labels

Maps commit type identifiers to display labels.

```toml
[changelog.commit_types]
feat     = "Features"
fix      = "Bug Fixes"
docs     = "Documentation"
perf     = "Performance Improvements"
refactor = "Code Refactoring"
chore    = "Maintenance"
```

### `changelog.header`

**Type**: string
**Default**: Standard header

Static text prepended to the changelog document.

### `changelog.body`

**Type**: string (Tera template)
**Default**: Standard git-cliff template

Main template rendered once per release. See
[Customise changelog templates](../how-to/configuration/custom-changelog-template.md) for the
full variable and filter reference.

### `changelog.footer`

**Type**: string (Tera template)
**Default**: `""`

Static text (or Tera template) appended to the changelog document.

### `changelog.trim`

**Type**: boolean
**Default**: `true`

Strip leading and trailing whitespace from template output.

### `changelog.filter_unconventional`

**Type**: boolean
**Default**: `true`

Exclude commits that do not follow the conventional commit format from the changelog.

### `changelog.protect_breaking_commits`

**Type**: boolean
**Default**: `false`

When `true`, breaking change commits are never filtered even if a matcher has `skip = true`.

---

## `[[changelog.commit_parsers]]`

Repeatable table that controls how commits are classified and grouped. Rules are evaluated
in order; the first matching rule wins.

Each entry can have:

| Field | Description |
| :--- | :--- |
| `message` | Regex matched against the commit subject line |
| `group` | Group label to assign to matching commits |
| `skip` | When `true`, matching commits are excluded from the changelog |

```toml
[[changelog.commit_parsers]]
message = "^chore\\(release\\): prepare"
skip = true

[[changelog.commit_parsers]]
message = "^feat"
group = "🚀 Features"

[[changelog.commit_parsers]]
message = "^fix"
group = "🐛 Bug Fixes"

[[changelog.commit_parsers]]
message = "^docs"
group = "📚 Documentation"

[[changelog.commit_parsers]]
message = "^perf"
group = "⚡ Performance"

[[changelog.commit_parsers]]
message = "^chore"
group = "🔧 Maintenance"
```

---

## `[[changelog.postprocessors]]`

Repeatable table of regex-replacement rules applied to the rendered changelog after all
templates are evaluated. Useful for inserting repository URLs or standardising link formats.

| Field | Description |
| :--- | :--- |
| `pattern` | Regex to search for |
| `replace` | Replacement string (may use capture groups: `$1`, `$2`) |

```toml
[[changelog.postprocessors]]
pattern = "\\(#(\\d+)\\)"
replace = "([#$1](https://github.com/myorg/myrepo/issues/$1))"
```
