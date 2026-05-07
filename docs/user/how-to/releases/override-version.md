---
title: Override the version
description: How to use PR comment commands to set an exact version or raise the version floor
---

# Override the version

Release Regent calculates the next version automatically from conventional commits. Sometimes
you need to step in and set a different version — for example, to ship a major release early or
to correct an automated calculation that does not match your team's intent.

You can override the version using PR comment commands without editing configuration files or
touching Git tags.

For full syntax of each command, see the
[PR comment commands reference](../../reference/pr-commands.md).

## Prerequisites

Your repository's `.release-regent.toml` must have version overrides enabled:

```toml
[versioning]
allow_override = true
```

The GitHub user posting the comment must have **Write** or higher access to the repository.

## Set an exact version on a release PR

Post a comment on the **release PR** (the automatically created PR with the
`release/v*` branch):

```
!set-version 2.0.0
```

Release Regent updates the release PR title, branch name, and changelog heading to reflect
`2.0.0`. If the branch name would conflict, a timestamp suffix is appended automatically.

!!! note
    `!set-version` only works on release PRs. Posting it on a regular feature PR has no effect.

## Raise the version floor on a feature PR

Post a comment on a **regular feature PR** (before it is merged to `main`):

```
!release major
```

This stores the intent as a GitHub label. When Release Regent next calculates the version for
a release PR, it treats `major` as a floor — the calculated bump will be at least `major`,
even if the commits alone would only warrant a `minor` or `patch` bump.

Accepted values: `major`, `minor`, `patch`.

### When is the label applied?

The label is applied immediately when Release Regent receives the comment event. You can post
the comment at any time while the PR is open.

### What happens after the PR is merged?

When the feature PR is merged, Release Regent reads the stored label and applies the floor
during version calculation. The label is consumed — it does not persist to future release
cycles.

## Examples

### Force a major release for a breaking change not captured in commits

1. Your feature PR contains a breaking API change but the commits use `feat:` rather than
   `feat!:` (perhaps the change was added during code review without a new commit).
2. Post a comment on the feature PR before merging:

   ```
   !release major
   ```

3. Merge the feature PR as normal. Release Regent will bump the major version.

### Correct a version after the release PR is already open

1. Release Regent calculated `1.3.0` but you want to release `2.0.0` for marketing reasons.
2. Find the open release PR (branch `release/v1.3.0`).
3. Post a comment on the release PR:

   ```
   !set-version 2.0.0
   ```

4. Release Regent updates the PR and renames the branch to `release/v2.0.0`.

---

## Next steps

- [PR comment commands reference](../../reference/pr-commands.md) — full syntax and permissions
- [How versions are calculated](../../explanation/version-calculation.md) — understand what the
  automation does before you override it
