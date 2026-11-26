# ADR-004: Bump Override Persistence Strategy for `!release` Commands

Status: Accepted
Date: 2026-04-01
Owners: @pvandervelde

**Amendment 2026-11-24:** Issue #211 extended the override floor beyond consumption at the
labelled PR's own merge. The floor from an outstanding `rr:override-*` label is now also
honored: (a) when a later PR merges without its own override label — a commit-history scan
since the last release tag finds the highest outstanding override among PRs already merged in
the current cycle (extends Q1/Q2's "specific PR" scope to a per-cycle scan); (b) as a guard on
`!set-version`, which is now rejected if the pinned version falls below an outstanding override
floor, revising Q4's "no precedence relationship to enforce at runtime" conclusion; and (c) when
a release PR is deleted before merging and a new one is created by a later merge, so the
override is not silently discarded. The label's removal condition DID change as part of this
work: a labelled PR's `rr:override-*` label is no longer removed when that PR itself merges —
it is removed at release-ship time instead, once the commit range for the release that actually
incorporates it has been scanned (see Fix D, `clear_override_labels_from_merged_prs_in_range`
in `crates/core/src/lib.rs`). This is what makes the per-cycle historical scan in (a) possible:
the label must still exist, on the originating PR, for the scan to find it. One consequence of
removal being deferred to release-ship time is that the historical scan only recovers an
override while its label still exists on the originating PR — in practice this covers
missed-webhook recovery and recreated-release-PR scenarios, not every theoretically possible
"release PR deleted after the override already shipped" case, since by the time a release
ships, the label is gone (removed as part of that same release-ship cleanup). See
`resolve_outstanding_override_floor` in `crates/core/src/comment_command_processor.rs` for the
implementation.

## Context

[ADR-003](ADR-003-pr-comment-commands.md) introduced PR comment commands, implementing
`!set-version X.Y.Z` fully and registering `!release major|minor|patch` as a recognised
but unimplemented stub. The stub posts an informational "not yet supported" comment and
acknowledges the event.

Implementing `!release major|minor|patch` raises a fundamental persistence problem:
`!release major` specifies only a *minimum bump direction* — the concrete target version
cannot be determined until the next `PullRequestMerged` event carries the commit history
needed for version calculation.

`!set-version X.Y.Z` specifies a complete, concrete target version and can invoke the
`ReleaseOrchestrator` immediately. However, it shares a different scope problem: if the
command is accepted on any open PR, then posted on a feature PR that is later abandoned,
the release PR has already been changed based on work that was never merged. This is a
stale-override risk identical in effect to the original (rejected) option of storing
`!release` overrides on the release PR — version decisions would be driven by
unmerged work.

Five design questions must be answered before implementation can begin.

### Design Question 1 — Storage medium

Where is the forced-minimum-bump stored between the comment event and the next
`PullRequestMerged` event?

**Options evaluated:**

| Option | Description | Assessment |
|--------|-------------|------------|
| (a) GitHub label on the release PR | `rr:override-major` on the release PR | ❌ Stale if the commented-upon feature PR is abandoned — an override from a PR that was never merged would affect the next arbitrary merge |
| (b) Hidden HTML comment in PR body | Embed structured data in the PR body | ❌ Fragile: manual edits corrupt state; parsing is error-prone; not human-friendly |
| (c) In-process memory | Store in a `HashMap` or similar | ❌ Not viable: lost on any restart or pod rescheduling |
| (d) External state store (DB, Redis) | Persist to a dedicated store | ❌ Adds infrastructure dependency and operational burden; contradicts the GitHub-native design |
| (e) GitHub label on the commented-upon (feature) PR | `rr:override-major` on the PR the comment was posted on | ✅ Override is scoped to that specific PR; if the PR is abandoned, no `PullRequestMerged` fires and the label is never consumed; restart-safe; visible in GitHub UI |

**Decision** (Q1): GitHub label on the **commented-upon PR** (the feature PR, option e).

Label set: `rr:override-major`, `rr:override-minor`, `rr:override-patch`.
At most one override label is active per PR at a time.

**Rationale for rejecting option (a)**: Storing the label on the release PR creates a
coupling between two unrelated concerns. A `!release major` comment on feature PR #55
should only affect orchestration if PR #55 *actually merges*. If PR #55 is closed
without merging, the override intent should be silently discarded. Option (a) cannot
distinguish between "PR #55 merged" and "some other PR merged after #55 was abandoned"
— the release PR label would be consumed by the first subsequent merge regardless.

### Design Question 2 — Scope / lifetime of the override

Does the override apply to any future `PullRequestMerged` event, or only to the specific
PR that the `!release` comment was posted on?

The override is an expression of intent about a specific piece of work: **"when this
PR is merged, ensure the release version is at least a major bump"**. That intent is
inseparably tied to the PR being merged. A PR that is abandoned without merging carries
no relevant intent for future repository activity.

**Decision** (Q2): The override label is consumed when the **specific PR it was posted
on** is merged via `PullRequestMerged`. If that PR is closed without merging, the label
remains on the closed PR but is never read by the orchestration path and has no effect.
No explicit cleanup is required.

### Design Question 3 — Conflict resolution with breaking-change commits

When `BREAKING CHANGE:` commits are present (which implies a required major bump),
does `!release minor` still apply?

`BREAKING CHANGE:` commits establish an *inherent* requirement for a major bump
according to the Conventional Commits specification. The override floor is a *minimum*
constraint — the final effective version is `max(calculated_version, floor_version)`.

| Calculated bump | Override floor | Result |
|----------------|----------------|--------|
| `patch` (1.2.4) | `major` floor → 2.0.0 | 2.0.0 (floor raised it) |
| `minor` (1.3.0) | `patch` floor → 1.2.4 | 1.3.0 (calculated already exceeds floor) |
| `major` (2.0.0) | `minor` floor → 1.3.0 | 2.0.0 (calculated already exceeds floor) |
| `major` (2.0.0) from BREAKING CHANGE | `minor` floor | 2.0.0 (BREAKING CHANGE wins) |

**Decision** (Q3): The override floor is a **minimum floor, not a ceiling**. The
effective version is `max(calculated_version, apply_floor(current_version, floor_kind))`.
`BREAKING CHANGE:` commits always produce a major-version increment regardless of any
floor. A `!release minor` label cannot reduce or override a major bump required by the
commit history.

### Design Question 4 — Precedence when `!set-version` and `!release` coexist

A `!set-version X.Y.Z` command is posted on some PR; a different feature PR has an
`rr:override-*` bump floor label. Which takes precedence?

Because override labels are scoped to individual PRs (Q1, Q2), `!set-version` and
`!release` are evaluated at different times and on potentially different PRs. They do
not directly conflict:

- `!set-version` invokes the orchestrator immediately with the pinned version.
- A subsequent merge of a PR carrying an `rr:override-*` label will apply the floor
  at that merge event, which may update the release PR version further.

This is consistent with normal release flow: any merged PR may change the release
version. An explicit `!set-version 1.5.0` sets the release PR to `1.5.0`; if a feature
PR with `rr:override-major` later merges, the floor may raise the release PR to `2.0.0`.
This is expected and correct — the `!set-version` represented the state of intent at
the time it was posted, not a permanent cap on future versions.

**Decision** (Q4): When both are in play, there is no precedence relationship to enforce
at runtime:

- `!set-version` (now restricted to the release PR, see Q5) invokes the orchestrator
  immediately with the pinned version.
- A subsequent merge of a feature PR carrying an `rr:override-*` label applies the
  floor at that merge event, which may update the release PR version above the pinned
  value. This is expected and correct.

Operators who wish to cancel a pending `rr:override-*` label must remove it manually
from the feature PR in the GitHub UI.

### Design Question 5 — Scope of `!set-version` commands

When a `!set-version X.Y.Z` command is received, the processor must call the
`ReleaseOrchestrator` immediately with the pinned version. But on which PR should
the command be accepted?

If `!set-version` is accepted on any open PR (the original design), and the commented-upon
PR is later abandoned, the release PR has already been changed to a version driven by
unmerged work. This is conceptually the same stale-override risk that motivated the
rejection of option (a) in Q1.

**Options evaluated:**

| Option | Description | Assessment |
|--------|-------------|------------|
| (a) Accept on any open PR | Current design; apply immediately | ❌ Stale-override risk: release PR version can reflect a PR that was never merged |
| (b) Defer to PR merge, store version in label | `rr:set-version:1.5.0` label on feature PR; apply when merged | ❌ Non-standard label naming; encoding semver in a label is fragile and limits label query capabilities; complicates parsing |
| (c) Restrict to the release PR only | Only accept when commented on a PR whose head branch matches `release/v*` | ✅ No stale risk; no deferred state; semantically natural — the release PR is the right place to manage release version |

**Decision** (Q5): `!set-version` is only accepted when posted on the **active release PR**,
identified by head branch matching the `release/v` prefix (i.e. `head.ref_name.starts_with(branch_prefix + "v")`
where `branch_prefix` comes from `OrchestratorConfig`). If the command is posted on any
other open PR, a **scope rejection comment** is posted and the event is acknowledged
without modifying any PR:

> ⚠️ **Release Regent**: `!set-version` must be posted on the active release PR
> (branch `release/v*`). Please re-post this command on the release PR.

**Rationale**: The release PR is the natural place to express "set this release to
exactly version X.Y.Z" — it is the artefact that represents the pending release.
Accepting the command on feature PRs would allow a never-merged PR to permanently
change the release version, which contradicts the principle that version decisions
are only based on merged work.

Note: `get_pull_request` already exists on `GitHubOperations` and `PullRequest.head.ref_name`
is already available, so no new trait methods are required to implement this check.

### Design Question 6 — Stale override labels when a release completes before the feature PR merges

A feature PR may sit open with an `rr:override-*` label for an extended period. During
that time, the active release PR may be merged (publishing a release) before the feature
PR merges. When the feature PR eventually merges, should its override label still be applied?

**The problem**: The override expressed intent for *that release cycle* — the commenter
intended the work in that feature PR to warrant a particular minimum bump. If the release
already happened without including the feature PR, applying the override to the *next*
release cycle is wrong: it would impose a version floor decision made in a prior context
onto a new release that the commenter never evaluated.

**Options evaluated:**

| Option | Description | Assessment |
|--------|-------------|------------|
| (a) Check label timestamp vs release timestamp | Fetch label application time via `GET /issues/{n}/events`; discard if before latest release | ✅ Accurate but requires an extra API call on every merge event that carries an override label |
| (b) Accept the edge case | Let stale overrides apply; document that operators must manually remove labels | ❌ Produces incorrect version bumps based on stale intent; poor UX |
| (c) Clear override labels when the release PR merges | When a release PR (`head: release/v*`) merges, find all open PRs with override labels and remove them | ✅ Proactive; enforces a clean one-cycle scope; each affected PR receives an explanatory comment |

**Decision** (Q6): Clear all outstanding `rr:override-*` labels from open PRs when a
**release PR** is merged (option c).

**Rationale**: Option (c) cleanly enforces the intended scope: overrides are valid for
one release cycle only. Once a release lands, version decisions for that cycle are final.
Feature PRs that were not part of that release must re-evaluate their version intent when
they eventually merge — contributors should re-post `!release` if the work still warrants
a minimum bump. The cleanup is proactive and observable (comment posted), so contributors
understand why their label was removed rather than wondering why the floor was silently
not applied.

**Detection**: Head branch of the merged PR at `payload["pull_request"]["head"]["ref"]`
starts with the configured `branch_prefix + "/v"` (e.g. `release/v*`).

**Cleanup comment posted on each affected open PR:**

> ℹ️ **Release Regent**: The `!release {kind}` override on this PR has been cleared
> because a new release was published before this PR merged. If the work in this PR still
> warrants a minimum bump for the next release, please re-post your `!release` command.

**New `GitHubOperations` capability required**: Finding open PRs that carry a specific
label. Options:

- Extend `search_pull_requests` to support `label:NAME` qualifiers (cleanest — no new
  method, just extend the query parsing).
- Add a dedicated `list_open_prs_with_label(owner, repo, label) -> CoreResult<Vec<PullRequest>>`.

The preferred approach is extending `search_pull_requests` with `label:` support, keeping
the API surface small. This is left to task 9.20.2.

## Decision

Use GitHub labels (`rr:override-major`, `rr:override-minor`, `rr:override-patch`) on
the **commented-upon (feature) PR** as the persistence mechanism for `!release` bump
overrides. The label is applied immediately when the command is received and is consumed
when that specific PR is merged.

`!set-version` is only accepted when posted on the release PR (head branch `release/v*`);
posted elsewhere, a scope rejection comment is returned.

When a **release PR merges**, all outstanding `rr:override-*` labels on open PRs are
cleared with an explanatory comment; those overrides were scoped to the completed release
cycle and must not carry forward.

Processing model:

1. **When `!release major|minor|patch` is received** (`CommentCommandProcessor`):
   - Validate: `allow_override = true`, PR is open, commenter has Write access or above.
   - Remove any existing `rr:override-*` labels from the **feature PR** (the PR the
     comment was posted on).
   - Apply the new label (e.g. `rr:override-major`) to the feature PR.
   - Post a **confirmation comment** on the feature PR:
     > ✅ **Release Regent**: `!release major` override recorded. When this PR is merged,
     > the next release version will be bumped by at least one major increment.
   - Acknowledge the event.

2. **When a PR is merged** (`ReleaseRegentProcessor::handle_merged_pull_request`):

   **Case A — Feature PR** (head branch does NOT start with `release/v`):
   - Read labels from the **merged PR** (`list_pr_labels(merged_pr_number)`).
   - If an `rr:override-*` label is present, compute the floor version:
     - `rr:override-major` → `current_version.next_major()` (e.g. 1.2.3 → 2.0.0)
     - `rr:override-minor` → `current_version.next_minor()` (e.g. 1.2.3 → 1.3.0)
     - `rr:override-patch` → `current_version.next_patch()` (e.g. 1.2.3 → 1.2.4)
   - Compute `effective_version = max(next_version, floor_version)` using
     `SemanticVersion::compare_precedence`.
   - Call `orchestrator.orchestrate(…, &effective_version, …)`.
   - If the floor was applied (i.e. `effective_version != next_version`), post an
     **audit comment** on the resulting release PR:
     > 🔼 **Release Regent**: Version floor applied from `!release {kind}` on PR #{n}
     > by @{login}. Effective version raised from `{next_version}` to `{effective_version}`.
   - No cleanup of the merged PR's label is needed; the PR is now closed.

   **Case B — Release PR** (head branch starts with `release/v`):
   - Perform normal orchestration (version calculation and `orchestrate` call).
   - After orchestration, search for all open PRs bearing any `rr:override-*` label
     (`search_pull_requests` with `is:open label:rr:override-major`, repeated for minor
     and patch, deduplicating results).
   - For each found PR: remove all `rr:override-*` labels; post the cleanup comment:
     > ℹ️ **Release Regent**: The `!release {kind}` override on this PR has been cleared
     > because a new release was published before this PR merged. If the work in this PR
     > still warrants a minimum bump for the next release, please re-post your `!release`
     > command.
   - Log cleanup at `info!` level per affected PR. Treat removal errors as `warn!` and
     continue — the cleanup is best-effort and must not fail the event processing.

3. **When `!set-version X.Y.Z` is received** (`CommentCommandProcessor`):
   - Validate: `allow_override = true`, PR is open, commenter has Write access or above.
   - Call `get_pull_request(owner, repo, issue_number)` to retrieve the PR.
   - If `head.ref_name` does **not** start with `release/v` (the configured branch prefix),
     post a scope rejection comment and acknowledge the event without calling the
     orchestrator:
     > ⚠️ **Release Regent**: `!set-version` must be posted on the active release PR
     > (branch `release/v*`). Please re-post this command on the release PR.
   - If on the release PR: validate `pinned > current_version` → call
     `ReleaseOrchestrator::orchestrate(…, &pinned_version, …)`.
   - No interaction with override labels on other PRs. Any open feature PRs that carry
     `rr:override-*` labels will still apply their floors when those PRs eventually merge.
     This is consistent with normal release flow.

## Consequences

- Override state is visible in the GitHub UI on the **feature PR** that bears the label,
  providing immediate context: any contributor can see that this PR carries a bump intent.
- Override labels on abandoned (closed without merging) PRs are harmless; they are never
  read by the orchestration path.
- Override labels on open PRs are **cleared automatically** when a release is published
  (release PR merged). Contributors receive an explanatory comment; re-posting `!release`
  is required if the intent still applies to the next release.
- State survives pod restarts, deployments, and scheduler preemptions with no additional
  infrastructure.
- GitHub label operations are idempotent: adding an already-present label is a no-op;
  removing an absent label returns 404, which the implementation must treat as `Ok(())`.
- Multiple feature PRs may each carry `rr:override-*` labels simultaneously. Each floor
  is applied independently at the time its PR merges. The orchestrator's natural version
  comparison logic ensures the highest version wins over time.
- Three audit trail mechanisms: confirmation comment on the feature PR when the override is
  recorded; audit comment on the release PR when the floor is actually applied; cleanup
  comment on each open feature PR when a release is published.
- API call budget for `handle_merged_pull_request` increases by **one** call per feature
  PR merge event that carries an override label (`list_pr_labels(merged_pr_number)`); and
  by up to three search calls + N remove/comment calls per release PR merge event (where
  N is the number of open PRs with stale override labels).
- Four new `GitHubOperations` trait capabilities are required: `add_labels`, `remove_label`,
  `list_pr_labels`, and `label:` filter support in `search_pull_requests`.
- Commands from users with insufficient permission produce a `❌` rejection comment
  identifying the commenter and explaining the permission requirement. This ensures
  contributors understand why their command was not processed rather than seeing silence.

## Alternatives considered

- **Label on the release PR**: Rejected — creates stale override risk: a `!release major`
  comment on an abandoned PR would apply its floor to the next arbitrary merge, regardless
  of which PR actually delivered the work that warranted the override. Version decisions
  should only be based on merged work.
- **Embed override in PR body (hidden HTML comment)**: Rejected — fragile to manual
  edits, requires custom parsing, and is not machine-idempotent.
- **Eagerly create a release PR when `!release` is received**: Rejected — requires calling
  the version calculator from the comment processor, which lacks the commit history
  needed to generate a useful changelog.
- **Persist to external store**: Rejected — introduces external dependencies and
  operational burden; contradicts the GitHub-native design philosophy.
- **Check label timestamp for staleness (Q6, option a)**: Rejected in favour of proactive
  cleanup on release PR merge — the timestamp approach requires an extra API call on every
  feature PR merge that carries an override label, and the intent is already served by
  cleaning up at the point of release.

## Implementation notes

### New `GitHubOperations` methods

```rust
/// Add one or more labels to an issue or pull request.
///
/// If a label is already present the operation is a no-op (idempotent).
async fn add_labels(
    &self,
    owner: &str,
    repo: &str,
    issue_number: u64,
    labels: &[String],
) -> CoreResult<()>;

/// Remove a single label from an issue or pull request.
///
/// If the label is not present, returns `Ok(())` (idempotent).
async fn remove_label(
    &self,
    owner: &str,
    repo: &str,
    issue_number: u64,
    label_name: &str,
) -> CoreResult<()>;

/// List all labels currently applied to an issue or pull request.
async fn list_pr_labels(
    &self,
    owner: &str,
    repo: &str,
    issue_number: u64,
) -> CoreResult<Vec<Label>>;
```

`search_pull_requests` must additionally support a `label:NAME` qualifier so open PRs
with override labels can be found when a release PR merges:

```
// Example query used during release PR merge cleanup:
"is:open label:rr:override-major"
```

### Label type

Add `Label` to the `github_operations` module:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub id: u64,
    pub name: String,
    pub color: String,
    pub description: Option<String>,
}
```

### Override floor computation helper

Add a free function in `crates/core/src/versioning.rs`:

```rust
/// Apply a minimum bump-kind floor to a calculated next version.
///
/// Returns `max(calculated, floor_version)` using semver precedence ordering.
/// `current` is the latest released version; it is used to derive the floor
/// target (e.g. next major of `1.2.3` is `2.0.0`).
pub fn apply_bump_floor(
    current: &SemanticVersion,
    calculated: &SemanticVersion,
    floor: BumpKind,
) -> SemanticVersion {
    let floor_version = match floor {
        BumpKind::Major => current.next_major(),
        BumpKind::Minor => current.next_minor(),
        BumpKind::Patch => current.next_patch(),
    };
    match calculated.compare_precedence(&floor_version) {
        Ordering::Less => floor_version,
        _ => calculated.clone(),
    }
}
```

### Label constants

Define in `crates/core/src/comment_command_processor.rs`:

```rust
pub const OVERRIDE_LABEL_MAJOR: &str = "rr:override-major";
pub const OVERRIDE_LABEL_MINOR: &str = "rr:override-minor";
pub const OVERRIDE_LABEL_PATCH: &str = "rr:override-patch";

pub const ALL_OVERRIDE_LABELS: &[&str] = &[
    OVERRIDE_LABEL_MAJOR,
    OVERRIDE_LABEL_MINOR,
    OVERRIDE_LABEL_PATCH,
];
```

### GitHub label creation

Override labels (`rr:override-major` etc.) must be created in the target repository
before they can be applied. The implementation should call `add_labels` with the label
name; if GitHub responds with 422 (label does not exist), it must first create the label
via a `create_label` API call. Add `create_label` to `GitHubOperations` or handle this
transparently inside `add_labels` in the concrete `GitHubClient` implementation. This
detail is left to task 9.20.2.

### `remove_label` 404 handling

GitHub returns HTTP 404 when attempting to remove a label that is not present.
Concrete implementations of `remove_label` must map this to `Ok(())` (idempotent).

### Extracting the PR number for a `PullRequestMerged` event

`handle_merged_pull_request` reads the merged PR number from the webhook payload:

```rust
let merged_pr_number: u64 = event
    .payload
    .get("pull_request")
    .and_then(|pr| pr.get("number"))
    .and_then(serde_json::Value::as_u64)
    .ok_or_else(|| CoreError::invalid_input("payload", "missing pull_request.number"))?;
```

This number is passed to `list_pr_labels` to read override labels from the merged PR.

### Confirmation comment body (feature PR)

When a `!release` command is accepted:

> ✅ **Release Regent**: `!release {kind}` override recorded. When this PR is merged,
> the next release version will be bumped by at least one {kind} increment.

If the commenter replaces a previous `!release` label on the same PR:

> ✅ **Release Regent**: `!release {kind}` override recorded (replacing previous
> `!release {old_kind}` override). When this PR is merged, the next release version
> will be bumped by at least one {kind} increment.

### Audit comment body (release PR)

When the floor is applied during `handle_merged_pull_request`:

> 🔼 **Release Regent**: Version floor applied from `!release {kind}` override on PR #{n}.
> The calculated version was `{next_version}` but was raised to `{effective_version}` to
> satisfy the requested minimum {kind} bump.

### Cleanup comment body (open feature PR after release PR merges)

When override labels are cleared because a release was published:

> ℹ️ **Release Regent**: The `!release {kind}` override on this PR has been cleared
> because a new release was published before this PR merged. If the work in this PR still
> warrants a minimum bump for the next release, please re-post your `!release` command.

## Examples

### Scenario: release PR merges before feature PR with override (stale label cleared)

```
1. Current released version: v1.2.3
2. Contributor posts `!release major` on feature PR #55 (open)
   → rr:override-major applied to PR #55; confirmation comment posted
3. PR #56 merges (unrelated, minor feature; no override label)
   → handle_merged_pull_request for PR #56 (head: feat/ui-update)
   → Reads labels from PR #56 → none
   → next_version = 1.3.0 (minor from commits)
   → orchestrate → release PR created at release/v1.3.0
4. Release PR (head: release/v1.3.0) is merged into main
   → handle_merged_pull_request for the release PR
   → Normal orchestration runs
   → Post-orchestration: search for open PRs with rr:override-* labels → finds PR #55
   → Removes rr:override-major from PR #55
   → Posts cleanup comment on PR #55
5. PR #55 later merges (fix commit)
   → handle_merged_pull_request reads labels from #55 → none (label was cleared)
   → next_version = 1.3.1 (patch)
   → orchestrate with 1.3.1 → no floor applied (correct)
   (Contributor must re-post !release major if the work still warrants a major bump)
```

### Scenario: override lifts patch bump to major

```
1. Current released version: v1.2.3 (from latest semver tag)
2. Contributor posts `!release major` on feature PR #55 (open, not yet merged)
   → CommentCommandProcessor removes any existing rr:override-* labels from PR #55
   → Applies `rr:override-major` label to PR #55
   → Posts confirmation comment on PR #55
3. PR #55 is merged (contains only a fix commit)
   → handle_merged_pull_request reads labels from merged PR #55
   → Finds rr:override-major
   → Calculates next_version = 1.2.4 (patch from commits)
   → Computes floor: next_major(1.2.3) = 2.0.0
   → effective_version = max(1.2.4, 2.0.0) = 2.0.0
   → Calls orchestrate(..., &2.0.0, ...)
   → Orchestrator creates/renames release PR to release/v2.0.0
   → Posts audit comment on the release PR
   (PR #55 label needs no cleanup; it is now merged and closed)
```

### Scenario: PR with override is abandoned (no effect)

```
1. Current released version: v1.2.3
2. Contributor posts `!release major` on PR #55
   → rr:override-major applied to PR #55
   → Confirmation comment posted on PR #55
3. PR #55 is closed without merging
   → No PullRequestMerged event fires
   → rr:override-major on PR #55 is never read by orchestration
   → PR #56 merges (unrelated, patch only)
   → handle_merged_pull_request reads labels from PR #56 → none
   → next_version = 1.2.4 (patch)
   → orchestrate with 1.2.4 → no floor applied (correct)
```

### Scenario: existing release PR already at higher version

```
1. Current released version: v1.2.3
2. A previous PR already raised the release PR to v2.0.0
3. Contributor posts `!release major` on PR #66
   → rr:override-major applied to PR #66
4. PR #66 merges (fix commit)
   → handle_merged_pull_request reads labels from #66 → rr:override-major
   → next_version = 1.2.4 (patch from commits)
   → floor = next_major(1.2.3) = 2.0.0
   → effective_version = max(1.2.4, 2.0.0) = 2.0.0
   → orchestrate: existing PR is already 2.0.0 → NoOp
   → No audit comment needed (floor did not change effective_version vs existing PR)
```

### Scenario: `!set-version` on a feature PR is rejected

```
1. Contributor opens feature PR #70 (not a release PR; head branch = `feat/my-feature`)
2. Contributor posts `!set-version 2.0.0` on PR #70
   → CommentCommandProcessor checks: allow_override ✓, PR open ✓, Write access ✓
   → Calls get_pull_request(owner, repo, 70) → head.ref_name = "feat/my-feature"
   → "feat/my-feature" does not start with "release/v"
   → Posts scope rejection comment on PR #70:
     "⚠️ Release Regent: `!set-version` must be posted on the active release PR..."
   → Event acknowledged; no orchestrator call; no PR modified
```

### Scenario: `!set-version` followed by a PR with an override label

```
1. Current released version: v1.2.3
2. Contributor posts `!release major` on feature PR #55 → rr:override-major on #55
3. Contributor posts `!set-version 1.5.0` on the release PR (head branch: release/v1.3.0)
   → CommentCommandProcessor checks: allow_override ✓, PR open ✓, Write access ✓
   → Calls get_pull_request(owner, repo, release_pr_number) → head.ref_name = "release/v1.3.0"
   → "release/v1.3.0" starts with "release/v" ✓
   → Validates: 1.5.0 > 1.2.3 ✓
   → orchestrate(..., &1.5.0, ...) → release PR renamed to 1.5.0
   (PR #55 still has rr:override-major; !set-version does not touch it)
4. PR #55 is later merged (fix commit)
   → handle_merged_pull_request reads labels from #55 → rr:override-major
   → next_version = 1.5.1 (patch from #55 commits)
   → floor = next_major(1.2.3) = 2.0.0
   → effective_version = max(1.5.1, 2.0.0) = 2.0.0
   → orchestrate: existing PR (1.5.0) < effective (2.0.0) → Renamed
   → Audit comment posted on release PR
```

This is expected and consistent: each merge event re-evaluates the release version.

## References

- [ADR-003: PR Comment Commands](ADR-003-pr-comment-commands.md)
- `docs/specs/requirements/functional-requirements.md` — DR-3, DR-4, DR-5
- `docs/specs/requirements/user-stories.md` — US-4
- `docs/specs/testing/behavioral-assertions.md` — BA-19 through BA-29
- GitHub REST API: Labels on Issues/PRs — `POST /repos/{owner}/{repo}/issues/{issue_number}/labels`
- GitHub REST API: Remove label — `DELETE /repos/{owner}/{repo}/issues/{issue_number}/labels/{name}`
- Conventional Commits specification: <https://www.conventionalcommits.org/>
