---
title: The release workflow
description: The two-phase approach to automated releases and why it is designed this way
---

# The release workflow

Release Regent implements a **two-phase automated release workflow** that separates release
preparation from release publication. Understanding why it works this way helps you use it
more effectively and troubleshoot unexpected behaviour.

## The problem: automation vs. control

Fully automated releases — where every merged commit immediately triggers a published release —
work well in some organisations, but many teams want a bit more control:

- The changelog content should be reviewable before it is public.
- The calculated version should be verifiable — did that change really warrant a minor bump?
- Releases should happen at a predictable point that the team can coordinate around.

Manual releases solve all of this but are tedious: someone has to remember to calculate the
version, write the changelog, create the tag, and publish the release — every time.

Release Regent sits in the middle. It automates everything that is mechanical while preserving
the moment of human decision: the merge of the release PR.

## Phase 1: release PR management

Every time a regular pull request is merged to your default branch, Release Regent:

1. Reads all commits since the last release.
2. Applies [conventional commit rules](../reference/conventional-commits.md) to determine the
   required version bump.
3. Generates a changelog grouped by commit type.
4. Creates or updates a **release PR** targeting your default branch from a
   `release/v<version>` branch.

The release PR is the artefact you review. Its title shows the calculated version; its body
shows the generated changelog and any manifest file diffs.

### Updating an existing release PR

If a release PR is already open when new commits are merged, Release Regent decides how to
update it based on version comparison:

| Situation | What happens |
| :--- | :--- |
| New version is higher than current PR version | PR is updated: branch renamed, title updated, changelogs merged |
| New version equals current PR version | Changelog is updated; version and branch unchanged |
| New version is lower than current PR version | Version is not downgraded; only the changelog is appended |

This **non-downgrade guarantee** means that once a minor version has been "scheduled" by a
`feat:` commit, a subsequent `fix:` commit cannot silently roll it back to a patch release.

### When no release PR is created

If every commit since the last release is non-version-bumping (`docs:`, `chore:`, `style:`,
etc.), no release PR is created. Release Regent logs the decision and waits for a version-
bumping commit.

## Phase 2: release creation

When the release PR is merged:

1. Release Regent detects the merge by recognising the `release/v*` branch pattern.
2. It extracts the version from the branch name.
3. It creates a Git tag pointing to the merge commit.
4. It publishes a GitHub release using the PR body as release notes.
5. It deletes the `release/v*` branch.

The release is published at the exact commit that the team approved by merging the PR —
no more, no less.

## The role of the release PR

The release PR serves several purposes:

**Visibility**: Every developer can see what the next release will contain and when it is
expected.

**Opportunity for correction**: If the version looks wrong, a team member can use
[`!set-version`](../reference/pr-commands.md) to correct it before it goes out.

**Traceability**: The PR is a permanent record of what was included, who approved it, and
when. CI status checks can be required before merging.

**Version lock**: Once the release PR is opened, the version is visible to the team. No
further merges can downgrade it.

## Branch naming

Release Regent uses a deterministic naming scheme for release branches:

| Pattern | Example | When used |
| :--- | :--- | :--- |
| `release/v<major>.<minor>.<patch>` | `release/v1.2.0` | Standard release |
| `release/v<version>-<timestamp>` | `release/v1.2.0-20250506T143052Z` | Branch name conflict |
| `release/v<version>-<prerelease>` | `release/v1.2.0-beta.1` | Pre-release |

The branch name is an implementation detail — the version that matters is in the PR title and
tag. The timestamp suffix is only ever added to avoid collisions; it does not appear in the
published release.

## Concurrent merges

If multiple PRs are merged in quick succession, Release Regent processes the resulting webhook
events sequentially. The release PR is updated each time with the freshest version calculation.
Only one release PR exists at a time per repository.

## Rate limiting and retries

All GitHub API calls use exponential backoff and respect GitHub's rate limit headers. If a
call fails transiently (network error, 5xx from GitHub), it is retried automatically. If
retries are exhausted, the error is logged with enough context to redeliver the webhook
manually.

---

## Related reading

- [How versions are calculated](version-calculation.md)
- [Release branch ownership](release-branch-ownership.md)
- [Recover from a failed release](../how-to/releases/recover-failed-release.md)
