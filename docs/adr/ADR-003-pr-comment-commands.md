# ADR-003: PR Comment Commands for Version Override

Status: Accepted
Date: 2026-03-28
Owners: @pvandervelde

## Context

Release Regent derives the next release version from conventional commit messages.
Teams occasionally need to override this calculation — for example, to skip a version
number, produce a first release at a non-default version (e.g. `1.0.0` instead of
`0.1.0`), or correct a mistaken tag.

Alternative override mechanisms considered:

- **Git tags**: Creating a tag manually before merge works but is invisible, hard to
  track, and bypasses the PR review process.
- **Repository labels**: Labels are durable but have no argument (cannot express
  `set-version 2.0.0` without dynamic label management).
- **Separate webhook endpoint / slash-command service**: Adds operational complexity;
  the GitHub App already receives `issue_comment` webhooks.
- **PR description keywords**: Parsed once at merge time; hard to update after the PR
  is opened.
- **Environment variables / config file edits**: Requires an additional commit inside
  the PR branch.

PR comments are the lowest-friction option: they are visible, auditable, can be
amended, and arrive as `issue_comment` webhooks already processed by the event loop.

## Decision

Accept and process `!set-version X.Y.Z` and `!release major|minor|patch` command
prefixes from PR comments.  Commands are:

1. **Case-insensitive** on the command keyword; semver pre-release/build identifiers
   are preserved as-is (e.g. `2.0.0-RC.1` is not lowercased).
2. **Line-anchored**: the command must appear at the start of a trimmed line, allowing
   commands to be embedded in longer comment bodies without polluting free-form text.
3. **Gated** by `VersioningConfig::allow_override = true`; when `false` all PR comment
   events are silently acknowledged with no GitHub API calls.
4. **Restricted to write-or-higher collaborators**: the commenter's GitHub permission
   level is checked via `GET /repos/{owner}/{repo}/collaborators/{username}/permission`
   before any action is taken.  Users with `read`, `triage`, or `none` permission
   receive a rejection comment explaining the access requirement.
5. **Only processed on open PRs**: comments on closed or merged PRs are silently
   ignored to prevent stale comments from triggering spurious version changes.
6. **`!set-version`** validates the pinned version is strictly greater than the current
   released version (or ≥ `0.0.1` for first releases); validation failures post a
   rejection comment and acknowledge the event.
7. **`!release major|minor|patch`** is a recognised but not-yet-implemented stub; it
   posts an informational "not yet supported" comment and acknowledges the event.
   Full implementation is deferred pending the bump-override label design (see
   `CommentCommand::ReleaseBump` in `comment_command_processor.rs`).
8. **Rejection and informational comment posting is best-effort**: if `create_issue_comment`
   returns an error it is logged as a warning and the event is still acknowledged
   (not retried).

## Consequences

- Any GitHub App user with write access or above can influence the release version of
  any repository the App is installed on, subject to the `allow_override` flag.
- No persistent state is written for a command: `!set-version` triggers
  `ReleaseOrchestrator` immediately.  Idempotency is provided by the orchestrator's
  existing branch-existence check.
- The `allow_override` flag provides an opt-in safety valve; repositories that do not
  need manual version overrides can leave it `false` and no comment will ever trigger
  a release action.
- Comment posting failures are silent from the user's perspective (event is
  acknowledged, no retry).  Operators must monitor warning logs to detect persistent
  GitHub API issues.

## Alternatives considered

- **Allowlist of authorized users in config**: More explicit control, but adds
  operational burden (keeping the list current) and blocks new maintainers.  The
  GitHub collaborator permission model is already managed through the repository and
  reflects the team structure in real time.
- **Require structured comment syntax (e.g. YAML front matter)**: More extensible for
  future complex commands but adds unnecessary friction for the common single-line
  override case.
- **Store command intent in a PR label**: Labels persist after the PR is merged, which
  could interfere with other label-based automation.  Labels also lack type-safe
  arguments.

## Implementation notes

- `CollaboratorPermission::can_issue_commands()` returns `true` for `Admin`,
  `Maintain`, and `Write`.  `Triage` and `Read` are treated as insufficient.
- `post_comment` swallows GitHub API errors (with a `warn!` log) so posting failures
  do not cause retries of the comment event.
- `parse_comment_command` is a pure free function to keep it independently testable.
- The changelog field passed to `ReleaseOrchestrator::orchestrate` for `!set-version`
  is the placeholder string `"Version pinned via PR comment override."`; the
  orchestrator renders this as the PR body.
- The `CommentCommandProcessor` is a domain component in the `core` crate; it calls
  `GitHubOperations` ports and contains no HTTP client code.

## Examples

Pin to a specific version:

```text
!set-version 2.0.0
```

Force a minimum bump (stub — not yet active):

```text
!release major
```

Both may appear anywhere in the comment body as long as the command starts at the
beginning of a trimmed line.

## References

- [ADR-001: Hexagonal Architecture](ADR-001-hexagonal-architecture.md)
- GitHub REST API: `GET /repos/{owner}/{repo}/collaborators/{username}/permission`
- Conventional Commits specification: <https://www.conventionalcommits.org/>
