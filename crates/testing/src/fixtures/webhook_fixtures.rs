//! Webhook fixtures for common webhook scenarios

use serde_json::Value;

/// Generate a GitHub push event webhook payload
///
/// # Returns
/// Realistic push event webhook payload
pub fn github_push_event() -> Value {
    // TODO: implement - placeholder for compilation
    serde_json::json!({
        "ref": "refs/heads/main",
        "before": "0000000000000000000000000000000000000000",
        "after": "1234567890abcdef1234567890abcdef12345678",
        "repository": {
            "id": 123456789,
            "name": "test-repo",
            "full_name": "test-owner/test-repo",
            "owner": {
                "login": "test-owner",
                "id": 12345,
                "type": "User"
            }
        },
        "commits": [
            {
                "id": "1234567890abcdef1234567890abcdef12345678",
                "message": "feat: add new feature",
                "author": {
                    "name": "Test Author",
                    "email": "test@example.com"
                }
            }
        ]
    })
}

/// Generate a GitHub release event webhook payload
///
/// # Returns
/// Realistic release event webhook payload
pub fn github_release_event() -> Value {
    // TODO: implement - placeholder for compilation
    serde_json::json!({
        "action": "published",
        "release": {
            "id": 123456,
            "tag_name": "v1.0.0",
            "name": "Release v1.0.0",
            "body": "Initial release"
        },
        "repository": {
            "name": "test-repo",
            "owner": {
                "login": "test-owner"
            }
        }
    })
}
