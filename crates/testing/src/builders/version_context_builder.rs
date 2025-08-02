//! Version context builder for creating test version calculation contexts

use crate::builders::{helpers::*, TestDataBuilder};
use release_regent_core::{
    traits::version_calculator::VersionContext, versioning::SemanticVersion,
};

/// Builder for creating test version context data
#[derive(Debug, Clone)]
pub struct VersionContextBuilder {
    owner: String,
    repo: String,
    current_version: Option<SemanticVersion>,
    target_branch: String,
    base_ref: Option<String>,
    head_ref: String,
}

impl VersionContextBuilder {
    /// Create a new version context builder with defaults
    pub fn new() -> Self {
        Self {
            owner: generate_github_login(),
            repo: generate_repo_name(),
            current_version: Some(SemanticVersion {
                major: 1,
                minor: 0,
                patch: 0,
                prerelease: None,
                build: None,
            }),
            target_branch: "main".to_string(),
            base_ref: Some("v1.0.0".to_string()),
            head_ref: "HEAD".to_string(),
        }
    }

    /// Set repository owner
    pub fn with_owner(mut self, owner: &str) -> Self {
        self.owner = owner.to_string();
        self
    }

    /// Set repository name
    pub fn with_repo(mut self, repo: &str) -> Self {
        self.repo = repo.to_string();
        self
    }

    /// Set repository from owner/repo string
    pub fn with_repository(mut self, repository: &str) -> Self {
        if let Some((owner, repo)) = repository.split_once('/') {
            self.owner = owner.to_string();
            self.repo = repo.to_string();
        }
        self
    }

    /// Set current version
    pub fn with_current_version(mut self, version: SemanticVersion) -> Self {
        self.current_version = Some(version);
        self
    }

    /// Set current version from semver string
    pub fn with_current_version_string(mut self, version: &str) -> Self {
        if let Some(parsed) = self.parse_semver(version) {
            self.current_version = Some(parsed);
        }
        self
    }

    /// Set as new repository (no current version)
    pub fn as_new_repository(mut self) -> Self {
        self.current_version = None;
        self.base_ref = None;
        self
    }

    /// Set target branch
    pub fn with_target_branch(mut self, branch: &str) -> Self {
        self.target_branch = branch.to_string();
        self
    }

    /// Set base reference
    pub fn with_base_ref(mut self, base_ref: &str) -> Self {
        self.base_ref = Some(base_ref.to_string());
        self
    }

    /// Set head reference
    pub fn with_head_ref(mut self, head_ref: &str) -> Self {
        self.head_ref = head_ref.to_string();
        self
    }

    /// Set for release preparation (from last tag to HEAD)
    pub fn for_release_preparation(mut self) -> Self {
        if let Some(ref version) = self.current_version {
            self.base_ref = Some(format!(
                "v{}.{}.{}",
                version.major, version.minor, version.patch
            ));
            self.head_ref = "HEAD".to_string();
        }
        self
    }

    /// Set for hotfix release
    pub fn for_hotfix_release(mut self, hotfix_branch: &str) -> Self {
        self.target_branch = hotfix_branch.to_string();
        self.head_ref = hotfix_branch.to_string();
        self
    }

    /// Set for prerelease
    pub fn for_prerelease(mut self, prerelease_id: &str) -> Self {
        if let Some(ref mut version) = self.current_version {
            version.prerelease = Some(prerelease_id.to_string());
        }
        self
    }

    /// Set version range for analysis
    pub fn with_version_range(mut self, from_version: &str, to_ref: &str) -> Self {
        self.base_ref = Some(from_version.to_string());
        self.head_ref = to_ref.to_string();
        self
    }

    /// Helper to parse semantic version strings
    fn parse_semver(&self, version_str: &str) -> Option<SemanticVersion> {
        // Simple semver parsing for test purposes
        let version_str = version_str.strip_prefix('v').unwrap_or(version_str);
        let parts: Vec<&str> = version_str.split('.').collect();

        if parts.len() >= 3 {
            if let (Ok(major), Ok(minor), Ok(patch)) = (
                parts[0].parse::<u64>(),
                parts[1].parse::<u64>(),
                parts[2].parse::<u64>(),
            ) {
                return Some(SemanticVersion {
                    major,
                    minor,
                    patch,
                    prerelease: None,
                    build: None,
                });
            }
        }
        None
    }
}

impl Default for VersionContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TestDataBuilder<VersionContext> for VersionContextBuilder {
    fn build(self) -> VersionContext {
        VersionContext {
            owner: self.owner,
            repo: self.repo,
            current_version: self.current_version,
            target_branch: self.target_branch,
            base_ref: self.base_ref,
            head_ref: self.head_ref,
        }
    }

    fn reset(self) -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_context_builder_defaults() {
        let context = VersionContextBuilder::new().build();

        assert!(!context.owner.is_empty());
        assert!(!context.repo.is_empty());
        assert_eq!(context.target_branch, "main");
        assert_eq!(context.head_ref, "HEAD");
        assert!(context.current_version.is_some());

        let version = context.current_version.unwrap();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 0);
    }

    #[test]
    fn test_version_context_builder_repository() {
        let context = VersionContextBuilder::new()
            .with_repository("owner/my-repo")
            .build();

        assert_eq!(context.owner, "owner");
        assert_eq!(context.repo, "my-repo");
    }

    #[test]
    fn test_version_context_builder_current_version() {
        let version = SemanticVersion {
            major: 2,
            minor: 1,
            patch: 3,
            prerelease: None,
            build: None,
        };

        let context = VersionContextBuilder::new()
            .with_current_version(version.clone())
            .build();

        assert_eq!(context.current_version, Some(version));
    }

    #[test]
    fn test_version_context_builder_version_string() {
        let context = VersionContextBuilder::new()
            .with_current_version_string("v2.1.3")
            .build();

        let version = context.current_version.unwrap();
        assert_eq!(version.major, 2);
        assert_eq!(version.minor, 1);
        assert_eq!(version.patch, 3);
    }

    #[test]
    fn test_version_context_builder_new_repository() {
        let context = VersionContextBuilder::new().as_new_repository().build();

        assert!(context.current_version.is_none());
        assert!(context.base_ref.is_none());
    }

    #[test]
    fn test_version_context_builder_release_preparation() {
        let context = VersionContextBuilder::new()
            .with_current_version_string("1.2.3")
            .for_release_preparation()
            .build();

        assert_eq!(context.base_ref, Some("v1.2.3".to_string()));
        assert_eq!(context.head_ref, "HEAD");
    }

    #[test]
    fn test_version_context_builder_hotfix() {
        let context = VersionContextBuilder::new()
            .for_hotfix_release("hotfix/critical-fix")
            .build();

        assert_eq!(context.target_branch, "hotfix/critical-fix");
        assert_eq!(context.head_ref, "hotfix/critical-fix");
    }

    #[test]
    fn test_version_context_builder_version_range() {
        let context = VersionContextBuilder::new()
            .with_version_range("v1.0.0", "feature/new-feature")
            .build();

        assert_eq!(context.base_ref, Some("v1.0.0".to_string()));
        assert_eq!(context.head_ref, "feature/new-feature");
    }
}
