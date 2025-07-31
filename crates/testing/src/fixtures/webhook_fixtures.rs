//! Webhook fixtures for common GitHub webhook scenarios
//!
//! This module provides realistic webhook payloads that match actual GitHub webhook structures.
//! All fixtures are based on real GitHub webhook examples and can be used for testing
//! webhook processing logic.

use crate::builders::helpers::*;
use chrono::{DateTime, Utc};
use serde_json::{json, Value};

/// Builder for GitHub push event webhook payloads
#[derive(Debug, Clone)]
pub struct PushEventBuilder {
    ref_name: String,
    before_sha: String,
    after_sha: String,
    repository_name: String,
    repository_owner: String,
    commits: Vec<Value>,
    forced: bool,
    created: bool,
    deleted: bool,
}

impl PushEventBuilder {
    /// Create a new push event builder with defaults
    pub fn new() -> Self {
        Self {
            ref_name: "refs/heads/main".to_string(),
            before_sha: "0000000000000000000000000000000000000000".to_string(),
            after_sha: generate_commit_sha(),
            repository_name: generate_repo_name(),
            repository_owner: generate_github_login(),
            commits: vec![Self::default_commit()],
            forced: false,
            created: false,
            deleted: false,
        }
    }

    /// Set the branch reference
    pub fn with_ref(mut self, ref_name: &str) -> Self {
        self.ref_name = if ref_name.starts_with("refs/") {
            ref_name.to_string()
        } else {
            format!("refs/heads/{}", ref_name)
        };
        self
    }

    /// Set the branch name (convenience method)
    pub fn with_branch(mut self, branch: &str) -> Self {
        self.ref_name = format!("refs/heads/{}", branch);
        self
    }

    /// Set before SHA (the commit before the push)
    pub fn with_before_sha(mut self, sha: &str) -> Self {
        self.before_sha = sha.to_string();
        self
    }

    /// Set after SHA (the commit after the push)
    pub fn with_after_sha(mut self, sha: &str) -> Self {
        self.after_sha = sha.to_string();
        self
    }

    /// Set repository details
    pub fn with_repository(mut self, owner: &str, name: &str) -> Self {
        self.repository_owner = owner.to_string();
        self.repository_name = name.to_string();
        self
    }

    /// Add commits to the push
    pub fn with_commits(mut self, commits: Vec<Value>) -> Self {
        self.commits = commits;
        self
    }

    /// Add a single commit with conventional commit message
    pub fn with_conventional_commit(mut self, commit_type: &str, description: &str) -> Self {
        let commit = json!({
            "id": generate_commit_sha(),
            "tree_id": generate_commit_sha(),
            "distinct": true,
            "message": format!("{}: {}", commit_type, description),
            "timestamp": generate_iso_timestamp(),
            "url": format!("https://github.com/{}/{}/commit/{}",
                          self.repository_owner, self.repository_name, generate_commit_sha()),
            "author": {
                "name": generate_full_name(),
                "email": generate_email(),
                "username": generate_github_login()
            },
            "committer": {
                "name": generate_full_name(),
                "email": generate_email(),
                "username": generate_github_login()
            },
            "added": [],
            "removed": [],
            "modified": ["README.md"]
        });
        self.commits = vec![commit];
        self
    }

    /// Create multiple conventional commits
    pub fn with_conventional_commits(mut self) -> Self {
        let commits = vec![
            json!({
                "id": generate_commit_sha(),
                "tree_id": generate_commit_sha(),
                "distinct": true,
                "message": "feat: add user authentication system",
                "timestamp": generate_iso_timestamp(),
                "url": format!("https://github.com/{}/{}/commit/{}",
                              self.repository_owner, self.repository_name, generate_commit_sha()),
                "author": {
                    "name": generate_full_name(),
                    "email": generate_email(),
                    "username": generate_github_login()
                },
                "committer": {
                    "name": generate_full_name(),
                    "email": generate_email(),
                    "username": generate_github_login()
                },
                "added": ["src/auth.rs"],
                "removed": [],
                "modified": ["Cargo.toml", "src/lib.rs"]
            }),
            json!({
                "id": generate_commit_sha(),
                "tree_id": generate_commit_sha(),
                "distinct": true,
                "message": "fix: resolve authentication token validation",
                "timestamp": generate_iso_timestamp(),
                "url": format!("https://github.com/{}/{}/commit/{}",
                              self.repository_owner, self.repository_name, generate_commit_sha()),
                "author": {
                    "name": generate_full_name(),
                    "email": generate_email(),
                    "username": generate_github_login()
                },
                "committer": {
                    "name": generate_full_name(),
                    "email": generate_email(),
                    "username": generate_github_login()
                },
                "added": [],
                "removed": [],
                "modified": ["src/auth.rs"]
            }),
            json!({
                "id": generate_commit_sha(),
                "tree_id": generate_commit_sha(),
                "distinct": true,
                "message": "docs: update authentication documentation",
                "timestamp": generate_iso_timestamp(),
                "url": format!("https://github.com/{}/{}/commit/{}",
                              self.repository_owner, self.repository_name, generate_commit_sha()),
                "author": {
                    "name": generate_full_name(),
                    "email": generate_email(),
                    "username": generate_github_login()
                },
                "committer": {
                    "name": generate_full_name(),
                    "email": generate_email(),
                    "username": generate_github_login()
                },
                "added": [],
                "removed": [],
                "modified": ["README.md", "docs/auth.md"]
            }),
        ];
        self.commits = commits;
        self
    }

    /// Set as forced push
    pub fn as_forced(mut self) -> Self {
        self.forced = true;
        self
    }

    /// Set as branch creation
    pub fn as_branch_creation(mut self) -> Self {
        self.created = true;
        self.before_sha = "0000000000000000000000000000000000000000".to_string();
        self
    }

    /// Set as branch deletion
    pub fn as_branch_deletion(mut self) -> Self {
        self.deleted = true;
        self.after_sha = "0000000000000000000000000000000000000000".to_string();
        self.commits = vec![];
        self
    }

    /// Build the webhook payload
    pub fn build(self) -> Value {
        json!({
            "ref": self.ref_name,
            "before": self.before_sha,
            "after": self.after_sha,
            "created": self.created,
            "deleted": self.deleted,
            "forced": self.forced,
            "base_ref": null,
            "compare": format!("https://github.com/{}/{}/compare/{}...{}",
                             self.repository_owner, self.repository_name,
                             &self.before_sha[..12], &self.after_sha[..12]),
            "commits": self.commits,
            "head_commit": self.commits.last().cloned().unwrap_or(Self::default_commit()),
            "repository": {
                "id": generate_id(),
                "node_id": format!("MDEwOlJlcG9zaXRvcnk{}", generate_id()),
                "name": self.repository_name,
                "full_name": format!("{}/{}", self.repository_owner, self.repository_name),
                "private": false,
                "owner": {
                    "name": self.repository_owner,
                    "email": format!("{}@users.noreply.github.com", self.repository_owner),
                    "login": self.repository_owner,
                    "id": generate_id(),
                    "node_id": format!("MDQ6VXNlcnt}", generate_id()),
                    "avatar_url": format!("https://avatars.githubusercontent.com/u/{}?v=4", generate_id()),
                    "gravatar_id": "",
                    "url": format!("https://api.github.com/users/{}", self.repository_owner),
                    "html_url": format!("https://github.com/{}", self.repository_owner),
                    "type": "User",
                    "site_admin": false
                },
                "html_url": format!("https://github.com/{}/{}", self.repository_owner, self.repository_name),
                "description": "A test repository for Release Regent",
                "fork": false,
                "url": format!("https://api.github.com/repos/{}/{}", self.repository_owner, self.repository_name),
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": generate_iso_timestamp(),
                "pushed_at": generate_iso_timestamp(),
                "git_url": format!("git://github.com/{}/{}.git", self.repository_owner, self.repository_name),
                "ssh_url": format!("git@github.com:{}/{}.git", self.repository_owner, self.repository_name),
                "clone_url": format!("https://github.com/{}/{}.git", self.repository_owner, self.repository_name),
                "size": 1024,
                "stargazers_count": 42,
                "watchers_count": 42,
                "language": "Rust",
                "has_issues": true,
                "has_projects": true,
                "has_wiki": true,
                "has_pages": false,
                "forks_count": 5,
                "archived": false,
                "disabled": false,
                "open_issues_count": 2,
                "license": {
                    "key": "mit",
                    "name": "MIT License",
                    "spdx_id": "MIT",
                    "url": "https://api.github.com/licenses/mit",
                    "node_id": "MDc6TGljZW5zZW1pdA=="
                },
                "allow_forking": true,
                "is_template": false,
                "topics": ["rust", "automation", "releases"],
                "visibility": "public",
                "forks": 5,
                "open_issues": 2,
                "watchers": 42,
                "default_branch": "main",
                "stargazers": 42,
                "master_branch": "main"
            },
            "pusher": {
                "name": self.repository_owner,
                "email": format!("{}@users.noreply.github.com", self.repository_owner)
            },
            "sender": {
                "login": self.repository_owner,
                "id": generate_id(),
                "node_id": format!("MDQ6VXNlcnt}", generate_id()),
                "avatar_url": format!("https://avatars.githubusercontent.com/u/{}?v=4", generate_id()),
                "gravatar_id": "",
                "url": format!("https://api.github.com/users/{}", self.repository_owner),
                "html_url": format!("https://github.com/{}", self.repository_owner),
                "type": "User",
                "site_admin": false
            }
        })
    }

    /// Default commit structure
    fn default_commit() -> Value {
        json!({
            "id": generate_commit_sha(),
            "tree_id": generate_commit_sha(),
            "distinct": true,
            "message": "Initial commit",
            "timestamp": generate_iso_timestamp(),
            "url": format!("https://github.com/owner/repo/commit/{}", generate_commit_sha()),
            "author": {
                "name": generate_full_name(),
                "email": generate_email(),
                "username": generate_github_login()
            },
            "committer": {
                "name": generate_full_name(),
                "email": generate_email(),
                "username": generate_github_login()
            },
            "added": [],
            "removed": [],
            "modified": []
        })
    }
}

impl Default for PushEventBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for GitHub pull request event webhook payloads
#[derive(Debug, Clone)]
pub struct PullRequestEventBuilder {
    action: String,
    number: u64,
    title: String,
    body: Option<String>,
    state: String,
    draft: bool,
    base_ref: String,
    head_ref: String,
    repository_owner: String,
    repository_name: String,
    user_login: String,
}

impl PullRequestEventBuilder {
    /// Create a new pull request event builder with defaults
    pub fn new() -> Self {
        Self {
            action: "opened".to_string(),
            number: generate_pr_number() as u64,
            title: generate_pr_title(),
            body: Some(generate_pr_description()),
            state: "open".to_string(),
            draft: false,
            base_ref: "main".to_string(),
            head_ref: "feature/new-feature".to_string(),
            repository_owner: generate_github_login(),
            repository_name: generate_repo_name(),
            user_login: generate_github_login(),
        }
    }

    /// Set the webhook action
    pub fn with_action(mut self, action: &str) -> Self {
        self.action = action.to_string();
        self
    }

    /// Set PR number
    pub fn with_number(mut self, number: u64) -> Self {
        self.number = number;
        self
    }

    /// Set PR title
    pub fn with_title(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }

    /// Set PR body
    pub fn with_body(mut self, body: Option<&str>) -> Self {
        self.body = body.map(|b| b.to_string());
        self
    }

    /// Set as draft PR
    pub fn as_draft(mut self) -> Self {
        self.draft = true;
        self
    }

    /// Set branch references
    pub fn with_branches(mut self, base: &str, head: &str) -> Self {
        self.base_ref = base.to_string();
        self.head_ref = head.to_string();
        self
    }

    /// Set repository details
    pub fn with_repository(mut self, owner: &str, name: &str) -> Self {
        self.repository_owner = owner.to_string();
        self.repository_name = name.to_string();
        self
    }

    /// Set user who created the PR
    pub fn with_user(mut self, login: &str) -> Self {
        self.user_login = login.to_string();
        self
    }

    /// Create as opened PR
    pub fn as_opened(mut self) -> Self {
        self.action = "opened".to_string();
        self.state = "open".to_string();
        self
    }

    /// Create as closed PR
    pub fn as_closed(mut self) -> Self {
        self.action = "closed".to_string();
        self.state = "closed".to_string();
        self
    }

    /// Create as merged PR
    pub fn as_merged(mut self) -> Self {
        self.action = "closed".to_string();
        self.state = "closed".to_string();
        self
    }

    /// Create as synchronize event (new commits pushed)
    pub fn as_synchronize(mut self) -> Self {
        self.action = "synchronize".to_string();
        self
    }

    /// Build the webhook payload
    pub fn build(self) -> Value {
        let merged = self.action == "closed" && self.state == "closed";
        let merged_at = if merged {
            Some(generate_iso_timestamp())
        } else {
            None
        };

        json!({
            "action": self.action,
            "number": self.number,
            "pull_request": {
                "url": format!("https://api.github.com/repos/{}/{}/pulls/{}",
                              self.repository_owner, self.repository_name, self.number),
                "id": generate_id(),
                "node_id": format!("MDExOlB1bGxSZXF1ZXN0e}", generate_id()),
                "html_url": format!("https://github.com/{}/{}/pull/{}",
                                   self.repository_owner, self.repository_name, self.number),
                "diff_url": format!("https://github.com/{}/{}/pull/{}.diff",
                                   self.repository_owner, self.repository_name, self.number),
                "patch_url": format!("https://github.com/{}/{}/pull/{}.patch",
                                    self.repository_owner, self.repository_name, self.number),
                "issue_url": format!("https://api.github.com/repos/{}/{}/issues/{}",
                                    self.repository_owner, self.repository_name, self.number),
                "number": self.number,
                "state": self.state,
                "locked": false,
                "title": self.title,
                "user": {
                    "login": self.user_login,
                    "id": generate_id(),
                    "node_id": format!("MDQ6VXNlcnt}", generate_id()),
                    "avatar_url": format!("https://avatars.githubusercontent.com/u/{}?v=4", generate_id()),
                    "gravatar_id": "",
                    "url": format!("https://api.github.com/users/{}", self.user_login),
                    "html_url": format!("https://github.com/{}", self.user_login),
                    "type": "User",
                    "site_admin": false
                },
                "body": self.body,
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": generate_iso_timestamp(),
                "closed_at": if self.state == "closed" { Some(generate_iso_timestamp()) } else { None },
                "merged_at": merged_at,
                "merge_commit_sha": if merged { Some(generate_commit_sha()) } else { None },
                "assignee": null,
                "assignees": [],
                "requested_reviewers": [],
                "requested_teams": [],
                "labels": [],
                "milestone": null,
                "draft": self.draft,
                "commits_url": format!("https://api.github.com/repos/{}/{}/pulls/{}/commits",
                                      self.repository_owner, self.repository_name, self.number),
                "review_comments_url": format!("https://api.github.com/repos/{}/{}/pulls/{}/comments",
                                              self.repository_owner, self.repository_name, self.number),
                "review_comment_url": format!("https://api.github.com/repos/{}/{}/pulls/comments/{{number}}",
                                             self.repository_owner, self.repository_name),
                "comments_url": format!("https://api.github.com/repos/{}/{}/issues/{}/comments",
                                       self.repository_owner, self.repository_name, self.number),
                "statuses_url": format!("https://api.github.com/repos/{}/{}/statuses/{}",
                                       self.repository_owner, self.repository_name, generate_commit_sha()),
                "head": {
                    "label": format!("{}:{}", self.user_login, self.head_ref),
                    "ref": self.head_ref,
                    "sha": generate_commit_sha(),
                    "user": {
                        "login": self.user_login,
                        "id": generate_id(),
                        "type": "User"
                    },
                    "repo": {
                        "id": generate_id(),
                        "name": self.repository_name,
                        "full_name": format!("{}/{}", self.repository_owner, self.repository_name),
                        "private": false,
                        "owner": {
                            "login": self.repository_owner,
                            "id": generate_id(),
                            "type": "User"
                        },
                        "html_url": format!("https://github.com/{}/{}", self.repository_owner, self.repository_name),
                        "clone_url": format!("https://github.com/{}/{}.git", self.repository_owner, self.repository_name),
                        "default_branch": "main"
                    }
                },
                "base": {
                    "label": format!("{}:{}", self.repository_owner, self.base_ref),
                    "ref": self.base_ref,
                    "sha": generate_commit_sha(),
                    "user": {
                        "login": self.repository_owner,
                        "id": generate_id(),
                        "type": "User"
                    },
                    "repo": {
                        "id": generate_id(),
                        "name": self.repository_name,
                        "full_name": format!("{}/{}", self.repository_owner, self.repository_name),
                        "private": false,
                        "owner": {
                            "login": self.repository_owner,
                            "id": generate_id(),
                            "type": "User"
                        },
                        "html_url": format!("https://github.com/{}/{}", self.repository_owner, self.repository_name),
                        "clone_url": format!("https://github.com/{}/{}.git", self.repository_owner, self.repository_name),
                        "default_branch": "main"
                    }
                },
                "author_association": "CONTRIBUTOR",
                "merged": merged,
                "mergeable": null,
                "rebaseable": null,
                "mergeable_state": "unknown",
                "merged_by": if merged {
                    Some(json!({
                        "login": self.repository_owner,
                        "id": generate_id(),
                        "type": "User"
                    }))
                } else {
                    None
                },
                "comments": 0,
                "review_comments": 0,
                "maintainer_can_modify": false,
                "commits": 1,
                "additions": 10,
                "deletions": 5,
                "changed_files": 2
            },
            "repository": {
                "id": generate_id(),
                "name": self.repository_name,
                "full_name": format!("{}/{}", self.repository_owner, self.repository_name),
                "private": false,
                "owner": {
                    "login": self.repository_owner,
                    "id": generate_id(),
                    "type": "User"
                },
                "html_url": format!("https://github.com/{}/{}", self.repository_owner, self.repository_name),
                "description": "A test repository",
                "clone_url": format!("https://github.com/{}/{}.git", self.repository_owner, self.repository_name),
                "default_branch": "main"
            },
            "sender": {
                "login": self.user_login,
                "id": generate_id(),
                "type": "User"
            }
        })
    }
}

impl Default for PullRequestEventBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for GitHub release event webhook payloads
#[derive(Debug, Clone)]
pub struct ReleaseEventBuilder {
    action: String,
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    draft: bool,
    prerelease: bool,
    repository_owner: String,
    repository_name: String,
    author_login: String,
}

impl ReleaseEventBuilder {
    /// Create a new release event builder with defaults
    pub fn new() -> Self {
        Self {
            action: "published".to_string(),
            tag_name: "v1.0.0".to_string(),
            name: Some("Release v1.0.0".to_string()),
            body: Some(generate_release_notes()),
            draft: false,
            prerelease: false,
            repository_owner: generate_github_login(),
            repository_name: generate_repo_name(),
            author_login: generate_github_login(),
        }
    }

    /// Set the webhook action
    pub fn with_action(mut self, action: &str) -> Self {
        self.action = action.to_string();
        self
    }

    /// Set tag name
    pub fn with_tag_name(mut self, tag_name: &str) -> Self {
        self.tag_name = tag_name.to_string();
        self
    }

    /// Set release name
    pub fn with_name(mut self, name: Option<&str>) -> Self {
        self.name = name.map(|n| n.to_string());
        self
    }

    /// Set release body
    pub fn with_body(mut self, body: Option<&str>) -> Self {
        self.body = body.map(|b| b.to_string());
        self
    }

    /// Set as draft release
    pub fn as_draft(mut self) -> Self {
        self.draft = true;
        self
    }

    /// Set as prerelease
    pub fn as_prerelease(mut self) -> Self {
        self.prerelease = true;
        self
    }

    /// Set repository details
    pub fn with_repository(mut self, owner: &str, name: &str) -> Self {
        self.repository_owner = owner.to_string();
        self.repository_name = name.to_string();
        self
    }

    /// Set release author
    pub fn with_author(mut self, login: &str) -> Self {
        self.author_login = login.to_string();
        self
    }

    /// Create as published release
    pub fn as_published(mut self) -> Self {
        self.action = "published".to_string();
        self.draft = false;
        self
    }

    /// Create as created release (draft)
    pub fn as_created(mut self) -> Self {
        self.action = "created".to_string();
        self.draft = true;
        self
    }

    /// Create as edited release
    pub fn as_edited(mut self) -> Self {
        self.action = "edited".to_string();
        self
    }

    /// Create as deleted release
    pub fn as_deleted(mut self) -> Self {
        self.action = "deleted".to_string();
        self
    }

    /// Build the webhook payload
    pub fn build(self) -> Value {
        json!({
            "action": self.action,
            "release": {
                "url": format!("https://api.github.com/repos/{}/{}/releases/{}",
                              self.repository_owner, self.repository_name, generate_id()),
                "assets_url": format!("https://api.github.com/repos/{}/{}/releases/{}/assets",
                                     self.repository_owner, self.repository_name, generate_id()),
                "upload_url": format!("https://uploads.github.com/repos/{}/{}/releases/{}/assets{{?name,label}}",
                                     self.repository_owner, self.repository_name, generate_id()),
                "html_url": format!("https://github.com/{}/{}/releases/tag/{}",
                                   self.repository_owner, self.repository_name, self.tag_name),
                "id": generate_id(),
                "author": {
                    "login": self.author_login,
                    "id": generate_id(),
                    "node_id": format!("MDQ6VXNlcnt}", generate_id()),
                    "avatar_url": format!("https://avatars.githubusercontent.com/u/{}?v=4", generate_id()),
                    "gravatar_id": "",
                    "url": format!("https://api.github.com/users/{}", self.author_login),
                    "html_url": format!("https://github.com/{}", self.author_login),
                    "type": "User",
                    "site_admin": false
                },
                "node_id": format!("MDc6UmVsZWFzZX{}", generate_id()),
                "tag_name": self.tag_name,
                "target_commitish": "main",
                "name": self.name,
                "draft": self.draft,
                "prerelease": self.prerelease,
                "created_at": "2024-01-01T00:00:00Z",
                "published_at": if self.draft { None } else { Some(generate_iso_timestamp()) },
                "assets": [],
                "tarball_url": format!("https://api.github.com/repos/{}/{}/tarball/{}",
                                      self.repository_owner, self.repository_name, self.tag_name),
                "zipball_url": format!("https://api.github.com/repos/{}/{}/zipball/{}",
                                      self.repository_owner, self.repository_name, self.tag_name),
                "body": self.body
            },
            "repository": {
                "id": generate_id(),
                "name": self.repository_name,
                "full_name": format!("{}/{}", self.repository_owner, self.repository_name),
                "private": false,
                "owner": {
                    "login": self.repository_owner,
                    "id": generate_id(),
                    "type": "User"
                },
                "html_url": format!("https://github.com/{}/{}", self.repository_owner, self.repository_name),
                "description": "A test repository",
                "clone_url": format!("https://github.com/{}/{}.git", self.repository_owner, self.repository_name),
                "default_branch": "main"
            },
            "sender": {
                "login": self.author_login,
                "id": generate_id(),
                "type": "User"
            }
        })
    }
}

impl Default for ReleaseEventBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience functions for common webhook scenarios

/// Generate a GitHub push event webhook payload
pub fn github_push_event() -> Value {
    PushEventBuilder::new().build()
}

/// Generate a GitHub push event with conventional commits
pub fn github_push_event_with_conventional_commits() -> Value {
    PushEventBuilder::new().with_conventional_commits().build()
}

/// Generate a GitHub pull request opened event
pub fn github_pull_request_opened() -> Value {
    PullRequestEventBuilder::new().as_opened().build()
}

/// Generate a GitHub pull request closed event
pub fn github_pull_request_closed() -> Value {
    PullRequestEventBuilder::new().as_closed().build()
}

/// Generate a GitHub pull request merged event
pub fn github_pull_request_merged() -> Value {
    PullRequestEventBuilder::new().as_merged().build()
}

/// Generate a GitHub release published event
pub fn github_release_published() -> Value {
    ReleaseEventBuilder::new().as_published().build()
}

/// Generate a GitHub release created (draft) event
pub fn github_release_created() -> Value {
    ReleaseEventBuilder::new().as_created().build()
}
