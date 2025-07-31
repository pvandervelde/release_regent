//! GitHub API response fixtures
//!
//! This module provides realistic GitHub API response data that matches actual GitHub API structures.
//! All fixtures are based on real GitHub API responses and can be used for testing
//! API integration logic.

use crate::builders::helpers::*;
use release_regent_core::traits::github_operations::*;
use serde_json::{json, Value};

/// Builder for GitHub repository API responses
#[derive(Debug, Clone)]
pub struct RepositoryResponseBuilder {
    id: u64,
    name: String,
    full_name: String,
    owner: String,
    description: Option<String>,
    private: bool,
    default_branch: String,
}

impl RepositoryResponseBuilder {
    /// Create a new repository response builder with defaults
    pub fn new() -> Self {
        let owner = generate_github_login();
        let name = generate_repo_name();

        Self {
            id: generate_id(),
            name: name.clone(),
            full_name: format!("{}/{}", owner, name),
            owner,
            description: Some("A test repository for Release Regent".to_string()),
            private: false,
            default_branch: "main".to_string(),
        }
    }

    /// Set repository ID
    pub fn with_id(mut self, id: u64) -> Self {
        self.id = id;
        self
    }

    /// Set repository name
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self.full_name = format!("{}/{}", self.owner, name);
        self
    }

    /// Set repository owner
    pub fn with_owner(mut self, owner: &str) -> Self {
        self.owner = owner.to_string();
        self.full_name = format!("{}/{}", owner, self.name);
        self
    }

    /// Set repository description
    pub fn with_description(mut self, description: Option<&str>) -> Self {
        self.description = description.map(|d| d.to_string());
        self
    }

    /// Set as private repository
    pub fn as_private(mut self) -> Self {
        self.private = true;
        self
    }

    /// Set default branch
    pub fn with_default_branch(mut self, branch: &str) -> Self {
        self.default_branch = branch.to_string();
        self
    }

    /// Build the API response
    pub fn build(self) -> Repository {
        Repository {
            id: self.id,
            name: self.name.clone(),
            full_name: self.full_name.clone(),
            private: self.private,
            owner: self.owner.clone(),
            description: self.description.clone(),
            ssh_url: format!("git@github.com:{}.git", self.full_name),
            clone_url: format!("https://github.com/{}.git", self.full_name),
            homepage: None,
            default_branch: self.default_branch,
        }
    }

    /// Build as JSON response
    pub fn build_json(self) -> Value {
        json!({
            "id": self.id,
            "node_id": format!("MDEwOlJlcG9zaXRvcnk{}", self.id),
            "name": self.name,
            "full_name": self.full_name,
            "private": self.private,
            "owner": {
                "login": self.owner,
                "id": generate_id(),
                "node_id": format!("MDQ6VXNlcnt}", generate_id()),
                "avatar_url": format!("https://avatars.githubusercontent.com/u/{}?v=4", generate_id()),
                "gravatar_id": "",
                "url": format!("https://api.github.com/users/{}", self.owner),
                "html_url": format!("https://github.com/{}", self.owner),
                "type": "User",
                "site_admin": false
            },
            "html_url": format!("https://github.com/{}", self.full_name),
            "description": self.description,
            "fork": false,
            "url": format!("https://api.github.com/repos/{}", self.full_name),
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": generate_iso_timestamp(),
            "pushed_at": generate_iso_timestamp(),
            "git_url": format!("git://github.com/{}.git", self.full_name),
            "ssh_url": format!("git@github.com:{}.git", self.full_name),
            "clone_url": format!("https://github.com/{}.git", self.full_name),
            "svn_url": format!("https://github.com/{}", self.full_name),
            "homepage": null,
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
            "default_branch": self.default_branch,
            "temp_clone_token": null,
            "network_count": 5,
            "subscribers_count": 10
        })
    }
}

impl Default for RepositoryResponseBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for GitHub pull request API responses
#[derive(Debug, Clone)]
pub struct PullRequestResponseBuilder {
    id: u64,
    number: u64,
    title: String,
    body: Option<String>,
    state: String,
    draft: bool,
    base_ref: String,
    head_ref: String,
    user_login: String,
    repository_owner: String,
    repository_name: String,
}

impl PullRequestResponseBuilder {
    /// Create a new pull request response builder with defaults
    pub fn new() -> Self {
        Self {
            id: generate_id(),
            number: generate_pr_number() as u64,
            title: generate_pr_title(),
            body: Some(generate_pr_description()),
            state: "open".to_string(),
            draft: false,
            base_ref: "main".to_string(),
            head_ref: "feature/new-feature".to_string(),
            user_login: generate_github_login(),
            repository_owner: generate_github_login(),
            repository_name: generate_repo_name(),
        }
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

    /// Set PR state
    pub fn with_state(mut self, state: &str) -> Self {
        self.state = state.to_string();
        self
    }

    /// Set as draft
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

    /// Build as core struct
    pub fn build(self) -> PullRequest {
        let repo = Repository {
            id: generate_id(),
            name: self.repository_name.clone(),
            full_name: format!("{}/{}", self.repository_owner, self.repository_name),
            private: false,
            owner: self.repository_owner.clone(),
            description: Some("Test repository".to_string()),
            ssh_url: format!(
                "git@github.com:{}/{}.git",
                self.repository_owner, self.repository_name
            ),
            clone_url: format!(
                "https://github.com/{}/{}.git",
                self.repository_owner, self.repository_name
            ),
            homepage: None,
            default_branch: "main".to_string(),
        };

        PullRequest {
            number: self.number,
            title: self.title,
            body: self.body,
            state: self.state,
            draft: self.draft,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            merged_at: None,
            base: PullRequestBranch {
                ref_name: self.base_ref,
                sha: generate_commit_sha(),
                repo: repo.clone(),
            },
            head: PullRequestBranch {
                ref_name: self.head_ref,
                sha: generate_commit_sha(),
                repo,
            },
            user: GitUser {
                login: Some(self.user_login.clone()),
                name: generate_full_name(),
                email: generate_email(),
            },
        }
    }

    /// Build as JSON response
    pub fn build_json(self) -> Value {
        json!({
            "url": format!("https://api.github.com/repos/{}/{}/pulls/{}",
                          self.repository_owner, self.repository_name, self.number),
            "id": self.id,
            "node_id": format!("MDExOlB1bGxSZXF1ZXN0e}", self.id),
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
            "merged_at": null,
            "merge_commit_sha": null,
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
                }
            },
            "author_association": "CONTRIBUTOR",
            "merged": false,
            "mergeable": true,
            "rebaseable": true,
            "mergeable_state": "clean",
            "merged_by": null,
            "comments": 0,
            "review_comments": 0,
            "maintainer_can_modify": false,
            "commits": 1,
            "additions": 10,
            "deletions": 5,
            "changed_files": 2
        })
    }
}

impl Default for PullRequestResponseBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for GitHub release API responses
#[derive(Debug, Clone)]
pub struct ReleaseResponseBuilder {
    id: u64,
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    draft: bool,
    prerelease: bool,
    author_login: String,
    repository_owner: String,
    repository_name: String,
}

impl ReleaseResponseBuilder {
    /// Create a new release response builder with defaults
    pub fn new() -> Self {
        Self {
            id: generate_id(),
            tag_name: "v1.0.0".to_string(),
            name: Some("Release v1.0.0".to_string()),
            body: Some(generate_release_notes()),
            draft: false,
            prerelease: false,
            author_login: generate_github_login(),
            repository_owner: generate_github_login(),
            repository_name: generate_repo_name(),
        }
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

    /// Set as draft
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

    /// Build as core struct
    pub fn build(self) -> Release {
        Release {
            id: self.id,
            tag_name: self.tag_name,
            target_commitish: "main".to_string(),
            name: self.name,
            body: self.body,
            draft: self.draft,
            prerelease: self.prerelease,
            created_at: chrono::Utc::now(),
            published_at: if self.draft {
                None
            } else {
                Some(chrono::Utc::now())
            },
            author: GitUser {
                login: Some(self.author_login.clone()),
                name: generate_full_name(),
                email: generate_email(),
            },
        }
    }

    /// Build as JSON response
    pub fn build_json(self) -> Value {
        json!({
            "url": format!("https://api.github.com/repos/{}/{}/releases/{}",
                          self.repository_owner, self.repository_name, self.id),
            "assets_url": format!("https://api.github.com/repos/{}/{}/releases/{}/assets",
                                 self.repository_owner, self.repository_name, self.id),
            "upload_url": format!("https://uploads.github.com/repos/{}/{}/releases/{}/assets{{?name,label}}",
                                 self.repository_owner, self.repository_name, self.id),
            "html_url": format!("https://github.com/{}/{}/releases/tag/{}",
                               self.repository_owner, self.repository_name, self.tag_name),
            "id": self.id,
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
            "node_id": format!("MDc6UmVsZWFzZX{}", self.id),
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
        })
    }
}

impl Default for ReleaseResponseBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience functions for common API responses

/// Generate a sample repository response
pub fn sample_repository() -> Repository {
    RepositoryResponseBuilder::new().build()
}

/// Generate a sample repository JSON response
pub fn sample_repository_json() -> Value {
    RepositoryResponseBuilder::new().build_json()
}

/// Generate a sample pull request response
pub fn sample_pull_request() -> PullRequest {
    PullRequestResponseBuilder::new().build()
}

/// Generate a sample pull request JSON response
pub fn sample_pull_request_json() -> Value {
    PullRequestResponseBuilder::new().build_json()
}

/// Generate a sample release response
pub fn sample_release() -> Release {
    ReleaseResponseBuilder::new().build()
}

/// Generate a sample release JSON response
pub fn sample_release_json() -> Value {
    ReleaseResponseBuilder::new().build_json()
}

/// Generate a list of commits response
pub fn sample_commits_list() -> Value {
    json!([
        {
            "sha": generate_commit_sha(),
            "node_id": format!("MDY6Q29tbWl0e}", generate_id()),
            "commit": {
                "author": {
                    "name": generate_full_name(),
                    "email": generate_email(),
                    "date": generate_iso_timestamp()
                },
                "committer": {
                    "name": generate_full_name(),
                    "email": generate_email(),
                    "date": generate_iso_timestamp()
                },
                "message": "feat: add new authentication feature",
                "tree": {
                    "sha": generate_commit_sha(),
                    "url": format!("https://api.github.com/repos/owner/repo/git/trees/{}", generate_commit_sha())
                },
                "url": format!("https://api.github.com/repos/owner/repo/git/commits/{}", generate_commit_sha())
            },
            "url": format!("https://api.github.com/repos/owner/repo/commits/{}", generate_commit_sha()),
            "html_url": format!("https://github.com/owner/repo/commit/{}", generate_commit_sha()),
            "author": {
                "login": generate_github_login(),
                "id": generate_id(),
                "type": "User"
            },
            "committer": {
                "login": generate_github_login(),
                "id": generate_id(),
                "type": "User"
            },
            "parents": [
                {
                    "sha": generate_commit_sha(),
                    "url": format!("https://api.github.com/repos/owner/repo/commits/{}", generate_commit_sha()),
                    "html_url": format!("https://github.com/owner/repo/commit/{}", generate_commit_sha())
                }
            ]
        }
    ])
}

/// Generate error response for API calls
pub fn error_response(status: u16, message: &str) -> Value {
    json!({
        "message": message,
        "documentation_url": "https://docs.github.com/rest"
    })
}

/// Generate rate limit exceeded response
pub fn rate_limit_exceeded_response() -> Value {
    error_response(403, "API rate limit exceeded for user")
}
