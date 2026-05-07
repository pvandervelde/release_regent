---
title: Update manifest files
description: How to keep Cargo.toml, package.json and other version files in sync with each release
---

# Update manifest files

Release Regent can automatically update version fields in common manifest files as part of the
release PR. This means `Cargo.toml`, `package.json`, `pyproject.toml`, and other version files
will already reflect the new version when you merge the release PR.

## Auto-detection

By default, `auto_detect_manifests = true` tells Release Regent to probe the repository for
well-known manifest files and update any it finds:

| File | Format | Key |
| :--- | :--- | :--- |
| `Cargo.toml` | TOML | `package.version` |
| `package.json` | JSON | `version` |
| `pyproject.toml` (PEP 621) | TOML | `project.version` |
| `composer.json` | JSON | `version` |

No configuration is needed for these files. If any are present in the repository root when a
release PR is created, their version field is updated to the new release version.

To disable auto-detection:

```toml
[release_pr]
auto_detect_manifests = false
```

## Explicit manifest list

To control exactly which files are updated — or to handle files in subdirectories, files with
non-standard key names, or plain-text version files — list them explicitly in
`manifest_files`:

```toml
[release_pr]
manifest_files = [
  { path = "Cargo.toml",     format = "toml", version_key = "package.version" },
  { path = "package.json",   format = "json", version_key = "version" },
]
```

Files listed here are always updated, regardless of `auto_detect_manifests`.

## Supported formats

### TOML

For `.toml` files, `version_key` is a dot-separated path to the field:

```toml
# Cargo.toml (standard Rust workspace)
{ path = "Cargo.toml", format = "toml", version_key = "package.version" }

# pyproject.toml — PEP 621 style
{ path = "pyproject.toml", format = "toml", version_key = "project.version" }

# pyproject.toml — Poetry style
{ path = "pyproject.toml", format = "toml", version_key = "tool.poetry.version" }
```

### JSON

For `.json` files, `version_key` names the top-level property:

```toml
# package.json
{ path = "package.json", format = "json", version_key = "version" }

# composer.json
{ path = "composer.json", format = "json", version_key = "version" }
```

### Plain text

For files whose version is embedded in arbitrary text, use `plain_text` format and provide a
regular expression with exactly one capture group that matches the current version:

```toml
# A file that contains a single version string on its own line
{ path = "VERSION", format = "plain_text", version_key = "^([0-9]+\\.[0-9]+\\.[0-9]+)$" }
```

Release Regent replaces the captured group with the new version string. The rest of the file
is left unchanged.

## Files in subdirectories

Set `path` to any repository-relative path:

```toml
[release_pr]
manifest_files = [
  { path = "rust/Cargo.toml",    format = "toml", version_key = "package.version" },
  { path = "js/package.json",    format = "json", version_key = "version" },
  { path = "python/pyproject.toml", format = "toml", version_key = "project.version" },
]
```

---

## Next steps

- [Configuration reference — `release_pr` section](../../reference/configuration.md#release-pr-configuration)
