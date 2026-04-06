# Interface Additions: `GitHubOperations` — ADR-004 bump-override support

**Status**: Ready for implementation
**ADR**: [ADR-004](../../adr/ADR-004-bump-override-persistence.md)
**Layer**: Core domain — `crates/core/src/traits/github_operations.rs`
**Tasks**: 9.20.2

---

## Overview

This document specifies the three new methods added to the `GitHubOperations` trait and
the new `Label` data type required by the bump-override feature. It also describes the
extension to the existing `search_pull_requests` method.

These interfaces are consumed by:

- `CommentCommandProcessor::handle_release_bump` — applies/removes override labels
- `ReleaseRegentProcessor::handle_merged_pull_request` — reads labels on the merged PR
  and cleans up stale labels after a release PR merges

---

## 1. New type: `Label`

Added alongside the other data types in `github_operations.rs`.

```rust
/// A GitHub label applied to an issue or pull request.
///
/// Returned by [`GitHubOperations::list_pr_labels`].
///
/// # GitHub API reference
///
/// `GET /repos/{owner}/{repo}/issues/{issue_number}/labels`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    /// GitHub's internal numeric label identifier.
    pub id: u64,
    /// Display name of the label (e.g. `"rr:override-major"`).
    pub name: String,
    /// Six-character hex colour code without the leading `#` (e.g. `"e11d48"`).
    pub color: String,
    /// Optional human-readable description of the label's purpose.
    pub description: Option<String>,
}
```

### Placement

Insert `Label` alphabetically with the other structs in the data-types section of
`github_operations.rs` (after `GitUser`, before `PullRequest`).

---

## 2. New trait method: `add_labels`

### Signature

```rust
/// Add one or more labels to an issue or pull request.
///
/// The operation is idempotent: if a label is already present on the
/// issue/PR it is **not** added a second time and no error is returned.
///
/// # Parameters
/// - `owner`: Repository owner name
/// - `repo`: Repository name
/// - `issue_number`: Issue or pull request number
/// - `labels`: Slice of label name strings to add
///
/// # Returns
/// `Ok(())` on success (whether or not labels were already present).
///
/// # Errors
/// - `CoreError::NotFound` — the issue/PR does not exist
/// - `CoreError::GitHub` — the API call failed for any other reason
///
/// # GitHub API
///
/// `POST /repos/{owner}/{repo}/issues/{issue_number}/labels`
///
/// JSON body: `{ "labels": ["rr:override-major"] }`
///
/// GitHub returns `200 OK` with the full updated label list. A 404 means
/// the issue/PR does not exist and must be surfaced as `CoreError::NotFound`.
/// An attempt to add a label that does not exist in the repository returns
/// `422 Unprocessable Entity`; callers must create labels before use.
async fn add_labels(
    &self,
    owner: &str,
    repo: &str,
    issue_number: u64,
    labels: &[&str],
) -> CoreResult<()>;
```

### Idempotency contract

The GitHub Labels API returns the full label list on success regardless of duplicates.
Implementations **must not** return an error when a label is already applied.

---

## 3. New trait method: `remove_label`

### Signature

```rust
/// Remove a single label from an issue or pull request.
///
/// The operation is idempotent: if the label is not present (GitHub returns
/// `404 Not Found` for the label resource), `Ok(())` is returned rather
/// than an error.
///
/// # Parameters
/// - `owner`: Repository owner name
/// - `repo`: Repository name
/// - `issue_number`: Issue or pull request number
/// - `label_name`: Exact name of the label to remove
///
/// # Returns
/// `Ok(())` on success or when the label is not currently applied.
///
/// # Errors
/// - `CoreError::NotFound` — the issue/PR itself does not exist (PR 404, not
///   label 404; distinguish via the response body or endpoint).
/// - `CoreError::GitHub` — the API call failed for any other reason.
///
/// # GitHub API
///
/// `DELETE /repos/{owner}/{repo}/issues/{issue_number}/labels/{name}`
///
/// - `204 No Content` — label removed successfully.
/// - `404 Not Found` — label is not on the issue/PR; treat as `Ok(())`.
/// - `410 Gone` — the issue/PR itself no longer exists; map to
///   `CoreError::NotFound`.
///
/// # Implementation note
///
/// Percent-encode the label name in the URL path segment. The colon in
/// `rr:override-major` must be encoded as `%3A`.
async fn remove_label(
    &self,
    owner: &str,
    repo: &str,
    issue_number: u64,
    label_name: &str,
) -> CoreResult<()>;
```

### 404 ambiguity

GitHub uses `404` for both "label not on this PR" and "PR does not exist". The
implementation **must** distinguish these cases — typically by inspecting the
response body or by checking whether the PR exists beforehand. Only a "PR does
not exist" 404 should map to `CoreError::NotFound`; a "label not present" 404
must return `Ok(())`.

---

## 4. New trait method: `list_pr_labels`

### Signature

```rust
/// Return all labels currently applied to an issue or pull request.
///
/// Used by [`ReleaseRegentProcessor::handle_merged_pull_request`] to read
/// any `rr:override-*` labels from the merged feature PR before version
/// calculation.
///
/// # Parameters
/// - `owner`: Repository owner name
/// - `repo`: Repository name
/// - `issue_number`: Issue or pull request number
///
/// # Returns
/// All labels on the issue/PR, or an empty `Vec` when none are applied.
///
/// # Errors
/// - `CoreError::NotFound` — the issue/PR does not exist
/// - `CoreError::GitHub` — the API call failed for any other reason
///
/// # GitHub API
///
/// `GET /repos/{owner}/{repo}/issues/{issue_number}/labels`
///
/// Returns a JSON array of label objects. An empty array is a valid response
/// and must be returned as `Ok(vec![])`.
async fn list_pr_labels(
    &self,
    owner: &str,
    repo: &str,
    issue_number: u64,
) -> CoreResult<Vec<Label>>;
```

---

## 5. Extension to existing `search_pull_requests`

The existing `search_pull_requests` method signature is unchanged. However, its
documented set of supported qualifiers must be extended with `label:NAME`.

### Updated doc comment addition

Append to the method's existing `Supports a subset of GitHub search qualifiers` list:

```
/// - `label:NAME` — filter PRs that carry the named label
```

### Behavioural contract

When a `label:NAME` token is present in the query string, the method must return only
pull requests that currently have a label whose `name` field equals `NAME` exactly
(case-sensitive, matching GitHub's behaviour).

A query may combine `label:` with other qualifiers:

```
is:open label:rr:override-major
```

### Usage in cleanup

After a release PR merges, `handle_merged_pull_request` calls:

```rust
// For each label name in ALL_OVERRIDE_LABELS:
github.search_pull_requests(
    owner,
    repo,
    &format!("is:open label:{label_name}"),
).await?
```

The mock implementation must filter the pre-configured pull request list using this
qualifier. See the mock section below.

---

## 6. `MockGitHubOperations` additions

File: `crates/testing/src/mocks/github_operations.rs`

### Struct fields to add

```rust
/// Per-repository label data keyed `"owner/repo/issue_number"`.
///
/// `list_pr_labels` returns the value for the matching key.
/// `add_labels` and `remove_label` operations update this map at runtime
/// (tests can chain `with_pr_labels` to seed initial state).
pr_labels: HashMap<String, Vec<Label>>,

/// Configurable per-method error overrides.
///
/// Key: method name (e.g. `"add_labels"`).
/// Value: `CoreError` to return instead of the normal result.
///
/// Populated via `with_method_error`.
method_errors: HashMap<String, String>,
```

### Builder methods to add

```rust
/// Pre-populate labels for a specific issue/PR.
///
/// `key` is formatted as `"{owner}/{repo}/{issue_number}"`.
/// Call this before the test to seed the label state the mock will return.
pub fn with_pr_labels(
    mut self,
    owner: &str,
    repo: &str,
    issue_number: u64,
    labels: Vec<Label>,
) -> Self {
    let key = format!("{owner}/{repo}/{issue_number}");
    self.pr_labels.insert(key, labels);
    self
}

/// Configure a specific method to return an error.
///
/// Useful for simulating partial failures (e.g. `remove_label` fails while
/// other operations succeed) without enabling global failure simulation.
///
/// `method_name` must exactly match the method name string used internally
/// (e.g. `"add_labels"`, `"remove_label"`, `"list_pr_labels"`).
pub fn with_method_error(mut self, method_name: &str, error_message: &str) -> Self {
    self.method_errors
        .insert(method_name.to_string(), error_message.to_string());
    self
}
```

### `add_labels` mock implementation

```rust
async fn add_labels(
    &self,
    owner: &str,
    repo: &str,
    issue_number: u64,
    labels: &[&str],
) -> CoreResult<()> {
    let method = "add_labels";
    let params_str = format!(
        "owner={owner}, repo={repo}, issue={issue_number}, labels={labels:?}"
    );

    self.check_quota().await?;
    self.simulate_latency().await;

    if self.should_simulate_failure().await {
        let error = CoreError::network("Simulated GitHub API error");
        self.record_call(method, &params_str, CallResult::Error(error.to_string()))
            .await;
        return Err(error);
    }

    // Check per-method error override.
    if let Some(msg) = self.method_errors.get(method) {
        let error = CoreError::github(msg.clone());
        self.record_call(method, &params_str, CallResult::Error(error.to_string()))
            .await;
        return Err(error);
    }

    // Idempotent: add only labels not yet present.
    let key = format!("{owner}/{repo}/{issue_number}");
    // NOTE: because &self is an immutable reference the mock cannot mutate
    // pr_labels here without interior mutability. Tests that need to verify
    // the post-add state should use a Mutex-wrapped or Arc<RwLock<_>>.
    // For call-recording purposes the test should inspect call_history().

    self.record_call(method, &params_str, CallResult::Success)
        .await;
    Ok(())
}
```

> **Interior-mutability note**: The existing `MockGitHubOperations` uses `&self` for all
> `GitHubOperations` methods (the trait requires `&self`). The `pr_labels` map therefore
> needs `Arc<RwLock<HashMap<…>>>` (matching the pattern already used for `state`) if
> runtime mutation is needed. The coder must decide whether to make the field a shared
> interior-mutable store or document that `add_labels`/`remove_label` are recorded but
> the in-memory label state is not mutated (tests seed state up-front with
> `with_pr_labels`). Either approach is acceptable; the latter is simpler and consistent
> with how `create_branch` is handled today.

### `remove_label` mock implementation

```rust
async fn remove_label(
    &self,
    owner: &str,
    repo: &str,
    issue_number: u64,
    label_name: &str,
) -> CoreResult<()> {
    let method = "remove_label";
    let params_str =
        format!("owner={owner}, repo={repo}, issue={issue_number}, label={label_name}");

    self.check_quota().await?;
    self.simulate_latency().await;

    if self.should_simulate_failure().await {
        let error = CoreError::network("Simulated GitHub API error");
        self.record_call(method, &params_str, CallResult::Error(error.to_string()))
            .await;
        return Err(error);
    }

    if let Some(msg) = self.method_errors.get(method) {
        let error = CoreError::github(msg.clone());
        self.record_call(method, &params_str, CallResult::Error(error.to_string()))
            .await;
        return Err(error);
    }

    // Idempotent: not-found is Ok(()).
    self.record_call(method, &params_str, CallResult::Success)
        .await;
    Ok(())
}
```

### `list_pr_labels` mock implementation

```rust
async fn list_pr_labels(
    &self,
    owner: &str,
    repo: &str,
    issue_number: u64,
) -> CoreResult<Vec<Label>> {
    let method = "list_pr_labels";
    let params_str = format!("owner={owner}, repo={repo}, issue={issue_number}");

    self.check_quota().await?;
    self.simulate_latency().await;

    if self.should_simulate_failure().await {
        let error = CoreError::network("Simulated GitHub API error");
        self.record_call(method, &params_str, CallResult::Error(error.to_string()))
            .await;
        return Err(error);
    }

    if let Some(msg) = self.method_errors.get(method) {
        let error = CoreError::github(msg.clone());
        self.record_call(method, &params_str, CallResult::Error(error.to_string()))
            .await;
        return Err(error);
    }

    let key = format!("{owner}/{repo}/{issue_number}");
    let labels = self.pr_labels.get(&key).cloned().unwrap_or_default();

    self.record_call(method, &params_str, CallResult::Success)
        .await;
    Ok(labels)
}
```

### Extension to `search_pull_requests` mock

Extend the existing qualifier-parsing loop to handle `label:`:

```rust
// Inside the existing search_pull_requests implementation, add to the
// for token in query.split_whitespace() loop:
} else if let Some(l) = token.strip_prefix("label:") {
    label_filter = Some(l.to_string());
}
```

Add a `label_filter: Option<String>` variable alongside the existing `state_filter`,
`head_filter`, and `base_filter`. Apply the filter in the iterator chain:

```rust
.filter(|pr| {
    label_filter.as_ref().map_or(true, |label_name| {
        let key = format!("{owner}/{repo}/{}", pr.number);
        self.pr_labels
            .get(&key)
            .map_or(false, |labels| labels.iter().any(|l| l.name == *label_name))
    })
})
```

---

## 7. Trait method ordering

Per the project's element-ordering convention, methods in `GitHubOperations` are in
alphabetical order. The new methods sort as follows in the full alphabetical list:

| Position | Method |
|---|---|
| … | `create_branch` |
| … | `create_issue_comment` |
| … | `create_pull_request` |
| … | `create_release` |
| … | `create_tag` |
| … | `delete_branch` |
| … | `get_collaborator_permission` |
| … | `get_latest_release` |
| … | `get_pull_request` |
| … | `get_release_by_tag` |
| **NEW** | `add_labels` — insert before `create_branch` |
| **NEW** | `list_pr_labels` — insert before `list_pull_requests` |
| … | `list_pull_requests` |
| … | `list_releases` |
| **NEW** | `remove_label` — insert before `search_pull_requests` |
| … | `search_pull_requests` |
| … | `update_pull_request` |
| … | `update_release` |

---

## 8. Error mapping guidance for `github_client` implementation

| HTTP status | Condition | Map to |
|---|---|---|
| `200 OK` | `add_labels` success | `Ok(())` |
| `204 No Content` | `remove_label` success | `Ok(())` |
| `404 Not Found` (label absent) | `remove_label` | `Ok(())` — idempotent |
| `404 Not Found` (issue/PR absent) | any method | `CoreError::NotFound` |
| `422 Unprocessable Entity` | `add_labels` with unknown label | `CoreError::GitHub` — label must be created first |
| `4xx` other | any method | `CoreError::GitHub` |
| `5xx` | any method | `CoreError::GitHub` (retryable) |
| network error | any method | `CoreError::Network` |

---

## 9. Test scenarios required (for coder reference)

The coder must provide unit tests (in the existing `github_operations_tests.rs` file in
the testing crate) covering at minimum:

| Scenario | Method | Expected |
|---|---|---|
| Add single label — not yet applied | `add_labels` | `Ok(())` |
| Add label already applied | `add_labels` | `Ok(())` (idempotent) |
| Add multiple labels | `add_labels` | `Ok(())` |
| Remove label present | `remove_label` | `Ok(())` |
| Remove label not present (404) | `remove_label` | `Ok(())` (idempotent) |
| List labels — none | `list_pr_labels` | `Ok(vec![])` |
| List labels — multiple | `list_pr_labels` | `Ok([...])` |
| Search with `label:rr:override-major` | `search_pull_requests` | Filtered by label |
| Search `is:open label:rr:override-minor` | `search_pull_requests` | Filtered by state AND label |
| Failure simulation | all new methods | `CoreError::Network` |
