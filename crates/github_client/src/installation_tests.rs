// Tests for `get_installation_id_for_repo` HTTP status mapping.
//
// Uses wiremock to provide a local mock GitHub API server so no real
// credentials are needed.  The key correctness property is that a 401
// response (transient JWT rejection) maps to `CoreError::Network` (retryable)
// — the inverse of `map_sdk_error`'s treatment of a general 401.

use super::*;
use github_bot_sdk::{
    auth::{
        AuthenticationProvider, GitHubAppId, Installation, InstallationId, InstallationPermissions,
        InstallationToken, JsonWebToken, Repository as SdkRepository,
    },
    error::AuthError,
};
use release_regent_core::CoreError;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

// ---------------------------------------------------------------------------
// Minimal mock auth provider that returns a working app JWT
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct MockAppAuth {
    token: String,
}

impl MockAppAuth {
    fn new() -> Self {
        Self {
            token: "fake-jwt-for-tests".to_string(),
        }
    }
}

#[async_trait::async_trait]
impl AuthenticationProvider for MockAppAuth {
    async fn app_token(&self) -> Result<JsonWebToken, AuthError> {
        let app_id = GitHubAppId::new(1);
        let expires_at = chrono::Utc::now() + chrono::Duration::minutes(10);
        Ok(JsonWebToken::new(self.token.clone(), app_id, expires_at))
    }

    async fn installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<InstallationToken, AuthError> {
        let expires_at = chrono::Utc::now() + chrono::Duration::hours(1);
        Ok(InstallationToken::new(
            "fake-token".to_string(),
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
// Helper: build a GitHubClient pointing at the mock server
// ---------------------------------------------------------------------------

fn make_app_client(mock_server: &MockServer) -> GitHubClient {
    GitHubClient::new_for_testing(MockAppAuth::new(), 0, &mock_server.uri())
        .expect("test client construction should not fail")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// A 200 response with `{"id": 42}` should return `Ok(42)`.
#[tokio::test]
async fn test_get_installation_id_success_returns_id() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/installation"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "id": 42 })))
        .mount(&server)
        .await;

    let client = make_app_client(&server);
    let result = client.get_installation_id_for_repo("owner", "repo").await;

    assert_eq!(result.unwrap(), 42);
}

/// A 401 response must map to `CoreError::Network` and be retryable.
///
/// This is the inverse of `map_sdk_error`'s treatment of a general 401
/// (which produces non-retryable `CoreError::Authentication`).  On the
/// installation endpoint a 401 means the JWT was transiently rejected; the
/// caller should retry with a fresh JWT.
#[tokio::test]
async fn test_get_installation_id_401_returns_retryable_network_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/installation"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .mount(&server)
        .await;

    let client = make_app_client(&server);
    let err = client
        .get_installation_id_for_repo("owner", "repo")
        .await
        .unwrap_err();

    assert!(
        matches!(err, CoreError::Network { .. }),
        "401 should produce CoreError::Network, got: {:?}",
        err
    );
    assert!(
        err.is_retryable(),
        "401 installation error must be retryable so a fresh JWT is attempted"
    );
}

/// A 404 response means the app is not installed — non-retryable `CoreError::NotFound`.
#[tokio::test]
async fn test_get_installation_id_404_returns_not_found_non_retryable() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/installation"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
        .mount(&server)
        .await;

    let client = make_app_client(&server);
    let err = client
        .get_installation_id_for_repo("owner", "repo")
        .await
        .unwrap_err();

    assert!(
        matches!(err, CoreError::NotFound { .. }),
        "404 should produce CoreError::NotFound, got: {:?}",
        err
    );
    assert!(
        !err.is_retryable(),
        "404 (app not installed) should not be retryable"
    );
}

/// A 500 response is a server-side transient error — retryable `CoreError::Network`.
#[tokio::test]
async fn test_get_installation_id_500_returns_retryable_network_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/installation"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&server)
        .await;

    let client = make_app_client(&server);
    let err = client
        .get_installation_id_for_repo("owner", "repo")
        .await
        .unwrap_err();

    assert!(
        matches!(err, CoreError::Network { .. }),
        "500 should produce CoreError::Network, got: {:?}",
        err
    );
    assert!(err.is_retryable(), "500 server error should be retryable");
}

/// A 403 response is a permanent client-side error — non-retryable `CoreError::GitHub`.
#[tokio::test]
async fn test_get_installation_id_403_returns_github_error_non_retryable() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/owner/repo/installation"))
        .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden"))
        .mount(&server)
        .await;

    let client = make_app_client(&server);
    let err = client
        .get_installation_id_for_repo("owner", "repo")
        .await
        .unwrap_err();

    assert!(
        matches!(err, CoreError::GitHub { .. }),
        "403 should produce CoreError::GitHub, got: {:?}",
        err
    );
    assert!(
        !err.is_retryable(),
        "403 (forbidden) should not be retryable"
    );
}
