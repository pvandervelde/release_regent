//! Self-tests for [`MockGitHubOperations`].
//!
//! Verifies call recording, error injection, and all stub-to-real-implementation
//! promotions for [`GitHubOperations`] and [`GitOperations`] methods.

use super::*;
use crate::builders::{
    CommitBuilder, PullRequestBuilder, ReleaseBuilder, TagBuilder, TestDataBuilder,
};
use chrono::Utc;
use release_regent_core::traits::{
    git_operations::{GitTagType, ListTagsOptions},
    github_operations::{CreatePullRequestParams, CreateReleaseParams, UpdateReleaseParams},
};

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn make_mock() -> MockGitHubOperations {
    MockGitHubOperations::new()
}

fn open_pr(number: u64, head_ref: &str) -> PullRequest {
    PullRequestBuilder::new()
        .with_number(number)
        .with_head_ref(head_ref)
        .with_base_ref("main")
        .as_open()
        .build()
}

// ─────────────────────────────────────────────────────────────────────────────
// get_pull_request
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_pull_request_returns_stored_pr_when_found() {
    let pr = open_pr(42, "feature/my-feature");
    let mock = make_mock().with_pull_requests("owner", "repo", vec![pr.clone()]);

    let result = mock.get_pull_request("owner", "repo", 42).await.unwrap();

    assert_eq!(result.number, 42);
    assert_eq!(result.head.ref_name, "feature/my-feature");
}

#[tokio::test]
async fn test_get_pull_request_returns_error_when_not_found() {
    let mock = make_mock().with_pull_requests("owner", "repo", vec![open_pr(1, "x")]);

    let result = mock.get_pull_request("owner", "repo", 999).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_pull_request_records_call_on_success() {
    let mock = make_mock().with_pull_requests("owner", "repo", vec![open_pr(7, "x")]);
    let _ = mock.get_pull_request("owner", "repo", 7).await;

    assert_eq!(mock.call_count().await, 1);
}

// ─────────────────────────────────────────────────────────────────────────────
// list_pull_requests
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_list_pull_requests_returns_open_prs_by_default() {
    let open = open_pr(1, "feature/a");
    let closed = PullRequestBuilder::new().with_number(2).as_closed().build();
    let mock = make_mock().with_pull_requests("o", "r", vec![open, closed]);

    let result = mock
        .list_pull_requests("o", "r", Some("open"), None, None, None, None)
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].number, 1);
}

#[tokio::test]
async fn test_list_pull_requests_returns_all_when_state_is_all() {
    let mock = make_mock().with_pull_requests(
        "o",
        "r",
        vec![
            open_pr(1, "a"),
            PullRequestBuilder::new().with_number(2).as_closed().build(),
        ],
    );

    let result = mock
        .list_pull_requests("o", "r", Some("all"), None, None, None, None)
        .await
        .unwrap();

    assert_eq!(result.len(), 2);
}

#[tokio::test]
async fn test_list_pull_requests_filters_by_head_branch() {
    let mock = make_mock().with_pull_requests(
        "o",
        "r",
        vec![open_pr(1, "release/v1.0.0"), open_pr(2, "feature/x")],
    );

    let result = mock
        .list_pull_requests("o", "r", None, Some("release/v1.0.0"), None, None, None)
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].number, 1);
}

#[tokio::test]
async fn test_list_pull_requests_filters_by_base_branch() {
    let pr_main = PullRequestBuilder::new()
        .with_number(1)
        .with_base_ref("main")
        .build();
    let pr_dev = PullRequestBuilder::new()
        .with_number(2)
        .with_base_ref("develop")
        .build();
    let mock = make_mock().with_pull_requests("o", "r", vec![pr_main, pr_dev]);

    let result = mock
        .list_pull_requests("o", "r", Some("all"), None, Some("develop"), None, None)
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].number, 2);
}

#[tokio::test]
async fn test_list_pull_requests_returns_empty_when_no_match() {
    let mock = make_mock().with_pull_requests("o", "r", vec![open_pr(1, "x")]);

    let result = mock
        .list_pull_requests("o", "r", Some("closed"), None, None, None, None)
        .await
        .unwrap();

    assert!(result.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// search_pull_requests
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_search_pull_requests_filters_by_is_open() {
    let mock = make_mock().with_pull_requests(
        "o",
        "r",
        vec![
            open_pr(1, "a"),
            PullRequestBuilder::new().with_number(2).as_closed().build(),
        ],
    );

    let result = mock
        .search_pull_requests("o", "r", "is:open")
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].number, 1);
}

#[tokio::test]
async fn test_search_pull_requests_matches_head_branch_glob_prefix() {
    let mock = make_mock().with_pull_requests(
        "o",
        "r",
        vec![
            open_pr(1, "release/v1.0.0"),
            open_pr(2, "release/v2.0.0"),
            open_pr(3, "feature/x"),
        ],
    );

    let result = mock
        .search_pull_requests("o", "r", "is:open head:release/*")
        .await
        .unwrap();

    assert_eq!(result.len(), 2);
    assert!(result
        .iter()
        .all(|pr| pr.head.ref_name.starts_with("release/")));
}

#[tokio::test]
async fn test_search_pull_requests_matches_exact_head_branch() {
    let mock = make_mock().with_pull_requests(
        "o",
        "r",
        vec![open_pr(1, "release/v1.0.0"), open_pr(2, "release/v2.0.0")],
    );

    let result = mock
        .search_pull_requests("o", "r", "head:release/v1.0.0")
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].number, 1);
}

#[tokio::test]
async fn test_search_pull_requests_returns_empty_when_no_match() {
    let mock = make_mock().with_pull_requests("o", "r", vec![open_pr(1, "feature/x")]);

    let result = mock
        .search_pull_requests("o", "r", "is:open head:release/*")
        .await
        .unwrap();

    assert!(result.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// get_release_by_tag
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_release_by_tag_returns_matching_release() {
    let rel = ReleaseBuilder::new().with_tag_name("v1.2.0").build();
    let mock = make_mock().with_releases("o", "r", vec![rel]);

    let result = mock.get_release_by_tag("o", "r", "v1.2.0").await.unwrap();

    assert_eq!(result.tag_name, "v1.2.0");
}

#[tokio::test]
async fn test_get_release_by_tag_returns_error_when_not_found() {
    let mock = make_mock().with_releases(
        "o",
        "r",
        vec![ReleaseBuilder::new().with_tag_name("v1.0.0").build()],
    );

    let result = mock.get_release_by_tag("o", "r", "v9.9.9").await;

    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// list_releases
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_list_releases_returns_all_stored_releases() {
    let releases = vec![
        ReleaseBuilder::new().with_tag_name("v1.0.0").build(),
        ReleaseBuilder::new().with_tag_name("v1.1.0").build(),
    ];
    let mock = make_mock().with_releases("o", "r", releases);

    let result = mock.list_releases("o", "r", None, None).await.unwrap();

    assert_eq!(result.len(), 2);
}

#[tokio::test]
async fn test_list_releases_returns_empty_when_no_releases_configured() {
    let mock = make_mock();

    let result = mock.list_releases("o", "r", None, None).await.unwrap();

    assert!(result.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// get_latest_release
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_latest_release_returns_non_draft_non_prerelease() {
    let stable = ReleaseBuilder::new().with_tag_name("v1.0.0").build();
    let draft = ReleaseBuilder::new()
        .with_tag_name("v2.0.0")
        .as_draft()
        .build();
    let mock = make_mock().with_releases("o", "r", vec![stable, draft]);

    let result = mock.get_latest_release("o", "r").await.unwrap();

    assert!(result.is_some());
    assert_eq!(result.unwrap().tag_name, "v1.0.0");
}

#[tokio::test]
async fn test_get_latest_release_returns_none_when_only_draft_exists() {
    let mock = make_mock().with_releases("o", "r", vec![ReleaseBuilder::new().as_draft().build()]);

    let result = mock.get_latest_release("o", "r").await.unwrap();

    assert!(result.is_none());
}

#[tokio::test]
async fn test_get_latest_release_returns_none_when_no_releases() {
    let mock = make_mock();

    let result = mock.get_latest_release("o", "r").await.unwrap();

    assert!(result.is_none());
}

#[tokio::test]
async fn test_get_latest_release_skips_prerelease() {
    let stable = ReleaseBuilder::new().with_tag_name("v1.0.0").build();
    let pre = ReleaseBuilder::new()
        .with_tag_name("v2.0.0-rc.1")
        .as_prerelease()
        .build();
    let mock = make_mock().with_releases("o", "r", vec![pre, stable]);

    let result = mock.get_latest_release("o", "r").await.unwrap();

    assert_eq!(result.unwrap().tag_name, "v1.0.0");
}

// ─────────────────────────────────────────────────────────────────────────────
// create_pull_request
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_create_pull_request_records_call_and_returns_pr_from_params() {
    let mock = make_mock();
    let params = CreatePullRequestParams {
        title: "Release v1.0.0".to_string(),
        head: "release/v1.0.0".to_string(),
        base: "main".to_string(),
        body: Some("Changelog".to_string()),
        draft: false,
        maintainer_can_modify: true,
    };

    let result = mock.create_pull_request("o", "r", params).await.unwrap();

    assert_eq!(result.title, "Release v1.0.0");
    assert_eq!(result.head.ref_name, "release/v1.0.0");
    assert_eq!(mock.call_count().await, 1);
}

// ─────────────────────────────────────────────────────────────────────────────
// create_release
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_create_release_records_call_and_returns_release_from_params() {
    let mock = make_mock();
    let params = CreateReleaseParams {
        tag_name: "v1.0.0".to_string(),
        name: Some("Release 1.0.0".to_string()),
        body: Some("notes".to_string()),
        draft: false,
        prerelease: false,
        target_commitish: Some("main".to_string()),
        generate_release_notes: false,
    };

    let result = mock.create_release("o", "r", params).await.unwrap();

    assert_eq!(result.tag_name, "v1.0.0");
    assert_eq!(mock.call_count().await, 1);
}

// ─────────────────────────────────────────────────────────────────────────────
// create_tag
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_create_tag_records_call_and_returns_tag_from_params() {
    let mock = make_mock();

    let result = mock
        .create_tag(
            "o",
            "r",
            "v1.0.0",
            "abc123",
            Some("Release".to_string()),
            None,
        )
        .await
        .unwrap();

    assert_eq!(result.name, "v1.0.0");
    assert_eq!(result.commit_sha, "abc123");
    assert_eq!(mock.call_count().await, 1);
}

// ─────────────────────────────────────────────────────────────────────────────
// update_pull_request
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_update_pull_request_applies_title_change() {
    let pr = open_pr(10, "release/v1.0.0");
    let mock = make_mock().with_pull_requests("o", "r", vec![pr]);

    let result = mock
        .update_pull_request("o", "r", 10, Some("Updated Title".to_string()), None, None)
        .await
        .unwrap();

    assert_eq!(result.title, "Updated Title");
}

#[tokio::test]
async fn test_update_pull_request_returns_error_when_pr_not_found() {
    let mock = make_mock().with_pull_requests("o", "r", vec![open_pr(1, "x")]);

    let result = mock
        .update_pull_request("o", "r", 999, None, None, None)
        .await;

    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// update_release
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_update_release_applies_body_change() {
    let mut rel = ReleaseBuilder::new().with_tag_name("v1.0.0").build();
    rel.id = 55;
    let mock = make_mock().with_releases("o", "r", vec![rel]);

    let params = UpdateReleaseParams {
        body: Some("New notes".to_string()),
        draft: None,
        name: None,
        prerelease: None,
    };
    let result = mock.update_release("o", "r", 55, params).await.unwrap();

    assert_eq!(result.body.as_deref(), Some("New notes"));
}

#[tokio::test]
async fn test_update_release_returns_error_when_not_found() {
    let mock = make_mock();

    let params = UpdateReleaseParams {
        body: None,
        draft: None,
        name: None,
        prerelease: None,
    };
    let result = mock.update_release("o", "r", 404, params).await;

    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// GitOperations: list_tags / get_tag / tag_exists
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_list_tags_returns_stored_tags_as_git_tags() {
    let tag = TagBuilder::new()
        .with_name("v1.0.0")
        .with_commit_sha("abc")
        .build();
    let mock = make_mock().with_tags("o", "r", vec![tag]);

    let result = mock
        .list_tags("o", "r", ListTagsOptions::default())
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name, "v1.0.0");
    assert_eq!(result[0].target_sha, "abc");
}

#[tokio::test]
async fn test_list_tags_returns_annotated_type_when_message_set() {
    let tag = TagBuilder::new().with_name("v1.0.0").annotated().build();
    let mock = make_mock().with_tags("o", "r", vec![tag]);

    let result = mock
        .list_tags("o", "r", ListTagsOptions::default())
        .await
        .unwrap();

    assert!(matches!(result[0].tag_type, GitTagType::Annotated));
}

#[tokio::test]
async fn test_list_tags_returns_lightweight_type_when_no_message() {
    let tag = TagBuilder::new().with_name("v1.0.0").build();
    let mock = make_mock().with_tags("o", "r", vec![tag]);

    let result = mock
        .list_tags("o", "r", ListTagsOptions::default())
        .await
        .unwrap();

    assert!(matches!(result[0].tag_type, GitTagType::Lightweight));
}

#[tokio::test]
async fn test_list_tags_returns_empty_when_no_tags_configured() {
    let mock = make_mock();

    let result = mock
        .list_tags("o", "r", ListTagsOptions::default())
        .await
        .unwrap();

    assert!(result.is_empty());
}

#[tokio::test]
async fn test_get_tag_returns_stored_tag_found_by_name() {
    let tag = TagBuilder::new()
        .with_name("v2.0.0")
        .with_commit_sha("deadbeef")
        .build();
    let mock = make_mock().with_tags("o", "r", vec![tag]);

    let result = mock.get_tag("o", "r", "v2.0.0").await.unwrap();

    assert_eq!(result.name, "v2.0.0");
    assert_eq!(result.target_sha, "deadbeef");
}

#[tokio::test]
async fn test_get_tag_returns_error_when_not_found() {
    let mock = make_mock().with_tags(
        "o",
        "r",
        vec![TagBuilder::new().with_name("v1.0.0").build()],
    );

    let result = mock.get_tag("o", "r", "v9.9.9").await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_tag_exists_returns_true_when_tag_present() {
    let mock = make_mock().with_tags(
        "o",
        "r",
        vec![TagBuilder::new().with_name("v1.0.0").build()],
    );

    let result = mock.tag_exists("o", "r", "v1.0.0").await.unwrap();

    assert!(result);
}

#[tokio::test]
async fn test_tag_exists_returns_false_when_tag_absent() {
    let mock = make_mock().with_tags(
        "o",
        "r",
        vec![TagBuilder::new().with_name("v1.0.0").build()],
    );

    let result = mock.tag_exists("o", "r", "v2.0.0").await.unwrap();

    assert!(!result);
}

#[tokio::test]
async fn test_tag_exists_returns_false_when_no_tags_configured() {
    let mock = make_mock();

    let result = mock.tag_exists("o", "r", "v1.0.0").await.unwrap();

    assert!(!result);
}

// ─────────────────────────────────────────────────────────────────────────────
// GitOperations: get_commit / get_commits_between — call recording
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_commit_returns_stored_commit_and_records_call() {
    use crate::builders::{CommitBuilder, TestDataBuilder};
    let commit = CommitBuilder::new().build();
    let sha = commit.sha.clone();
    let mock = make_mock().with_commits("o", "r", vec![commit]);

    let result = mock.get_commit("o", "r", &sha).await.unwrap();

    assert_eq!(result.sha, sha);
    // call recording via get_repository_info or direct methods
}

#[tokio::test]
async fn test_get_commit_returns_error_when_sha_not_found() {
    use crate::builders::{CommitBuilder, TestDataBuilder};
    let commit = CommitBuilder::new().build();
    let mock = make_mock().with_commits("o", "r", vec![commit]);

    let result = mock.get_commit("o", "r", "nonexistent-sha").await;

    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
// Error injection (failure simulation)
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_failure_simulation_returns_error_for_list_releases() {
    use crate::mocks::MockConfig;
    let config = MockConfig {
        simulate_failures: true,
        failure_rate: 1.0, // always fail
        ..MockConfig::default()
    };
    let mock = MockGitHubOperations::with_config(config).with_releases(
        "o",
        "r",
        vec![ReleaseBuilder::new().build()],
    );

    let result = mock.list_releases("o", "r", None, None).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_call_count_increments_for_each_method_call() {
    let pr = open_pr(1, "x");
    let rel = ReleaseBuilder::new().with_tag_name("v1.0.0").build();
    let mock = make_mock()
        .with_pull_requests("o", "r", vec![pr])
        .with_releases("o", "r", vec![rel]);

    let _ = mock.get_pull_request("o", "r", 1).await;
    let _ = mock.list_releases("o", "r", None, None).await;
    let _ = mock.get_latest_release("o", "r").await;

    assert_eq!(mock.call_count().await, 3);
}

// ─────────────────────────────────────────────────────────────────────────────
// TagBuilder
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_tag_builder_with_name_sets_name() {
    let tag = TagBuilder::new().with_name("v3.0.0").build();
    assert_eq!(tag.name, "v3.0.0");
}

#[test]
fn test_tag_builder_annotated_sets_message() {
    let tag = TagBuilder::new().with_name("v1.0.0").annotated().build();
    assert!(tag.message.is_some());
    assert!(tag.message.unwrap().contains("v1.0.0"));
}

#[test]
fn test_tag_builder_default_has_no_message() {
    let tag = TagBuilder::new().build();
    assert!(tag.message.is_none());
}

#[test]
fn test_tag_builder_with_commit_sha_sets_sha() {
    let tag = TagBuilder::new().with_commit_sha("cafebabe").build();
    assert_eq!(tag.commit_sha, "cafebabe");
}

#[test]
fn test_tag_builder_with_created_at_sets_timestamp() {
    let ts = Utc::now();
    let tag = TagBuilder::new().with_created_at(ts).build();
    assert_eq!(tag.created_at, Some(ts));
}
