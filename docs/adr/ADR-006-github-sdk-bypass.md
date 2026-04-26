# ADR-006: GitHub SDK bypass for raw HTTP calls

Status: Accepted
Date: 2026-04-27
Owners: release-regent-maintainers

## Context

`release-regent-github-client` wraps the `github-bot-sdk` crate.  In two
cases the SDK's higher-level helpers could not be used because their
deserialisation logic is incompatible with what GitHub's API actually returns:

1. **`get_commits_between`** — the SDK's `compare_commits` helper deserialises
   the response into a `FullCommit` type that requires a `comment_count: u32`
   field.  GitHub's compare endpoint does not include this field at the commit
   envelope level (it returns `comments_url` instead), so serde rejects the
   response.

2. **`list_pull_requests` / `search_pull_requests`** — the SDK's `PullRequest`
   type marks `head.repo` and `base.repo` as required (non-`Option`), but
   GitHub returns `null` when a fork repository has been deleted.

Both failures are silent runtime panics / deserialisation errors rather than
compile-time issues, so they were discovered through integration testing.

## Decision

For the two affected methods, **bypass the SDK and issue raw HTTP requests**
directly against the GitHub REST API using the lower-level
`InstallationClient::get` method.  Private local structs (`CompareApiResponse`,
`ListPrItem`, etc.) map exactly to what GitHub actually returns.

All other API operations continue to use the SDK's typed helpers.

## Consequences

- **Enables**: Stable, correct operation for the two affected endpoints without
  waiting for upstream SDK fixes.
- **Risk**: The private deserialisation types must be kept in sync with the
  GitHub API manually.  GitHub rarely makes breaking changes to stable endpoints,
  but fields could be added or renamed in a future API version.
- **Mitigation**: Each bypass is annotated with a comment explaining the reason.
  When the upstream SDK is updated to fix the relevant issues, the bypass should
  be removed and the types should revert to SDK-provided ones.  Track via
  GitHub issues.
- **Scope**: The bypass is limited to `get_commits_between`, `list_pull_requests`,
  and `search_pull_requests` in `crates/github_client/src/lib.rs`.

## Alternatives considered

- **Fork / patch the SDK**: Would require maintaining a fork with upstream
  divergence.  Rejected in favour of the minimal local fix.
- **Contribute upstream**: The correct long-term fix.  Not yet done because the
  upstream SDK is under active development and the fix requires understanding
  their serde strategy.  Tracked as a follow-up.
- **Re-derive response types with `#[serde(default)]`**: This is essentially
  what the bypass does, but at the consumer level rather than modifying the SDK.

## Implementation notes

Bypass locations and their guard comments:

| Location | Endpoint | Reason |
|---|---|---|
| `get_commits_between` | `GET /repos/{owner}/{repo}/compare/{base}...{head}` | SDK `FullCommit` missing `comment_count` |
| `list_pull_requests` | `GET /repos/{owner}/{repo}/pulls` | SDK `PullRequest` non-optional `repo` on branches |
| `search_pull_requests` | `GET /repos/{owner}/{repo}/pulls` (with filters) | Same as above |

To re-enable the SDK path after an upstream fix:

1. Replace the private struct definitions with the SDK types.
2. Replace the raw `installation.get(&path)` calls with the corresponding SDK
   methods.
3. Run the integration test suite with a live sandbox to confirm no regressions.

## References

- `crates/github_client/src/lib.rs` — bypass implementations
- GitHub Compare API: <https://docs.github.com/en/rest/commits/commits#compare-two-commits>
- GitHub Pulls API: <https://docs.github.com/en/rest/pulls/pulls>
