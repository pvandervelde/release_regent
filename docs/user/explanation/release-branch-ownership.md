---
title: Release branch ownership
description: Why Release Regent takes exclusive ownership of release/v* branches
---

# Release branch ownership

Release Regent takes **exclusive ownership** of any branch whose name matches the
`release/v*` pattern. This page explains what that means in practice and why the design works
this way.

## What exclusive ownership means

When Release Regent creates a `release/v1.2.0` branch, it is the only actor that should
push commits to, rename, or delete that branch. In particular:

- **Do not push commits** to a `release/v*` branch directly. Release Regent may overwrite or
  discard them when it next updates the release PR.
- **Do not delete the branch** manually while the release PR is open. Deleting it causes the
  PR to close and the release cycle to be lost.
- **Do not rename the branch** manually. Release Regent identifies the release PR by branch
  name pattern; renaming it breaks that identification.

If you need to change the version, use the [`!set-version` command](../reference/pr-commands.md)
on the release PR. Release Regent will handle the branch rename.

## Why this design?

### Simpler reasoning

When a branch is owned by exactly one actor, the state of that branch is predictable. There is
no ambiguity about who last updated it or whether a manual push was intentional.

### Conflict-free automation

Release Regent needs to be able to update release branches as new commits are merged to the
default branch. If developer commits could arrive on the release branch at any time, Release
Regent would need a merge strategy that could produce conflicts. Exclusive ownership sidesteps
the problem entirely.

### Deliberate release content

Part of the value of the release PR is that it captures a clean picture of what is going in the
release: the automatically generated changelog and the manifest file diffs. Commits pushed
directly to the release branch would appear in the merge without going through the version
calculation, which could produce incorrect changelogs or version numbers.

If you need a change to appear in the release, merge it to the default branch first. Release
Regent will pick it up and update the release PR.

## Branch protection settings

If your repository uses branch protection rules, you do not need to protect `release/v*`
branches specifically — Release Regent manages their lifecycle automatically. However, if you
have a rule that protects all branches (e.g., "require pull request before merging"), ensure
that the GitHub App has the bypass permission, or that the `release/v*` branches are exempt.

## What happens to the branch after the release

After the release PR is merged and the GitHub release is published, Release Regent
**deletes the `release/v*` branch**. This is intentional. The branch served its purpose
as a staging area for the release; the release itself is preserved as a tag and GitHub
release. Keeping the branch would add noise to the repository's branch list.

---

## Related reading

- [The release workflow](release-workflow.md)
- [Override the version](../how-to/releases/override-version.md)
- [Recover from a failed release](../how-to/releases/recover-failed-release.md)
