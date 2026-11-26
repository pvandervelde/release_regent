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

### Minimum version enforced by outstanding overrides

If an `rr:override-*` label is still outstanding on a merged PR in the current release cycle,
`!set-version` will not let you pin below the floor that label requires. A pinned version below
the floor is rejected with a comment naming the required minimum version, the PR the override
came from, and what to do next:

> ❌ **Release Regent**: `!set-version 1.3.0` was rejected — an `rr:override-major` label is
> outstanding on PR #55, which requires the release version to be at least `2.0.0`. Use
> `!set-version 2.0.0` or higher, or remove the `!release major` override first.

Either pin to the required floor version (or higher), or remove the `rr:override-*` label from
the originating PR first if the override no longer applies.

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

### The floor still applies if a different PR merges first

You do not need the labelled PR to be the one that triggers the release PR update. If a
**later** feature PR (one without its own override label) merges while the override is still
outstanding, Release Regent scans the commit history for the current release cycle, finds the
highest `rr:override-*` label among the PRs that have already merged, and still applies it as
the floor. This also covers the case where the release PR was deleted before merging and a new
one is created by that later merge — the new release PR's version honors the outstanding
override instead of being recalculated from scratch. The label itself is only removed once the
PR that carries it is merged.

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

### Keep an override after a different PR merges first

1. Post `!release major` on feature PR #55 (not yet merged). Release Regent labels PR #55 with
   `rr:override-major`.
2. PR #56, an unrelated minor feature with no override label, merges before #55 does. Release
   Regent scans the commit history, finds the outstanding `rr:override-major` label on #55, and
   still applies it as the floor when creating or updating the release PR.
3. When #55 itself merges later, the floor is applied again (no change to the release PR
   version) and the `rr:override-major` label is removed from #55.

### `!set-version` rejected below an outstanding override

1. `!release major` is posted on feature PR #55 (not yet merged). From the current base version,
   the outstanding floor requires at least `2.0.0`.
2. Someone posts `!set-version 1.3.0` on the open release PR.
3. Release Regent rejects the command instead of pinning the version:

   > ❌ **Release Regent**: `!set-version 1.3.0` was rejected — an `rr:override-major` label is
   > outstanding on PR #55, which requires the release version to be at least `2.0.0`. Use
   > `!set-version 2.0.0` or higher, or remove the `!release major` override first.

4. Post `!set-version 2.0.0` (or higher) instead, or remove the `rr:override-major` label from
   PR #55 first if the override no longer applies.

---

## Next steps

- [PR comment commands reference](../../reference/pr-commands.md) — full syntax and permissions
- [How versions are calculated](../../explanation/version-calculation.md) — understand what the
  automation does before you override it
