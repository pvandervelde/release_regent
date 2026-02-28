// Tests for type conversions and SDK integration
// Note: Most functionality requires live SDK which is tested via integration tests
// These tests validate basic structure and compilation

#[test]
fn test_github_client_exports() {
    // Verify public API exports exist
    use crate::{AuthConfig, Error, GitHubClient, GitHubResult};

    // Type checking - ensures types are properly exported
    let _: Option<GitHubClient> = None;
    let _: Option<AuthConfig> = None;
    let _: Option<Error> = None;
    let _: GitHubResult<()> = Ok(());
}

#[test]
fn test_sdk_types_reexported() {
    // Verify SDK types are re-exported
    use crate::{GitHubAppId, SdkInstallationId};

    let app_id = GitHubAppId::new(12345);
    assert_eq!(app_id.as_u64(), 12345);

    let installation_id = SdkInstallationId::new(67890);
    assert_eq!(installation_id.as_u64(), 67890);
}
