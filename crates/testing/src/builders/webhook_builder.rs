//! Webhook builder for creating test webhook payloads

use super::helpers::{generate_id, generate_iso_timestamp};
use crate::builders::TestDataBuilder;
use serde_json::{json, Value};

/// Builder for creating test webhook payloads
#[derive(Debug, Clone)]
pub struct WebhookBuilder {
    event_type: String,
    action: Option<String>,
    repository_name: String,
    repository_owner: String,
}

impl WebhookBuilder {
    /// Create a new webhook builder with defaults
    #[must_use]
    pub fn new() -> Self {
        Self {
            event_type: "push".to_string(),
            action: None,
            repository_name: "test-repo".to_string(),
            repository_owner: "test-owner".to_string(),
        }
    }

    /// Set event type
    #[must_use]
    pub fn with_event_type(mut self, event_type: &str) -> Self {
        self.event_type = event_type.to_string();
        self
    }

    /// Set action (for some event types)
    #[must_use]
    pub fn with_action(mut self, action: &str) -> Self {
        self.action = Some(action.to_string());
        self
    }

    /// Set repository
    #[must_use]
    pub fn with_repository(mut self, owner: &str, name: &str) -> Self {
        self.repository_owner = owner.to_string();
        self.repository_name = name.to_string();
        self
    }
}

impl Default for WebhookBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TestDataBuilder<Value> for WebhookBuilder {
    fn build(self) -> Value {
        let owner = &self.repository_owner;
        let repo = &self.repository_name;
        let repo_id = generate_id();
        let owner_id = generate_id();

        let owner_obj = json!({
            "login": owner,
            "id": owner_id,
            "node_id": format!("MDQ6VXNlc{owner_id}"),
            "avatar_url": format!("https://avatars.githubusercontent.com/u/{owner_id}?v=4"),
            "gravatar_id": "",
            "url": format!("https://api.github.com/users/{owner}"),
            "html_url": format!("https://github.com/{owner}"),
            "type": "User",
            "site_admin": false
        });

        let repository_obj = json!({
            "id": repo_id,
            "node_id": format!("MDEwOlJlcG9zaXRvcnk{repo_id}"),
            "name": repo,
            "full_name": format!("{owner}/{repo}"),
            "private": false,
            "owner": owner_obj,
            "html_url": format!("https://github.com/{owner}/{repo}"),
            "description": "A test repository for Release Regent",
            "fork": false,
            "url": format!("https://api.github.com/repos/{owner}/{repo}"),
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": generate_iso_timestamp(),
            "pushed_at": generate_iso_timestamp(),
            "git_url": format!("git://github.com/{owner}/{repo}.git"),
            "ssh_url": format!("git@github.com:{owner}/{repo}.git"),
            "clone_url": format!("https://github.com/{owner}/{repo}.git"),
            "default_branch": "main",
            "visibility": "public",
            "size": 1024,
            "stargazers_count": 0,
            "watchers_count": 0,
            "language": "Rust",
            "has_issues": true,
            "has_projects": true,
            "has_wiki": true,
            "has_pages": false,
            "forks_count": 0,
            "archived": false,
            "disabled": false,
            "open_issues_count": 0,
            "forks": 0,
            "open_issues": 0,
            "watchers": 0
        });

        let sender_obj = json!({
            "login": owner,
            "id": owner_id,
            "node_id": format!("MDQ6VXNlc{owner_id}"),
            "avatar_url": format!("https://avatars.githubusercontent.com/u/{owner_id}?v=4"),
            "gravatar_id": "",
            "url": format!("https://api.github.com/users/{owner}"),
            "html_url": format!("https://github.com/{owner}"),
            "type": "User",
            "site_admin": false
        });

        let installation_obj = json!({
            "id": generate_id(),
            "node_id": format!("MDIzOkludGVncmF0aW9uSW5zdGFsbGF0aW9u{}", generate_id())
        });

        let mut payload = json!({
            "repository": repository_obj,
            "sender": sender_obj,
            "installation": installation_obj
        });

        // Merge in action when present (pull_request, issues, etc.)
        if let Some(ref action) = self.action {
            payload["action"] = json!(action);
        }

        // Add event-type-specific top-level fields so the payload is
        // recognisable by the GitHub event type header.
        match self.event_type.as_str() {
            "push" => {
                payload["ref"] = json!("refs/heads/main");
                payload["before"] = json!("0000000000000000000000000000000000000000");
                payload["after"] = json!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
                payload["created"] = json!(false);
                payload["deleted"] = json!(false);
                payload["forced"] = json!(false);
                payload["commits"] = json!([]);
            }
            "pull_request" => {
                let pr_number = generate_id();
                payload["number"] = json!(pr_number);
                payload["pull_request"] = json!({
                    "number": pr_number,
                    "state": "open",
                    "title": "Test pull request",
                    "body": "",
                    "head": {
                        "ref": "feature/test",
                        "sha": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    },
                    "base": {
                        "ref": "main",
                        "sha": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    },
                    "merged": false,
                    "draft": false,
                    "user": sender_obj.clone()
                });
            }
            _ => {}
        }

        payload
    }

    fn reset(self) -> Self {
        Self::new()
    }
}
