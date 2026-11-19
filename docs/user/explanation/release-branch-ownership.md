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

Release Regent does not need write access to the default branch. It updates its release PR by
writing to `release/v*` branches; it does not merge PRs or write to the default branch.

If your repository protects all branches, prefer exempting `release/v*` from the broad rule
and protecting those branches with a separate ruleset. Target the `release/v*` pattern, enable
the creation and update restrictions, and add only the Release Regent GitHub App identity to
that ruleset's bypass list. This lets the App manage release branches without granting it
authority over the default branch.

Granting the App bypass permission on a rule that covers all branches should be a last resort.
GitHub applies that bypass to every branch in the rule's scope; it cannot be limited to a
branch pattern within that rule. The App could therefore push directly to the default branch
and bypass review in every repository where that broad grant applies, an authority Release
Regent does not need.

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
