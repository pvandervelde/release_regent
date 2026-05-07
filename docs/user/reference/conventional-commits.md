---
title: Conventional commits
description: Which commit types produce which version bumps and how to write them correctly
---

# Conventional commits

Release Regent uses the [Conventional Commits](https://www.conventionalcommits.org/) standard
to determine which semantic version component to increment. This page is the reference for how
that mapping works.

## Commit message format

```
<type>[(<scope>)][!]: <description>

[optional body]

[optional footers]
```

The `type` and `description` are required. `scope`, `!`, body, and footers are all optional.

### Examples

```
feat: add user authentication
fix(auth): resolve session timeout
feat!: remove deprecated API endpoint
docs(readme): update installation instructions
chore(deps): bump serde to 1.0.200
```

---

## Version bump rules

| Commit syntax | Bump | Example |
| :--- | :--- | :--- |
| `feat: …` | Minor | `1.2.0 → 1.3.0` |
| `fix: …` | Patch | `1.2.0 → 1.2.1` |
| `feat!: …` | Major | `1.2.0 → 2.0.0` |
| `fix!: …` | Major | `1.2.0 → 2.0.0` |
| `BREAKING CHANGE:` footer | Major | `1.2.0 → 2.0.0` |
| `docs: …` | None | changelog only |
| `style: …` | None | changelog only |
| `refactor: …` | None | changelog only |
| `perf: …` | None | changelog only |
| `test: …` | None | changelog only |
| `build: …` | None | changelog only |
| `ci: …` | None | changelog only |
| `chore: …` | None | changelog only |

When multiple commits contribute different bump sizes (e.g., one `feat:` and two `fix:`),
Release Regent picks the largest bump. A single `feat!:` overrides everything else in the set
and produces a major bump.

### Breaking change declaration

A commit is treated as a breaking change in either of two ways:

**Type with `!`**:

```
feat!: rename the configuration file format
```

**`BREAKING CHANGE:` footer**:

```
feat: redesign the webhook API

This commit changes the expected request body format.

BREAKING CHANGE: The `payload` field is now `data`. Clients must update their
integration accordingly.
```

Both produce an equivalent major version bump. The footer form lets you include a longer
explanation of the breaking change, which Release Regent surfaces in the changelog.

---

## Scope

The scope is a short noun in parentheses after the type that describes the part of the
codebase affected:

```
fix(auth): handle expired tokens correctly
feat(api): add pagination support
```

Scopes are optional but improve changelog readability when the project has multiple distinct
components. When `group_by = "scope"` is set in `.release-regent.toml`, scopes become the
primary grouping key in the changelog.

---

## Commits that do not follow the convention

Commits that do not match the `type: description` format are treated as non-version-bumping
and are controlled by the `filter_unconventional` setting:

- `filter_unconventional = true` (default): non-conventional commits are excluded from the
  changelog
- `filter_unconventional = false`: non-conventional commits appear under "Other Changes"

They never contribute to a version bump regardless of this setting.

---

## Commit type quick reference

| Type | Bump | Description |
| :--- | :--- | :--- |
| `feat` | Minor | New feature for the user |
| `fix` | Patch | Bug fix for the user |
| `feat!` / `fix!` / any `!` | Major | Any change with `!` or `BREAKING CHANGE` footer |
| `docs` | None | Documentation only |
| `style` | None | Whitespace, formatting, missing semicolons |
| `refactor` | None | Refactoring production code |
| `perf` | None | Performance improvements |
| `test` | None | Adding or refactoring tests |
| `build` | None | Build system or dependency changes |
| `ci` | None | CI configuration changes |
| `chore` | None | Maintenance, tooling, other |
| `revert` | Patch | Reverts a previous commit |
