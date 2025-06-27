use super::*;
use crate::{GitHubClient, GitHubConfig};

#[test]
fn test_create_pull_request_options() {
    let options = CreatePullRequestOptions {
        title: "chore(release): v1.0.0".to_string(),
        body: "Release PR for v1.0.0".to_string(),
        base: "main".to_string(),
        head: "release/v1.0.0".to_string(),
        draft: false,
    };

    assert_eq!(options.title, "chore(release): v1.0.0");
    assert_eq!(options.body, "Release PR for v1.0.0");
    assert_eq!(options.base, "main");
    assert_eq!(options.head, "release/v1.0.0");
    assert!(!options.draft);
}

#[test]
fn test_pull_request_struct() {
    let pr = PullRequest {
        number: 42,
        title: "Test PR".to_string(),
        body: "Test description".to_string(),
        base: "main".to_string(),
        head: "feature-branch".to_string(),
        draft: true,
    };

    assert_eq!(pr.number, 42);
    assert_eq!(pr.title, "Test PR");
    assert_eq!(pr.body, "Test description");
    assert_eq!(pr.base, "main");
    assert_eq!(pr.head, "feature-branch");
    assert!(pr.draft);
}

// Note: Tests for actual GitHub operations require async runtime and
// GitHub credentials, so they will be covered in integration tests.
