---
title: Migrate from YAML configuration to TOML
description: Step-by-step instructions for converting a .release-regent.yml file to .release-regent.toml
---

# Migrate from YAML configuration to TOML

Release Regent no longer reads YAML configuration files. If your repository uses a
`.release-regent.yml` (or `.release-regent.yaml`) file, you must convert it to TOML before
upgrading.

Repositories that have no configuration file — or already use `.release-regent.toml` — are
unaffected.

## Step 1: Rename the file

Rename `.release-regent.yml` to `.release-regent.toml` in your repository root.

```bash
git mv .release-regent.yml .release-regent.toml
```

## Step 2: Convert the contents to TOML syntax

TOML and YAML use different syntax for the same logical structure. The table below shows
the most common conversions.

| YAML | TOML |
| :--- | :--- |
| `key: value` | `key = "value"` |
| `key: true` / `key: false` | `key = true` / `key = false` |
| `key: 42` | `key = 42` |
| Nested map under `section:` | `[section]` header |
| List item `- item` | `["item"]` inline array, or `[[section]]` array of tables |

### Minimal configuration

YAML:

```yaml
core:
  version_prefix: "v"
  branches:
    main: "main"
```

TOML equivalent:

```toml
[core]
version_prefix = "v"

[core.branches]
main = "main"
```

### Standard configuration

YAML:

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
  draft: false

releases:
  draft: false
  prerelease: false
  generate_notes: true
```

TOML equivalent:

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
draft = false

[releases]
draft = false
prerelease = false
generate_notes = true
```

### External versioning strategy

YAML used a custom `!external` tag:

```yaml
versioning:
  strategy: !external
    command: "./scripts/calculate-version.sh"
    timeout_ms: 30000
```

TOML uses a sub-table:

```toml
[versioning.strategy.external]
command = "./scripts/calculate-version.sh"
timeout_ms = 30000
```

### Multi-line string templates

YAML block scalars (using `|`) become TOML multi-line basic strings (using `"""`):

```yaml
release_pr:
  body_template: |
    ## Changelog

    ${changelog}
```

TOML equivalent:

```toml
[release_pr]
body_template = """
## Changelog

${changelog}
"""
```

### Manifest files list

YAML sequences of objects become TOML arrays of tables using `[[section]]`:

```yaml
release_pr:
  manifest_files:
    - path: "Cargo.toml"
      format: "toml"
      version_key: "package.version"
    - path: "package.json"
      format: "json"
      version_key: "version"
```

TOML equivalent:

```toml
[[release_pr.manifest_files]]
path = "Cargo.toml"
format = "toml"
version_key = "package.version"

[[release_pr.manifest_files]]
path = "package.json"
format = "json"
version_key = "version"
```

## Step 3: Commit and verify

Commit the renamed and converted file:

```bash
git add .release-regent.toml
git commit -m "chore: migrate Release Regent config from YAML to TOML"
```

After upgrading Release Regent, open a test PR to confirm the configuration is read
correctly. If the configuration file has a syntax error, Release Regent logs a
`CoreError::Config` and hard-fails the event — check the server logs for details.

## Further reading

- [Configuration file reference](../../reference/configuration.md)
