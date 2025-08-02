//! Version builder for creating test version data

use crate::builders::TestDataBuilder;
use release_regent_core::versioning::SemanticVersion;

/// Builder for creating test semantic version data
#[derive(Debug, Clone)]
pub struct VersionBuilder {
    major: u64,
    minor: u64,
    patch: u64,
    prerelease: Option<String>,
    build: Option<String>,
}

impl VersionBuilder {
    /// Create a new version builder with defaults
    pub fn new() -> Self {
        Self {
            major: 1,
            minor: 0,
            patch: 0,
            prerelease: None,
            build: None,
        }
    }

    /// Set major version
    pub fn with_major(mut self, major: u64) -> Self {
        self.major = major;
        self
    }

    /// Set minor version
    pub fn with_minor(mut self, minor: u64) -> Self {
        self.minor = minor;
        self
    }

    /// Set patch version
    pub fn with_patch(mut self, patch: u64) -> Self {
        self.patch = patch;
        self
    }

    /// Set prerelease identifier
    pub fn with_prerelease(mut self, prerelease: &str) -> Self {
        self.prerelease = Some(prerelease.to_string());
        self
    }

    /// Set build metadata
    pub fn with_build(mut self, build: &str) -> Self {
        self.build = Some(build.to_string());
        self
    }
}

impl Default for VersionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TestDataBuilder<SemanticVersion> for VersionBuilder {
    fn build(self) -> SemanticVersion {
        SemanticVersion {
            major: self.major,
            minor: self.minor,
            patch: self.patch,
            prerelease: self.prerelease,
            build: self.build,
        }
    }

    fn reset(self) -> Self {
        Self::new()
    }
}
