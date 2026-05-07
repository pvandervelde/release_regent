---
title: PR comment commands
description: Syntax and permissions for the !set-version and !release comment commands
---

# PR comment commands

PR comment commands let authorised team members override Release Regent's automatic version
calculation by posting a specially formatted comment on a pull request. No configuration file
edits or Git operations are required.

## Enabling commands

Commands are disabled unless `allow_override = true` is set in `.release-regent.toml`:

```toml
[versioning]
allow_override = true
```

## Permissions

The GitHub user posting the command must have **Write** or higher access to the repository.
Comments from users with only Read access are ignored.

---

## `!set-version`

Set an exact version on a release PR.

### Syntax

```
!set-version <VERSION>
```

`<VERSION>` must be a valid semantic version, optionally with a pre-release identifier.

### Valid examples

```
!set-version 2.0.0
!set-version 1.3.0
!set-version 2.0.0-beta.1
!set-version 0.1.0-rc.2
```

### What it does

When Release Regent receives this command on a **release PR**:

1. Updates the PR title to reflect the new version.
2. Renames the release branch from `release/v<old>` to `release/v<new>`. If the new branch
   name already exists, a timestamp suffix (`-20250506T143052Z`) is appended.
3. Updates the changelog heading to the new version.

### Scope

Only works on release PRs (PRs whose head branch matches the `release/v*` pattern). Posting
this command on a regular feature PR has no effect.

---

## `!release`

Raise the version floor on a feature PR.

### Syntax

```
!release <LEVEL>
```

`<LEVEL>` is one of: `major`, `minor`, `patch`.

### Valid examples

```
!release major
!release minor
!release patch
```

### What it does

When Release Regent receives this command on a **regular (non-release) PR**:

1. Stores the requested bump level as a GitHub label on the PR.
2. When the PR is merged, the stored level acts as a **floor** during version calculation —
   the calculated bump will be at least `<LEVEL>`, even if commits alone would produce a
   smaller bump.
3. The label is consumed after the release PR is created. It does not affect future release
   cycles.

### Scope

Only works on regular (non-release) PRs. Posting this command on a release PR has no effect.

---

## Error handling

If a command is syntactically invalid (e.g., `!set-version not-a-version` or
`!release massive`), Release Regent posts a comment on the PR explaining the error. The
original PR state is unchanged.

If the posting user lacks write access, the command is silently ignored.

---

## Audit trail

Every command that Release Regent processes is recorded in the server logs with the repository,
PR number, user, and the action taken. This provides an audit trail for compliance and
debugging purposes.
