use crate::models::Repository;

#[tokio::test]
async fn test_github_client_new() {
    // Test that GitHubClient can be created with an Octocrab instance
    let octocrab = octocrab::Octocrab::builder()
        .build()
        .expect("Failed to create Octocrab client");
    
    let _client = crate::GitHubClient::new(octocrab);
    // If we get here without panicking, the test passes
    assert!(true);
}

#[test]
fn test_repository_creation() {
    let repo = Repository::new(
        "repo".to_string(),
        "owner/repo".to_string(),
        "MDEwOlJlcG9zaXRvcnkx".to_string(),
        false,
    );

    assert_eq!(repo.name(), "repo");
    assert_eq!(repo.is_private(), false);
}

// Note: GitHub client creation tests require async runtime and are harder to test
// without actual GitHub credentials. These will be covered in integration tests.
