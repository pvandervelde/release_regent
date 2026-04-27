# ADR-005: Release Regent owns release branches exclusively

Status: Accepted
Date: 2026-04-27
Owners: release-regent-maintainers

## Context

Release Regent automates the creation and maintenance of release pull requests.
As part of this workflow it pushes a single commit to a release branch and opens
(or updates) a PR targeting the repository's default branch.

A question arose: what should happen when a developer manually pushes commits to
an existing release branch between two Release Regent runs?

Two approaches were considered:

1. **Merge** — preserve the manual commits by rebasing or merging on top of them.
2. **Force-replace** — discard the manual commits and replace the branch tip with
   the canonical Release Regent commit.

## Decision

Release Regent **force-pushes** its generated commit to the release branch
(`force_update_branch`).  Release branches are considered owned exclusively by
Release Regent; manual commits pushed directly to a release branch are
intentionally discarded.

## Consequences

- **Enables**: The PR diff is always exactly one commit authored by Release
  Regent. This keeps the changelog and version-bump audit trail clean and
  deterministic.
- **Forbids**: Developers cannot land hand-crafted changes on a release branch by
  pushing directly. All changes must go through feature branches and be merged to
  the default branch so that the next Release Regent run picks them up via
  conventional-commit analysis.
- **Trade-off accepted**: Any work-in-progress commits on a release branch will
  be silently lost on the next Release Regent run. This is the intended behaviour;
  it is documented here and in the operator guide so that teams understand the
  contract before adopting Release Regent.

## Alternatives considered

- **Merge/rebase strategy**: Would preserve manual commits but makes the PR diff
  unpredictable and breaks the invariant that the release branch always reflects
  exactly what Release Regent computed. Rejected.
- **Error on dirty branch**: Abort the run when unexpected commits are detected.
  Would require a human to clean up before Release Regent can proceed, increasing
  toil. Rejected.

## Implementation notes

The force-push is implemented in `release_orchestrator.rs` via
`GitHubOperations::force_update_branch`.  The method sets the branch ref
unconditionally using the GitHub Refs API (`PATCH /repos/{owner}/{repo}/git/refs/heads/{branch}`
with `"force": true`).

Operators who need to preserve commits on a release branch should merge those
changes to the default branch first, then let Release Regent generate a fresh
release PR in the next run.

## References

- GitHub Refs API: <https://docs.github.com/en/rest/git/refs>
- ADR-001: Hexagonal architecture (context for how git operations are abstracted)
