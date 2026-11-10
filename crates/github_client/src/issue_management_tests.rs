// Tests for `create_issue` / `list_issues` implementations.
// Uses wiremock to provide a local mock GitHub API server so no real credentials are needed.

use super::*;
use github_bot_sdk::{
    auth::{
        AuthenticationProvider, Installation, InstallationId, InstallationPermissions,
        InstallationToken, JsonWebToken, Repository as SdkRepository,
    },
    error::AuthError,
};
use release_regent_core::traits::github_operations::{
    CreateIssueParams, IssueState, ListIssuesOptions,
};
use wiremock::{
    matchers::{method, path, query_param},
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

fn make_client(mock_server: &MockServer, token: &str) -> GitHubClient {
    let auth = MockAuthProvider::new(token);
    GitHubClient::new_for_testing(auth, 12345, &mock_server.uri())
        .expect("test client construction should not fail")
}

fn issue_json(number: u64, title: &str, state: &str) -> serde_json::Value {
    serde_json::json!({
        "number": number,
        "title": title,
        "body": "issue body",
        "state": state,
        "labels": [
            { "id": 1, "name": "bug", "color": "ff0000", "description": null }
        ],
        "html_url": format!("https://github.com/owner/repo/issues/{}", number)
    })
}

// ---------------------------------------------------------------------------
// create_issue
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_create_issue_success_returns_mapped_issue() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/repos/owner/repo/issues"))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(issue_json(42, "Something broke", "open")),
        )
        .mount(&mock_server)
        .await;

    let client = make_client(&mock_server, "test-token");
    let params = CreateIssueParams {
        title: "Something broke".to_string(),
        body: Some("issue body".to_string()),
        labels: vec!["bug".to_string()],
        assignees: vec![],
    };

    let issue = client
        .create_issue("owner", "repo", params)
        .await
        .expect("create_issue should succeed");

    assert_eq!(issue.number, 42);
    assert_eq!(issue.title, "Something broke");
    assert_eq!(issue.state, "open");
    assert_eq!(issue.labels.len(), 1);
    assert_eq!(issue.labels[0].name, "bug");
    assert_eq!(issue.html_url, "https://github.com/owner/repo/issues/42");
}

#[tokio::test]
async fn test_create_issue_empty_title_returns_invalid_input_without_http_call() {
    let mock_server = MockServer::start().await;

    // No mock registered: any HTTP call would fail to match and the test
    // would fail with a 404 from wiremock's default response, or hang.
    let client = make_client(&mock_server, "test-token");
    let params = CreateIssueParams {
        title: "   ".to_string(),
        body: None,
        labels: vec![],
        assignees: vec![],
    };

    let result = client.create_issue("owner", "repo", params).await;

    assert!(matches!(
        result,
        Err(release_regent_core::CoreError::InvalidInput { .. })
    ));
}

// ---------------------------------------------------------------------------
// list_issues
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_issues_filters_out_pull_requests() {
    let mock_server = MockServer::start().await;

    let mut pr_item = issue_json(2, "A pull request", "open");
    pr_item["pull_request"] = serde_json::json!({ "url": "https://api.github.com/repos/owner/repo/pulls/2" });

    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/issues"))
        .and(query_param("state", "open"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            issue_json(1, "A real issue", "open"),
            pr_item,
        ])))
        .mount(&mock_server)
        .await;

    let client = make_client(&mock_server, "test-token");
    let issues = client
        .list_issues(
            "owner",
            "repo",
            ListIssuesOptions {
                state: IssueState::Open,
                labels: vec![],
            },
        )
        .await
        .expect("list_issues should succeed");

    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].number, 1);
}

#[tokio::test]
async fn test_list_issues_with_labels_sends_labels_query_param() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/issues"))
        .and(query_param("state", "all"))
        .and(query_param("labels", "bug,urgent"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!([issue_json(7, "Labelled issue", "closed")])),
        )
        .mount(&mock_server)
        .await;

    let client = make_client(&mock_server, "test-token");
    let issues = client
        .list_issues(
            "owner",
            "repo",
            ListIssuesOptions {
                state: IssueState::All,
                labels: vec!["bug".to_string(), "urgent".to_string()],
            },
        )
        .await
        .expect("list_issues should succeed with label filter");

    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].number, 7);
}
