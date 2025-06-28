use super::*;

#[tokio::test]
async fn test_github_config_creation() {
    let config = GitHubConfig {
        app_id: 123456,
        private_key: "test-key".to_string(),
        installation_id: 789012,
        base_url: None,
    };

    assert_eq!(config.app_id, 123456);
    assert_eq!(config.private_key, "test-key");
    assert_eq!(config.installation_id, 789012);
    assert!(config.base_url.is_none());
}

#[tokio::test]
async fn test_github_config_with_custom_url() {
    let config = GitHubConfig {
        app_id: 123456,
        private_key: "test-key".to_string(),
        installation_id: 789012,
        base_url: Some("https://github.enterprise.com/api/v3".to_string()),
    };

    assert!(config.base_url.is_some());
    assert_eq!(
        config.base_url.unwrap(),
        "https://github.enterprise.com/api/v3"
    );
}

#[test]
fn test_repository_creation() {
    let repo = Repository {
        owner: "owner".to_string(),
        name: "repo".to_string(),
        default_branch: "main".to_string(),
    };

    assert_eq!(repo.owner, "owner");
    assert_eq!(repo.name, "repo");
    assert_eq!(repo.default_branch, "main");
}

// Note: GitHub client creation tests require async runtime and are harder to test
// without actual GitHub credentials. These will be covered in integration tests.
