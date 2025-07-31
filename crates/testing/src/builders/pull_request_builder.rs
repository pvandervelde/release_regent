//! Pull request builder for creating test GitHub pull request data

use crate::builders::{helpers::*, TestDataBuilder};
use chrono::{DateTime, Utc};
use release_regent_core::traits::github_operations::{
    GitUser, PullRequest, PullRequestBranch, Repository,
};

/// Builder for creating test GitHub pull request data
#[derive(Debug, Clone)]
pub struct PullRequestBuilder {
    base: PullRequestBranch,
    body: Option<String>,
    created_at: DateTime<Utc>,
    draft: bool,
    head: PullRequestBranch,
    merged_at: Option<DateTime<Utc>>,
    number: u64,
    state: String,
    title: String,
    updated_at: DateTime<Utc>,
    user: GitUser,
}

impl PullRequestBuilder {
    /// Create a new pull request builder with defaults
    pub fn new() -> Self {
        let now = Utc::now();
        let repo = Repository {
            clone_url: format!(
                "https://github.com/{}/{}.git",
                generate_github_login(),
                generate_repo_name()
            ),
            default_branch: "main".to_string(),
            description: Some("A test repository".to_string()),
            full_name: format!("{}/{}", generate_github_login(), generate_repo_name()),
            homepage: None,
            id: generate_id(),
            name: generate_repo_name(),
            owner: generate_github_login(),
            private: false,
            ssh_url: format!(
                "git@github.com:{}/{}.git",
                generate_github_login(),
                generate_repo_name()
            ),
        };

        Self {
            base: PullRequestBranch {
                ref_name: "main".to_string(),
                repo: repo.clone(),
                sha: generate_commit_sha(),
            },
            body: Some(generate_pr_description()),
            created_at: now,
            draft: false,
            head: PullRequestBranch {
                ref_name: "feature/new-feature".to_string(),
                repo: repo.clone(),
                sha: generate_commit_sha(),
            },
            merged_at: None,
            number: generate_pr_number() as u64,
            state: "open".to_string(),
            title: generate_pr_title(),
            updated_at: now,
            user: GitUser {
                login: Some(generate_github_login()),
                name: generate_full_name(),
                email: generate_email(),
            },
        }
    }

    /// Set pull request number
    pub fn with_number(mut self, number: u64) -> Self {
        self.number = number;
        self
    }

    /// Set pull request title
    pub fn with_title(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }

    /// Set pull request body
    pub fn with_body(mut self, body: Option<&str>) -> Self {
        self.body = body.map(|b| b.to_string());
        self
    }

    /// Set pull request state
    pub fn with_state(mut self, state: &str) -> Self {
        self.state = state.to_string();
        self
    }

    /// Set as open PR
    pub fn as_open(mut self) -> Self {
        self.state = "open".to_string();
        self.merged_at = None;
        self
    }

    /// Set as closed PR
    pub fn as_closed(mut self) -> Self {
        self.state = "closed".to_string();
        self.merged_at = None;
        self
    }

    /// Set as merged PR
    pub fn as_merged(mut self) -> Self {
        self.state = "merged".to_string();
        self.merged_at = Some(Utc::now());
        self
    }

    /// Set as draft PR
    pub fn as_draft(mut self) -> Self {
        self.draft = true;
        self
    }

    /// Set head branch
    pub fn with_head_ref(mut self, ref_name: &str) -> Self {
        self.head.ref_name = ref_name.to_string();
        self
    }

    /// Set base branch
    pub fn with_base_ref(mut self, ref_name: &str) -> Self {
        self.base.ref_name = ref_name.to_string();
        self
    }

    /// Set head SHA
    pub fn with_head_sha(mut self, sha: &str) -> Self {
        self.head.sha = sha.to_string();
        self
    }

    /// Set base SHA
    pub fn with_base_sha(mut self, sha: &str) -> Self {
        self.base.sha = sha.to_string();
        self
    }

    /// Set pull request author
    pub fn with_user(mut self, user: GitUser) -> Self {
        self.user = user;
        self
    }

    /// Set repository for both head and base
    pub fn with_repository(mut self, repository: Repository) -> Self {
        self.head.repo = repository.clone();
        self.base.repo = repository;
        self
    }

    /// Set created timestamp
    pub fn created_at(mut self, timestamp: DateTime<Utc>) -> Self {
        self.created_at = timestamp;
        self
    }

    /// Set updated timestamp
    pub fn updated_at(mut self, timestamp: DateTime<Utc>) -> Self {
        self.updated_at = timestamp;
        self
    }

    /// Set merged timestamp
    pub fn merged_at(mut self, timestamp: Option<DateTime<Utc>>) -> Self {
        self.merged_at = timestamp;
        self
    }

    /// Create feature branch PR
    pub fn feature_branch() -> Self {
        Self::new()
            .with_head_ref("feature/awesome-feature")
            .with_title("Add awesome new feature")
            .with_body(Some(
                "This PR adds an awesome new feature that will improve user experience.",
            ))
    }

    /// Create hotfix PR
    pub fn hotfix() -> Self {
        Self::new()
            .with_head_ref("hotfix/critical-bug")
            .with_title("Fix critical bug in production")
            .with_body(Some(
                "Urgent fix for critical bug affecting production users.",
            ))
    }

    /// Create release PR
    pub fn release_pr() -> Self {
        Self::new()
            .with_head_ref("release/v1.0.0")
            .with_title("Release v1.0.0")
            .with_body(Some("Release candidate for version 1.0.0"))
    }
}

impl TestDataBuilder<PullRequest> for PullRequestBuilder {
    fn build(self) -> PullRequest {
        PullRequest {
            base: self.base,
            body: self.body,
            created_at: self.created_at,
            draft: self.draft,
            head: self.head,
            merged_at: self.merged_at,
            number: self.number,
            state: self.state,
            title: self.title,
            updated_at: self.updated_at,
            user: self.user,
        }
    }

    fn reset(self) -> Self {
        Self::new()
    }
}

impl Default for PullRequestBuilder {
    fn default() -> Self {
        Self::new()
    }
}
