// Tests for list_pull_requests implementation.
// Uses wiremock to provide a local mock GitHub API server so no real credentials are needed.

use super::*;
use github_bot_sdk::{
    auth::{
        AuthenticationProvider, Installation, InstallationId, InstallationPermissions,
        InstallationToken, JsonWebToken, Repository as SdkRepository,
    },
    error::AuthError,
};
use wiremock::{
    matchers::{header, method, path, query_param},
    Mock, MockServer, ResponseTemplate,
};

// ---------------------------------------------------------------------------
// Minimal mock auth provider (no real credentials needed)
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct MockAuthProvider {
    token: String,
}

impl MockAuthProvider {
    fn new(token: &str) -> Self {
        Self {
            token: token.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl AuthenticationProvider for MockAuthProvider {
    async fn app_token(&self) -> Result<JsonWebToken, AuthError> {
        Err(AuthError::TokenGenerationFailed {
            message: "not implemented for mock".into(),
        })
    }

    async fn installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<InstallationToken, AuthError> {
        let expires_at = chrono::Utc::now() + chrono::Duration::hours(1);
        Ok(InstallationToken::new(
            self.token.clone(),
            installation_id,
            expires_at,
            InstallationPermissions::default(),
            Vec::new(),
        ))
    }

    async fn refresh_installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<InstallationToken, AuthError> {
        self.installation_token(installation_id).await
    }

    async fn list_installations(&self) -> Result<Vec<Installation>, AuthError> {
        Ok(Vec::new())
    }

    async fn get_installation_repositories(
        &self,
        _installation_id: InstallationId,
    ) -> Result<Vec<SdkRepository>, AuthError> {
        Ok(Vec::new())
    }
}

// ---------------------------------------------------------------------------
// Helper: build mock JSON for a single pull request
// ---------------------------------------------------------------------------

fn pr_json(number: u64, head_ref: &str, base_ref: &str, state: &str) -> serde_json::Value {
    serde_json::json!({
        "id": number,
        "node_id": format!("PR_{}", number),
        "number": number,
        "title": format!("PR #{}", number),
        "body": null,
        "state": state,
        "user": { "login": "testuser", "id": 1, "node_id": "U_1", "type": "User" },
        "head": {
            "ref": head_ref,
            "sha": format!("head{}", number),
            "repo": { "id": 100, "name": "repo", "full_name": "owner/repo" }
        },
        "base": {
            "ref": base_ref,
            "sha": format!("base{}", number),
            "repo": { "id": 100, "name": "repo", "full_name": "owner/repo" }
        },
        "draft": false,
        "merged": false,
        "mergeable": null,
        "merge_commit_sha": null,
        "assignees": [],
        "requested_reviewers": [],
        "labels": [],
        "milestone": null,
        "created_at": "2024-01-01T00:00:00Z",
        "updated_at": "2024-01-01T00:00:00Z",
        "closed_at": null,
        "merged_at": null,
        "html_url": format!("https://github.com/owner/repo/pull/{}", number)
    })
}

// ---------------------------------------------------------------------------
// Helper: build a GitHubClient pointing at the mock server
// ---------------------------------------------------------------------------

fn make_client(mock_server: &MockServer, token: &str) -> GitHubClient {
    let auth = MockAuthProvider::new(token);
    GitHubClient::new_for_testing(auth, 12345, &mock_server.uri())
        .expect("test client construction should not fail")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// `list_pull_requests` with no filters returns all open PRs from GitHub API.
#[tokio::test]
async fn test_list_pull_requests_no_filters_returns_open_prs() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/pulls"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            pr_json(1, "feature/one", "main", "open"),
            pr_json(2, "feature/two", "main", "open"),
        ])))
        .mount(&mock_server)
        .await;

    let client = make_client(&mock_server, "test-token");
    let prs = client
        .list_pull_requests("owner", "repo", None, None, None, None, None)
        .await
        .expect("list_pull_requests should succeed");

    assert_eq!(prs.len(), 2);
    assert_eq!(prs[0].number, 1);
    assert_eq!(prs[1].number, 2);
}

/// `list_pull_requests` with `state = Some("closed")` sends `state=closed` query param.
#[tokio::test]
async fn test_list_pull_requests_closed_state_sends_correct_query_param() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/pulls"))
        .and(query_param("state", "closed"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([pr_json(
                5, "fix/bug", "main", "closed"
            ),])),
        )
        .mount(&mock_server)
        .await;

    let client = make_client(&mock_server, "test-token");
    let prs = client
        .list_pull_requests("owner", "repo", Some("closed"), None, None, None, None)
        .await
        .expect("list_pull_requests should succeed with state=closed");

    assert_eq!(prs.len(), 1);
    assert_eq!(prs[0].number, 5);
}

/// `list_pull_requests` with `head = Some("release/v*")` applies client-side prefix filter.
#[tokio::test]
async fn test_list_pull_requests_head_filter_applied_client_side() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/pulls"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            pr_json(1, "release/v1.0.0", "main", "open"),
            pr_json(2, "feature/unrelated", "main", "open"),
            pr_json(3, "release/v2.0.0", "main", "open"),
        ])))
        .mount(&mock_server)
        .await;

    let client = make_client(&mock_server, "test-token");
    let prs = client
        .list_pull_requests("owner", "repo", None, Some("release/v"), None, None, None)
        .await
        .expect("list_pull_requests with head filter should succeed");

    // Only PRs whose head ref starts with "release/v" should be returned
    assert_eq!(prs.len(), 2);
    assert!(prs
        .iter()
        .all(|pr| pr.head.ref_name.starts_with("release/v")));
}

/// `list_pull_requests` with `base = Some("main")` applies client-side exact-match filter.
#[tokio::test]
async fn test_list_pull_requests_base_filter_applied_client_side() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/pulls"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            pr_json(1, "feature/a", "main", "open"),
            pr_json(2, "feature/b", "develop", "open"),
            pr_json(3, "feature/c", "main", "open"),
        ])))
        .mount(&mock_server)
        .await;

    let client = make_client(&mock_server, "test-token");
    let prs = client
        .list_pull_requests("owner", "repo", None, None, Some("main"), None, None)
        .await
        .expect("list_pull_requests with base filter should succeed");

    assert_eq!(prs.len(), 2);
    assert!(prs.iter().all(|pr| pr.base.ref_name == "main"));
}

/// `list_pull_requests` follows pagination and returns combined results from both pages.
#[tokio::test]
async fn test_list_pull_requests_pagination_combines_all_pages() {
    let mock_server = MockServer::start().await;

    // First page — includes Link header pointing to page 2
    let link_header = format!(
        r#"<{}repos/owner/repo/pulls?page=2>; rel="next""#,
        mock_server.uri()
    );
    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/pulls"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Link", link_header)
                .set_body_json(serde_json::json!([pr_json(
                    1,
                    "feature/first",
                    "main",
                    "open"
                ),])),
        )
        .up_to_n_times(1)
        .mount(&mock_server)
        .await;

    // Second page — no Link header → last page
    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/pulls"))
        .and(query_param("page", "2"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([pr_json(
                2,
                "feature/second",
                "main",
                "open"
            ),])),
        )
        .mount(&mock_server)
        .await;

    let client = make_client(&mock_server, "test-token");
    let prs = client
        .list_pull_requests("owner", "repo", None, None, None, None, None)
        .await
        .expect("list_pull_requests should follow pagination");

    assert_eq!(prs.len(), 2, "should combine results from both pages");
}

/// `list_pull_requests` returns `CoreError::NotFound` when the repository does not exist.
#[tokio::test]
async fn test_list_pull_requests_not_found_returns_core_error_not_found() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/owner/missing-repo/pulls"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_server)
        .await;

    let client = make_client(&mock_server, "test-token");
    let result = client
        .list_pull_requests("owner", "missing-repo", None, None, None, None, None)
        .await;

    assert!(
        matches!(result, Err(CoreError::NotFound { .. })),
        "404 should produce CoreError::NotFound, got: {:?}",
        result
    );
}
