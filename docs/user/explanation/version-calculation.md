---
title: How versions are calculated
description: The logic behind semantic version bumps, override floors, and the non-downgrade guarantee
---

# How versions are calculated

Release Regent calculates the next semantic version by analysing the commits that have been
merged since the last release. This article explains the full calculation process, including
how overrides and the non-downgrade guarantee work.

## The base: last released version

The starting point for every calculation is the highest Git tag in the repository that matches
the configured version prefix pattern (e.g., `v*` for the default `"v"` prefix). This tag
determines the "current version".

If no matching tag exists, the calculation starts from `versioning.initial_version` (default
`0.1.0`).

## Step 1: commit analysis

Release Regent reads every commit between the current version tag and the merge commit of the
PR being processed. Each commit is matched against the conventional commit rules:

| Commit prefix or marker | Contribution |
| :--- | :--- |
| `feat!:`, `fix!:`, or any type with `!` | Major bump |
| `BREAKING CHANGE:` footer | Major bump |
| `feat:` | Minor bump |
| `fix:`, `revert:` | Patch bump |
| All other types | No bump |

The contributions are collected and the highest one wins. A set containing one `feat:` and
three `fix:` commits produces a minor bump (not three patch bumps).

## Step 2: override floor (optional)

If `versioning.allow_override = true` and a developer posted `!release major` (or `minor` or
`patch`) on the feature PR before it was merged, the stored level acts as a floor:

- The calculated bump is compared to the override floor.
- If the calculated bump is less than the floor, the floor is used.
- If the calculated bump equals or exceeds the floor, the original calculation stands.

The floor only applies to the single release cycle that consumed the label. It does not persist
to future releases.

## Step 3: apply the bump to the current version

The bump is applied following semantic versioning rules:

| Bump | Effect |
| :--- | :--- |
| **Major** | Increment major, reset minor and patch to 0 |
| **Minor** | Increment minor, reset patch to 0 |
| **Patch** | Increment patch |

**Examples** (current version `1.4.2`):

| Bump | Result |
| :--- | :--- |
| Major | `2.0.0` |
| Minor | `1.5.0` |
| Patch | `1.4.3` |

## Step 4: non-downgrade check

When updating an existing release PR, the newly calculated version is compared to the version
already on the release PR:

- If the new version is **higher**: the release PR is updated to the new version.
- If the new version is **equal**: only the changelog is updated; the version is unchanged.
- If the new version is **lower**: the version is **not** downgraded. Only the changelog is
  appended. A warning is logged.

This guarantee means that once a minor release has been "locked in" by a `feat:` commit, later
`fix:` merges cannot quietly roll it back to a patch release.

## The `!set-version` escape hatch

If the automated calculation produces a version that does not match your team's intent, you can
override it on the release PR using [`!set-version`](../reference/pr-commands.md). This is an
escape hatch for situations such as:

- Marketing has decided to call it `2.0.0` even though the changes are technically backwards
  compatible.
- A breaking change was not annotated correctly in commits and the calculated bump is wrong.
- You want to release `1.3.0-rc.1` before committing to the stable `1.3.0`.

---

## Label persistence (how override floors survive PR merges)

When `!release major` is posted on a PR, Release Regent adds a GitHub label such as
`release-regent:bump:major` to that PR. The label persists until the PR is merged.

When Release Regent processes the merge event, it:

1. Reads all open-and-recently-closed labels on the PR.
2. Extracts any `release-regent:bump:*` labels.
3. Uses the highest bump level found as the floor for the calculation described above.
4. Removes the label after applying it.

Labels are stored on GitHub rather than in a database or file, which means they survive server
restarts and are visible to anyone with repository access.

---

## Related reading

- [Conventional commits reference](../reference/conventional-commits.md)
- [PR comment commands](../reference/pr-commands.md)
- [The release workflow](release-workflow.md)
