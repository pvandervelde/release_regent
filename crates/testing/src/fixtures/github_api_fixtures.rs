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
        let owner_data = json!({
            "login": self.owner,
            "id": generate_id(),
            "node_id": format!("MDQ6VXNlcn{}", generate_id()),
            "avatar_url": format!("https://avatars.githubusercontent.com/u/{}?v=4", generate_id()),
            "gravatar_id": "",
            "url": format!("https://api.github.com/users/{}", self.owner),
            "html_url": format!("https://github.com/{}", self.owner),
            "type": "User",
            "site_admin": false
        });

        let license_data = json!({
            "key": "mit",
            "name": "MIT License",
            "spdx_id": "MIT",
            "url": "https://api.github.com/licenses/mit",
            "node_id": "MDc6TGljZW5zZW1pdA=="
        });

        // Build JSON in parts to avoid macro recursion limits
        let mut result = serde_json::Map::new();

        result.insert("id".to_string(), json!(self.id));
        result.insert(
            "node_id".to_string(),
            json!(format!("MDEwOlJlcG9zaXRvcnk{}", self.id)),
        );
        result.insert("name".to_string(), json!(self.name));
        result.insert("full_name".to_string(), json!(self.full_name));
        result.insert("private".to_string(), json!(self.private));
        result.insert("owner".to_string(), owner_data);
        result.insert(
            "html_url".to_string(),
            json!(format!("https://github.com/{}", self.full_name)),
        );
        result.insert("description".to_string(), json!(self.description));
        result.insert("fork".to_string(), json!(false));
        result.insert(
            "url".to_string(),
            json!(format!("https://api.github.com/repos/{}", self.full_name)),
        );
        result.insert("created_at".to_string(), json!("2024-01-01T00:00:00Z"));
        result.insert("updated_at".to_string(), json!(generate_iso_timestamp()));
        result.insert("pushed_at".to_string(), json!(generate_iso_timestamp()));
        result.insert(
            "git_url".to_string(),
            json!(format!("git://github.com/{}.git", self.full_name)),
        );
        result.insert(
            "ssh_url".to_string(),
            json!(format!("git@github.com:{}.git", self.full_name)),
        );
        result.insert(
            "clone_url".to_string(),
            json!(format!("https://github.com/{}.git", self.full_name)),
        );
        result.insert(
            "svn_url".to_string(),
            json!(format!("https://github.com/{}", self.full_name)),
        );
        result.insert("homepage".to_string(), Value::Null);
        result.insert("size".to_string(), json!(1024));
        result.insert("stargazers_count".to_string(), json!(42));
        result.insert("watchers_count".to_string(), json!(42));
        result.insert("language".to_string(), json!("Rust"));
        result.insert("has_issues".to_string(), json!(true));
        result.insert("has_projects".to_string(), json!(true));
        result.insert("has_wiki".to_string(), json!(true));
        result.insert("has_pages".to_string(), json!(false));
        result.insert("forks_count".to_string(), json!(5));
        result.insert("archived".to_string(), json!(false));
        result.insert("disabled".to_string(), json!(false));
        result.insert("open_issues_count".to_string(), json!(2));
        result.insert("license".to_string(), license_data);
        result.insert("allow_forking".to_string(), json!(true));
        result.insert("is_template".to_string(), json!(false));
        result.insert(
            "topics".to_string(),
            json!(["rust", "automation", "releases"]),
        );
        result.insert("visibility".to_string(), json!("public"));
        result.insert("forks".to_string(), json!(5));
        result.insert("open_issues".to_string(), json!(2));
        result.insert("watchers".to_string(), json!(42));
        result.insert("default_branch".to_string(), json!(self.default_branch));
        result.insert("temp_clone_token".to_string(), Value::Null);
        result.insert("network_count".to_string(), json!(5));
        result.insert("subscribers_count".to_string(), json!(10));

        Value::Object(result)
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
        let user_data = json!({
            "login": self.user_login,
            "id": generate_id(),
            "node_id": format!("MDQ6VXNlcn{}", generate_id()),
            "avatar_url": format!("https://avatars.githubusercontent.com/u/{}?v=4", generate_id()),
            "gravatar_id": "",
            "url": format!("https://api.github.com/users/{}", self.user_login),
            "html_url": format!("https://github.com/{}", self.user_login),
            "type": "User",
            "site_admin": false
        });

        let head_data = json!({
            "label": format!("{}:{}", self.user_login, self.head_ref),
            "ref": self.head_ref,
            "sha": generate_commit_sha(),
            "user": {
                "login": self.user_login,
                "id": generate_id(),
                "type": "User"
            }
        });

        let base_data = json!({
            "label": format!("{}:{}", self.repository_owner, self.base_ref),
            "ref": self.base_ref,
            "sha": generate_commit_sha(),
            "user": {
                "login": self.repository_owner,
                "id": generate_id(),
                "type": "User"
            }
        });

        // Build JSON in parts to avoid macro recursion limits
        let mut result = serde_json::Map::new();

        result.insert(
            "url".to_string(),
            json!(format!(
                "https://api.github.com/repos/{}/{}/pulls/{}",
                self.repository_owner, self.repository_name, self.number
            )),
        );
        result.insert("id".to_string(), json!(self.id));
        result.insert(
            "node_id".to_string(),
            json!(format!("MDExOlB1bGxSZXF1ZXN0{}", self.id)),
        );
        result.insert(
            "html_url".to_string(),
            json!(format!(
                "https://github.com/{}/{}/pull/{}",
                self.repository_owner, self.repository_name, self.number
            )),
        );
        result.insert("number".to_string(), json!(self.number));
        result.insert("state".to_string(), json!(self.state));
        result.insert("locked".to_string(), json!(false));
        result.insert("title".to_string(), json!(self.title));
        result.insert("user".to_string(), user_data);
        result.insert("body".to_string(), json!(self.body));
        result.insert("created_at".to_string(), json!("2024-01-01T00:00:00Z"));
        result.insert("updated_at".to_string(), json!(generate_iso_timestamp()));
        result.insert(
            "closed_at".to_string(),
            if self.state == "closed" {
                json!(generate_iso_timestamp())
            } else {
                Value::Null
            },
        );
        result.insert("merged_at".to_string(), Value::Null);
        result.insert("draft".to_string(), json!(self.draft));
        result.insert("head".to_string(), head_data);
        result.insert("base".to_string(), base_data);
        result.insert("author_association".to_string(), json!("CONTRIBUTOR"));
        result.insert("merged".to_string(), json!(false));
        result.insert("mergeable".to_string(), json!(true));
        result.insert("maintainer_can_modify".to_string(), json!(false));
        result.insert("commits".to_string(), json!(1));
        result.insert("additions".to_string(), json!(10));
        result.insert("deletions".to_string(), json!(5));
        result.insert("changed_files".to_string(), json!(2));

        Value::Object(result)
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
        let author_data = json!({
            "login": self.author_login,
            "id": generate_id(),
            "node_id": format!("MDQ6VXNlcn{}", generate_id()),
            "avatar_url": format!("https://avatars.githubusercontent.com/u/{}?v=4", generate_id()),
            "gravatar_id": "",
            "url": format!("https://api.github.com/users/{}", self.author_login),
            "html_url": format!("https://github.com/{}", self.author_login),
            "type": "User",
            "site_admin": false
        });

        // Build JSON in parts to avoid macro recursion limits
        let mut result = serde_json::Map::new();

        result.insert(
            "url".to_string(),
            json!(format!(
                "https://api.github.com/repos/{}/{}/releases/{}",
                self.repository_owner, self.repository_name, self.id
            )),
        );
        result.insert(
            "html_url".to_string(),
            json!(format!(
                "https://github.com/{}/{}/releases/tag/{}",
                self.repository_owner, self.repository_name, self.tag_name
            )),
        );
        result.insert("id".to_string(), json!(self.id));
        result.insert("author".to_string(), author_data);
        result.insert(
            "node_id".to_string(),
            json!(format!("MDc6UmVsZWFzZX{}", self.id)),
        );
        result.insert("tag_name".to_string(), json!(self.tag_name));
        result.insert("target_commitish".to_string(), json!("main"));
        result.insert("name".to_string(), json!(self.name));
        result.insert("draft".to_string(), json!(self.draft));
        result.insert("prerelease".to_string(), json!(self.prerelease));
        result.insert("created_at".to_string(), json!("2024-01-01T00:00:00Z"));
        result.insert(
            "published_at".to_string(),
            if self.draft {
                Value::Null
            } else {
                json!(generate_iso_timestamp())
            },
        );
        result.insert("assets".to_string(), json!([]));
        result.insert(
            "tarball_url".to_string(),
            json!(format!(
                "https://api.github.com/repos/{}/{}/tarball/{}",
                self.repository_owner, self.repository_name, self.tag_name
            )),
        );
        result.insert(
            "zipball_url".to_string(),
            json!(format!(
                "https://api.github.com/repos/{}/{}/zipball/{}",
                self.repository_owner, self.repository_name, self.tag_name
            )),
        );
        result.insert("body".to_string(), json!(self.body));

        Value::Object(result)
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
            "node_id": format!("MDY6Q29tbWl0{}", generate_id()),
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
