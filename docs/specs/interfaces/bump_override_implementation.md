# Interface Specification: Bump-Override Implementation (ADR-004)

**Status**: Ready for implementation
**ADR**: [ADR-004](../../adr/ADR-004-bump-override-persistence.md)
**Tasks**: 9.20.3, 9.20.4
**Affected files**:

- `crates/core/src/versioning.rs` — new `apply_bump_floor` function, new `SemanticVersion`
  bump helpers
- `crates/core/src/comment_command_processor.rs` — label constants, `!release` handler,
  `!set-version` scope guard
- `crates/core/src/lib.rs` — `handle_merged_pull_request` changes (feature PR path and
  release PR path)

---

## 1. Constants — `comment_command_processor.rs`

Define as `pub const` at module scope (after the public-type section, before the
`CommentCommandProcessor` struct). Alphabetical ordering within the constants block.

```rust
/// Label applied to a feature PR when `!release major` is posted.
///
/// Consumed when the PR is merged to apply a major-version floor.
pub const OVERRIDE_LABEL_MAJOR: &str = "rr:override-major";

/// Label applied to a feature PR when `!release minor` is posted.
///
/// Consumed when the PR is merged to apply a minor-version floor.
pub const OVERRIDE_LABEL_MINOR: &str = "rr:override-minor";

/// Label applied to a feature PR when `!release patch` is posted.
///
/// Consumed when the PR is merged to apply a patch-version floor.
pub const OVERRIDE_LABEL_PATCH: &str = "rr:override-patch";

/// All three override label names, ordered major → minor → patch.
///
/// Used by cleanup logic in `handle_merged_pull_request` when a release PR
/// merges: every open PR carrying any of these labels receives a cleanup
/// comment and has the label removed.
pub const ALL_OVERRIDE_LABELS: &[&str] = &[
    OVERRIDE_LABEL_MAJOR,
    OVERRIDE_LABEL_MINOR,
    OVERRIDE_LABEL_PATCH,
];
```

---

## 2. `SemanticVersion` bump helpers — `versioning.rs`

These are new methods on `SemanticVersion`. They must be inserted into the
`impl SemanticVersion` block, sorted alphabetically with existing methods.

```rust
/// Compute the next major version (x+1.0.0), discarding pre-release and
/// build metadata from the current version.
///
/// # Examples
///
/// ```
/// use release_regent_core::versioning::SemanticVersion;
///
/// let v = SemanticVersion { major: 1, minor: 2, patch: 3,
///                            prerelease: None, build: None };
/// assert_eq!(v.next_major().to_string(), "2.0.0");
///
/// // Pre-release is discarded in the result.
/// let pre = SemanticVersion { major: 1, minor: 0, patch: 0,
///                              prerelease: Some("rc.1".to_string()), build: None };
/// assert_eq!(pre.next_major().to_string(), "2.0.0");
/// ```
#[must_use]
pub fn next_major(&self) -> SemanticVersion {
    SemanticVersion {
        major: self.major + 1,
        minor: 0,
        patch: 0,
        prerelease: None,
        build: None,
    }
}

/// Compute the next minor version (x.y+1.0), discarding pre-release and
/// build metadata from the current version.
///
/// # Examples
///
/// ```
/// use release_regent_core::versioning::SemanticVersion;
///
/// let v = SemanticVersion { major: 1, minor: 2, patch: 3,
///                            prerelease: None, build: None };
/// assert_eq!(v.next_minor().to_string(), "1.3.0");
/// ```
#[must_use]
pub fn next_minor(&self) -> SemanticVersion {
    SemanticVersion {
        major: self.major,
        minor: self.minor + 1,
        patch: 0,
        prerelease: None,
        build: None,
    }
}

/// Compute the next patch version (x.y.z+1), discarding pre-release and
/// build metadata from the current version.
///
/// # Examples
///
/// ```
/// use release_regent_core::versioning::SemanticVersion;
///
/// let v = SemanticVersion { major: 1, minor: 2, patch: 3,
///                            prerelease: None, build: None };
/// assert_eq!(v.next_patch().to_string(), "1.2.4");
/// ```
#[must_use]
pub fn next_patch(&self) -> SemanticVersion {
    SemanticVersion {
        major: self.major,
        minor: self.minor,
        patch: self.patch + 1,
        prerelease: None,
        build: None,
    }
}
```

---

## 3. `apply_bump_floor` — `versioning.rs`

Free function, exported at crate level via `pub`. Place in the free-functions section of
`versioning.rs` (alphabetically, before `latest_semver_tag`).

### Signature

```rust
/// Apply a minimum-bump floor to a calculated semantic version.
///
/// Computes the version that `floor` would produce from `current`
/// (`current.next_major()`, `current.next_minor()`, or `current.next_patch()`),
/// then returns whichever of `calculated` and `floor_version` is the greater
/// according to semver precedence.
///
/// This is a pure function with no I/O and no side effects.
///
/// # Arguments
///
/// * `current` — the highest released version tag at the time the feature PR
///   was merged (the baseline from which the floor is computed).
/// * `calculated` — the version that conventional-commit analysis produced
///   before any floor is applied.
/// * `floor` — the minimum bump dimension requested via `!release`.
///
/// # Returns
///
/// `max(calculated, floor_version)` by semver precedence.
///
/// # Examples
///
/// ```
/// use release_regent_core::comment_command_processor::BumpKind;
/// use release_regent_core::versioning::{apply_bump_floor, SemanticVersion};
///
/// let current    = SemanticVersion { major: 1, minor: 2, patch: 3,
///                                    prerelease: None, build: None };
/// let calculated = SemanticVersion { major: 1, minor: 2, patch: 4, // patch bump
///                                    prerelease: None, build: None };
///
/// // !release major → floor = 2.0.0, which is greater than 1.2.4.
/// let effective = apply_bump_floor(&current, &calculated, BumpKind::Major);
/// assert_eq!(effective.to_string(), "2.0.0");
///
/// // !release patch → floor = 1.2.4, which equals the calculated version.
/// let effective = apply_bump_floor(&current, &calculated, BumpKind::Patch);
/// assert_eq!(effective.to_string(), "1.2.4");
///
/// // If the conventional commits already force a higher version the floor
/// // has no effect.
/// let major_calc = SemanticVersion { major: 2, minor: 0, patch: 0,
///                                    prerelease: None, build: None };
/// let effective  = apply_bump_floor(&current, &major_calc, BumpKind::Minor);
/// assert_eq!(effective.to_string(), "2.0.0");
/// ```
pub fn apply_bump_floor(
    current: &SemanticVersion,
    calculated: &SemanticVersion,
    floor: BumpKind,
) -> SemanticVersion {
    use crate::comment_command_processor::BumpKind;
    use std::cmp::Ordering;

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

### Import note

`apply_bump_floor` imports `BumpKind` from `crate::comment_command_processor`. The
`versioning` module does not currently depend on `comment_command_processor`. The coder
must add that import. Alternatively (and preferably, to avoid a cyclic dependency risk),
the coder may move `BumpKind` to `versioning.rs` and re-export it from
`comment_command_processor` using `pub use`. The current codebase has `BumpKind` in
`comment_command_processor.rs`; if there is no cyclic dependency between the two modules,
the import approach is acceptable.

**Recommendation**: Move `BumpKind` to `versioning.rs` (it is a versioning concept, not
a command-processing concept) and update the import in `comment_command_processor.rs` to
`use crate::versioning::BumpKind;`. The public re-export from `comment_command_processor`
preserves the existing public API without breakage.

---

## 4. `!release` handler — `CommentCommandProcessor` (task 9.20.3)

### Location

`crates/core/src/comment_command_processor.rs`

Replace the stub `CommentCommand::ReleaseBump(_kind)` arm in `process_inner` with a full
implementation. The new private helper `handle_release_bump` is called from that arm.

### Updated match arm

```rust
CommentCommand::ReleaseBump(kind) => {
    self.handle_release_bump(owner, repo, issue_number, kind, &event.event_id)
        .await
}
```

### New private method: `handle_release_bump`

```rust
/// Handle a validated `!release major|minor|patch` command.
///
/// Applies an `rr:override-*` label to the commented-upon PR to persist the
/// minimum-bump intent until the PR is merged. Removes any previously applied
/// override label first (idempotent removes, 404 is ignored).
///
/// Posts a confirmation comment indicating:
/// - Whether this is a fresh override or a replacement.
/// - The bump dimension recorded.
/// - The effect at merge time.
///
/// # Guards
///
/// The guards (allow_override, PR open, commenter permission) have already
/// been enforced by `process_inner` before this method is called.
///
/// # Errors
///
/// - `CoreError::GitHub` / `CoreError::Network` — a GitHub API call failed;
///   propagated so the event loop can retry if transient.
async fn handle_release_bump(
    &self,
    owner: &str,
    repo: &str,
    pr_number: u64,
    kind: BumpKind,
    event_id: &str,
) -> CoreResult<()> {
    let new_label = match kind {
        BumpKind::Major => OVERRIDE_LABEL_MAJOR,
        BumpKind::Minor => OVERRIDE_LABEL_MINOR,
        BumpKind::Patch => OVERRIDE_LABEL_PATCH,
    };

    // Determine which (if any) override label was already applied.
    let existing_labels = self
        .github
        .list_pr_labels(owner, repo, pr_number)
        .await?;

    let previous_kind: Option<BumpKind> = existing_labels.iter().find_map(|l| {
        match l.name.as_str() {
            OVERRIDE_LABEL_MAJOR => Some(BumpKind::Major),
            OVERRIDE_LABEL_MINOR => Some(BumpKind::Minor),
            OVERRIDE_LABEL_PATCH => Some(BumpKind::Patch),
            _ => None,
        }
    });

    // Remove all existing override labels (idempotent; ignore 404).
    for label in ALL_OVERRIDE_LABELS {
        if let Err(e) = self.github.remove_label(owner, repo, pr_number, label).await {
            warn!(
                event_id,
                pr_number,
                label,
                error = %e,
                "Failed to remove existing override label; continuing"
            );
        }
    }

    // Apply the new override label.
    self.github
        .add_labels(owner, repo, pr_number, &[new_label])
        .await?;

    info!(
        event_id,
        pr_number,
        label = new_label,
        "Override label applied"
    );

    // Post confirmation comment.
    let kind_str = match kind {
        BumpKind::Major => "major",
        BumpKind::Minor => "minor",
        BumpKind::Patch => "patch",
    };
    let body = if let Some(prev) = &previous_kind {
        let old_kind_str = match prev {
            BumpKind::Major => "major",
            BumpKind::Minor => "minor",
            BumpKind::Patch => "patch",
        };
        format!(
            "✅ **Release Regent**: `!release {kind_str}` override recorded \
             (replacing previous `!release {old_kind_str}` override). \
             When this PR is merged, the next release version will be bumped \
             by at least one {kind_str} increment."
        )
    } else {
        format!(
            "✅ **Release Regent**: `!release {kind_str}` override recorded. \
             When this PR is merged, the next release version will be bumped \
             by at least one {kind_str} increment."
        )
    };

    self.post_comment(owner, repo, pr_number, &body).await
}
```

---

## 5. `!set-version` scope guard — `CommentCommandProcessor` (task 9.20.3)

### Change summary

Before invoking `resolve_current_version`, fetch the PR via `get_pull_request` and
check that its `head.ref_name` starts with `"{branch_prefix}/v"`. If the branch pattern
does not match, post a rejection comment and return `Ok(())`.

This prevents `!set-version` from being accepted on feature branches, which would allow
the override to be applied before the relevant work is merged and could create a stale
release PR.

### Guard code (insert at the top of `handle_set_version`, before `resolve_current_version`)

```rust
// Guard: !set-version is only accepted when posted on a release PR
// (head branch must start with "{branch_prefix}/v", e.g. "release/v").
// Posting on a feature PR would create a stale override for work that
// may never be merged.
let pr = self.github.get_pull_request(owner, repo, pr_number).await?;
let release_branch_prefix = format!(
    "{}/v",
    self.config.orchestrator_config.branch_prefix
);
if !pr.head.ref_name.starts_with(&release_branch_prefix) {
    let warning = format!(
        "⚠️ **Release Regent**: `!set-version` must be posted on the active \
         release PR (branch `{release_branch_prefix}*`). Please re-post this \
         command on the release PR."
    );
    warn!(
        pr_number,
        head_ref = %pr.head.ref_name,
        branch_prefix = %release_branch_prefix,
        "Rejecting !set-version: not posted on a release PR branch"
    );
    return self.post_comment(owner, repo, pr_number, &warning).await;
}
```

### Updated `handle_set_version` method signature (no change — same parameters)

The PR retrieval for the scope guard replaces the existing `get_pull_request` call that
currently appears later in the method (used to extract `base_branch` and `base_sha`).
The coder should unify these into a single call: fetch the PR once at the top of
`handle_set_version`, use it for the scope guard, then use the same `pr` value for
`base_branch` and `base_sha` extraction. Remove the duplicate `get_pull_request` call.

### Full updated callsite sketch

```rust
async fn handle_set_version(
    &self,
    owner: &str,
    repo: &str,
    pr_number: u64,
    pinned_version: &SemanticVersion,
    correlation_id: &str,
) -> CoreResult<()> {
    // --- NEW: fetch PR once; used for scope guard and branch extraction ---
    let pr = self.github.get_pull_request(owner, repo, pr_number).await?;

    // --- NEW: scope guard ---
    let release_branch_prefix = format!(
        "{}/v",
        self.config.orchestrator_config.branch_prefix
    );
    if !pr.head.ref_name.starts_with(&release_branch_prefix) {
        let warning = format!(
            "⚠️ **Release Regent**: `!set-version` must be posted on the active \
             release PR (branch `{release_branch_prefix}*`). Please re-post this \
             command on the release PR."
        );
        warn!(
            pr_number,
            head_ref = %pr.head.ref_name,
            "Rejecting !set-version: not on a release branch"
        );
        return self.post_comment(owner, repo, pr_number, &warning).await;
    }

    // --- EXISTING: version validation (unchanged) ---
    let current_version = resolve_current_version(self.github, owner, repo, false).await?;
    // ... (existing minimum-version checks) ...

    info!(pr_number, pinned = %pinned_version, "!set-version accepted");

    // --- EXISTING: reuse already-fetched PR for branch info (remove duplicate get_pull_request) ---
    let base_branch = pr.base.ref_name.clone();
    let base_sha = pr.base.sha.clone();

    let orchestrator =
        ReleaseOrchestrator::new(self.config.orchestrator_config.clone(), self.github);

    orchestrator
        .orchestrate(
            owner,
            repo,
            pinned_version,
            "Version pinned via PR comment override.",
            &base_branch,
            &base_sha,
            correlation_id,
        )
        .await
        .map(|_| ())
}
```

---

## 6. `handle_merged_pull_request` changes — `lib.rs` (task 9.20.4)

### Affected method

`ReleaseRegentProcessor::handle_merged_pull_request` in
`crates/core/src/lib.rs`.

### Branch detection

The merged PR's head branch is read from:

```rust
let merged_pr_head_ref = event
    .payload
    .get("pull_request")
    .and_then(|pr| pr.get("head"))
    .and_then(|h| h.get("ref"))
    .and_then(|v| v.as_str())
    .unwrap_or_default()
    .to_string();

// "release/v" is the hardcoded detection prefix; the branch_prefix value
// from OrchestratorConfig is "release" and the separator "/v" follows the
// convention established in the ADR.
let release_branch_prefix = format!(
    "{}/v",
    repo_config.release_pr.branch_prefix   // or construct from OrchestratorConfig
);
let is_release_pr = merged_pr_head_ref.starts_with(&release_branch_prefix);
```

> **Clarification**: `repo_config` does not currently expose `branch_prefix` directly;
> the value is derived when constructing `OrchestratorConfig`. The coder should use the
> same literal `"release/v"` pattern established throughout the codebase, or extract the
> prefix from the same place it is passed to `OrchestratorConfig::branch_prefix`.

The merged PR number for label lookup:

```rust
let merged_pr_number: u64 = event
    .payload
    .get("pull_request")
    .and_then(|pr| pr.get("number"))
    .and_then(serde_json::Value::as_u64)
    .ok_or_else(|| {
        CoreError::invalid_input(
            "payload",
            "PullRequestMerged payload is missing pull_request.number",
        )
    })?;
```

---

### 6A. Feature PR path — apply bump floor

Insert the following block **after** version calculation (`calc_result`) and **before**
the `format_changelog_for_release` call.

```rust
// ── Bump-floor: read override label from the merged feature PR ──────────
//
// A collaborator may have posted `!release major|minor|patch` on this PR
// before it was merged. That command applied an `rr:override-*` label to
// the PR. We read that label here and, if present, raise the calculated
// version to satisfy the requested minimum bump.
let labels = self
    .github_operations
    .list_pr_labels(owner, repo, merged_pr_number)
    .await?;

let floor_kind: Option<BumpKind> = labels.iter().find_map(|l| {
    use comment_command_processor::{BumpKind, OVERRIDE_LABEL_MAJOR, OVERRIDE_LABEL_MINOR, OVERRIDE_LABEL_PATCH};
    match l.name.as_str() {
        OVERRIDE_LABEL_MAJOR => Some(BumpKind::Major),
        OVERRIDE_LABEL_MINOR => Some(BumpKind::Minor),
        OVERRIDE_LABEL_PATCH => Some(BumpKind::Patch),
        _ => None,
    }
});

let effective_version = if let (Some(floor), Some(ref current)) = (floor_kind.as_ref(), &current_version) {
    versioning::apply_bump_floor(current, &calc_result.next_version, floor.clone())
} else {
    calc_result.next_version.clone()
};

tracing::debug!(
    owner = %owner,
    repo = %repo,
    calculated = %calc_result.next_version,
    effective  = %effective_version,
    floor      = ?floor_kind,
    "Resolved effective release version after bump-floor check"
);
```

Then pass `&effective_version` to orchestration instead of `&calc_result.next_version`:

```rust
orchestrator
    .orchestrate(
        owner,
        repo,
        &effective_version,   // ← was &calc_result.next_version
        &changelog,
        &base_branch,
        &base_sha,
        correlation_id,
    )
    .await
```

After the orchestration call returns, if the floor was applied post an audit comment on
the release PR. The release PR number comes from the `OrchestratorResult`:

```rust
let orch_result = orchestrator
    .orchestrate(
        owner,
        repo,
        &effective_version,
        &changelog,
        &base_branch,
        &base_sha,
        correlation_id,
    )
    .await?;

// Post floor-applied audit comment when the effective version differs.
if effective_version != calc_result.next_version {
    use comment_command_processor::BumpKind;
    let kind_str = match floor_kind.as_ref().expect("floor_kind is Some when versions differ") {
        BumpKind::Major => "major",
        BumpKind::Minor => "minor",
        BumpKind::Patch => "patch",
    };
    // Extract the release PR number from the orchestration result.
    let release_pr_number: Option<u64> = match &orch_result {
        release_orchestrator::OrchestratorResult::Created { pr, .. } => Some(pr.number),
        release_orchestrator::OrchestratorResult::Updated { pr } => Some(pr.number),
        release_orchestrator::OrchestratorResult::Renamed { pr } => Some(pr.number),
        release_orchestrator::OrchestratorResult::NoOp { pr } => Some(pr.number),
    };

    if let Some(release_pr) = release_pr_number {
        let audit_body = format!(
            "🔼 **Release Regent**: Version floor applied from `!release {kind_str}` \
             override on PR #{merged_pr_number}. The calculated version was \
             `{calc}` but was raised to `{eff}` to satisfy the requested \
             minimum {kind_str} bump.",
            calc = calc_result.next_version,
            eff  = effective_version,
        );
        if let Err(e) = self
            .github_operations
            .create_issue_comment(owner, repo, release_pr, &audit_body)
            .await
        {
            tracing::warn!(
                error = %e,
                release_pr,
                merged_pr = merged_pr_number,
                "Failed to post bump-floor audit comment; continuing"
            );
        }
    }
}

Ok(orch_result)
```

---

### 6B. Release PR path — stale label cleanup

After the orchestration call on the release PR path, run cleanup for every open feature
PR that still carries an override label. Cleanup failures are `warn!`-logged and must
**not** fail the event.

```rust
// ── Stale override cleanup ───────────────────────────────────────────────
//
// Now that a release has been published, any open feature PRs with
// rr:override-* labels carry stale intent (the release cycle they were
// meant to influence has already closed). Remove the labels and post an
// explanatory comment so the PR author knows they need to re-post their
// `!release` command if the work still warrants a minimum bump in the
// next release.
use comment_command_processor::ALL_OVERRIDE_LABELS;

for &label_name in ALL_OVERRIDE_LABELS {
    let query = format!("is:open label:{label_name}");
    match self
        .github_operations
        .search_pull_requests(owner, repo, &query)
        .await
    {
        Ok(stale_prs) => {
            for stale_pr in stale_prs {
                // Remove the override label (idempotent).
                if let Err(e) = self
                    .github_operations
                    .remove_label(owner, repo, stale_pr.number, label_name)
                    .await
                {
                    tracing::warn!(
                        error = %e,
                        pr = stale_pr.number,
                        label = label_name,
                        "Failed to remove stale override label; continuing"
                    );
                }

                // Determine bump dimension for the comment.
                let kind_str = label_name
                    .strip_prefix("rr:override-")
                    .unwrap_or(label_name);

                let cleanup_body = format!(
                    "ℹ️ **Release Regent**: The `!release {kind_str}` override on \
                     this PR has been cleared because a new release was published \
                     before this PR merged. If the work in this PR still warrants \
                     a minimum bump for the next release, please re-post your \
                     `!release` command."
                );

                if let Err(e) = self
                    .github_operations
                    .create_issue_comment(owner, repo, stale_pr.number, &cleanup_body)
                    .await
                {
                    tracing::warn!(
                        error = %e,
                        pr = stale_pr.number,
                        "Failed to post stale-override cleanup comment; continuing"
                    );
                }
            }
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                label = label_name,
                "Failed to search for PRs with stale override label; continuing"
            );
        }
    }
}
```

### 6C. Complete updated control flow sketch for `handle_merged_pull_request`

```
handle_merged_pull_request(event)
│
├── extract base_branch, base_sha, merged_pr_head_ref, merged_pr_number
├── load repo_config
├── resolve current_version from tags
│
├── build VersionContext, strategy, options
├── calculate calc_result (next_version + changelog entries)
├── format_changelog_for_release → changelog string
│
├── if is_release_pr (merged_pr_head_ref.starts_with("release/v"))
│   │
│   ├── build OrchestratorConfig, orchestrator
│   ├── orchestrate(…, &calc_result.next_version, …) → orch_result
│   │
│   └── stale label cleanup [6B]
│       └── for each label in ALL_OVERRIDE_LABELS:
│           ├── search_pull_requests("is:open label:{label}")
│           └── for each found PR:
│               ├── remove_label (warn on error, continue)
│               └── create_issue_comment cleanup msg (warn on error, continue)
│
└── else (feature PR path)
    │
    ├── list_pr_labels(merged_pr_number) → labels
    ├── find floor_kind from labels
    ├── compute effective_version = apply_bump_floor or calc_result.next_version
    │
    ├── build OrchestratorConfig, orchestrator
    ├── orchestrate(…, &effective_version, …) → orch_result
    │
    └── if effective_version != calc_result.next_version:
        └── create_issue_comment floor audit comment on release PR (warn on error)
```

---

## 7. `BumpKind` placement (cross-cutting concern)

`BumpKind` is currently defined in `comment_command_processor.rs`. It is used by both
`comment_command_processor.rs` (to dispatch commands) and `versioning.rs` (in
`apply_bump_floor`). To avoid a cyclic module dependency:

**Recommended move**: Relocate `BumpKind` to `versioning.rs`. Add a re-export in
`comment_command_processor.rs`:

```rust
// In comment_command_processor.rs
pub use crate::versioning::BumpKind;
```

This keeps the public API stable and makes the semantic home of `BumpKind` clear (it
represents a versioning concept). The coder must update all import paths accordingly.

---

## 8. Structured-logging fields reference

All new log events must use structured fields. Mandatory fields per log site:

| Emitter | Required fields |
|---|---|
| `handle_release_bump` start | `event_id`, `pr_number`, `label` |
| `handle_release_bump` remove existing label | `event_id`, `pr_number`, `label`, `error` (on warn) |
| `handle_release_bump` apply new label | `event_id`, `pr_number`, `label` |
| `handle_set_version` scope-guard rejection | `pr_number`, `head_ref`, `branch_prefix` |
| `handle_merged_pull_request` floor debug | `owner`, `repo`, `calculated`, `effective`, `floor` |
| `handle_merged_pull_request` floor audit comment failure | `error`, `release_pr`, `merged_pr` |
| `handle_merged_pull_request` stale cleanup — search failure | `error`, `label` |
| `handle_merged_pull_request` stale cleanup — remove failure | `error`, `pr`, `label` |
| `handle_merged_pull_request` stale cleanup — comment failure | `error`, `pr` |

---

## 9. Test scenarios required (for coder reference)

### `apply_bump_floor` (unit tests in `versioning_tests.rs`)

| Scenario | Input | Expected |
|---|---|---|
| Floor raises patch → major | current=1.2.3, calc=1.2.4, floor=Major | 2.0.0 |
| Floor raises patch → minor | current=1.2.3, calc=1.2.4, floor=Minor | 1.3.0 |
| Floor has no effect (calc already exceeds) | current=1.2.3, calc=2.0.0, floor=Minor | 2.0.0 |
| Floor equals calculated | current=1.2.3, calc=1.2.4, floor=Patch | 1.2.4 |
| Floor with pre-release current version | current=2.0.0-rc.1, calc=2.0.0, floor=Major | 3.0.0 |

### `handle_release_bump` (unit tests in `comment_command_processor_tests.rs`)

| Scenario | Expected |
|---|---|
| Fresh `!release major` — no existing label | Applies `rr:override-major`; posts fresh confirmation |
| Replacing `!release minor` with `!release major` | Removes `rr:override-minor`, applies `rr:override-major`; posts replacing confirmation |
| Same override reposted (`!release patch` again) | Removes, re-applies `rr:override-patch`; posts replacing confirmation |
| `allow_override = false` | Silently ignored (existing guard) |
| Commenter lacks write permission | Rejection posted (existing guard) |
| `add_labels` API fails | `CoreError::GitHub` propagated |
| `remove_label` fails with network error | `warn!` logged; `add_labels` still called |

### `handle_set_version` scope guard (unit tests in `comment_command_processor_tests.rs`)

| Scenario | Expected |
|---|---|
| PR on `release/v1.2.3` branch | Guard passes; proceeds to version validation |
| PR on `feature/my-feature` branch | Rejection comment posted; `Ok(())` returned |
| PR on `release/some-branch` (no `/v`) | Rejection comment posted; `Ok(())` returned |
| `branch_prefix = "hotfix"` and PR on `hotfix/v2.0.0` | Guard passes |

### `handle_merged_pull_request` feature-PR path (unit tests in `lib_tests.rs`)

| Scenario | Expected |
|---|---|
| Merged PR has `rr:override-major`, calc is minor | `effective = major`; audit comment posted on release PR |
| Merged PR has `rr:override-patch`, calc is minor | `effective = minor` (floor has no effect); no audit comment |
| Merged PR has no override labels | `effective = calc`; no audit comment |
| `list_pr_labels` fails | `CoreError::GitHub` propagated; event retried |
| Audit comment posting fails | `warn!` logged; `Ok(orch_result)` returned |

### `handle_merged_pull_request` release-PR path (unit tests in `lib_tests.rs`)

| Scenario | Expected |
|---|---|
| No open PRs with override labels | No cleanup calls |
| One open PR with `rr:override-major` | Label removed; cleanup comment posted |
| Two open PRs with different override labels | Both cleaned up |
| `search_pull_requests` fails for one label | `warn!` logged; other labels still processed |
| `remove_label` fails for one PR | `warn!` logged; cleanup comment still attempted |
| Cleanup comment fails | `warn!` logged; event still succeeds |

---

## 10. Compilation checklist

Before merging the implementation:

- [ ] `cargo check` passes with no errors
- [ ] `cargo clippy -- -D warnings` passes (pedantic lints active)
- [ ] All new public items have `///` doc comments with `# Examples` where applicable
- [ ] No `unwrap()` or `expect()` in production paths
- [ ] `BumpKind` placement resolved (either moved to `versioning.rs` + re-exported, or
      import direction verified as acyclic)
- [ ] `next_major`, `next_minor`, `next_patch` marked `#[must_use]`
- [ ] `apply_bump_floor` marked `#[must_use]`
- [ ] Tracing spans and structured fields present for all new code paths
- [ ] Tests cover all scenarios in section 9
