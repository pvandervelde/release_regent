//! Test data builders for creating realistic test data
//!
//! This module provides builder pattern implementations for creating
//! test data with realistic values and proper relationships.

pub mod commit_builder;
pub mod configuration_builder;
pub mod pull_request_builder;
pub mod release_builder;
pub mod repository_builder;
pub mod version_builder;
pub mod version_context_builder;
pub mod webhook_builder;

pub use commit_builder::CommitBuilder;
pub use configuration_builder::ConfigurationBuilder;
pub use pull_request_builder::PullRequestBuilder;
pub use release_builder::ReleaseBuilder;
pub use repository_builder::RepositoryBuilder;
pub use version_builder::VersionBuilder;
pub use version_context_builder::VersionContextBuilder;
pub use webhook_builder::WebhookBuilder;

/// Base trait for all test data builders
///
/// Provides common functionality for resetting builder state
/// and ensuring consistent builder patterns across all implementations.
pub trait TestDataBuilder<T> {
    /// Build the final test data object
    ///
    /// # Returns
    /// The constructed test data object
    fn build(self) -> T;

    /// Reset the builder to default state
    ///
    /// # Returns
    /// Builder instance with default values
    fn reset(self) -> Self;
}

/// Helper functions for generating realistic test data
pub mod helpers {
    use chrono::{DateTime, Utc};
    use rand::Rng;

    /// Generate a realistic Git SHA
    ///
    /// # Returns
    /// 40-character hexadecimal Git SHA
    pub fn generate_git_sha() -> String {
        let mut rng = rand::thread_rng();

        // Generate 40 hex characters
        (0..40)
            .map(|_| {
                let hex_chars = b"0123456789abcdef";
                hex_chars[rng.gen_range(0..16)] as char
            })
            .collect()
    }

    /// Generate a realistic GitHub user login
    ///
    /// # Returns
    /// GitHub-style username
    pub fn generate_github_login() -> String {
        let adjectives = ["happy", "clever", "bright", "swift", "gentle"];
        let nouns = ["cat", "dog", "bird", "fish", "bear"];
        let mut rng = rand::thread_rng();

        format!(
            "{}{}{}",
            adjectives[rng.gen_range(0..adjectives.len())],
            nouns[rng.gen_range(0..nouns.len())],
            rng.gen_range(100..999)
        )
    }

    /// Generate a realistic email address
    ///
    /// # Returns
    /// Valid email address for testing
    pub fn generate_email() -> String {
        format!("{}@example.com", generate_github_login())
    }

    /// Generate a timestamp within the last 30 days
    ///
    /// # Returns
    /// Recent timestamp for realistic test data
    pub fn generate_recent_timestamp() -> DateTime<Utc> {
        let mut rng = rand::thread_rng();
        let days_ago = rng.gen_range(0..30);
        let hours_ago = rng.gen_range(0..24);
        let minutes_ago = rng.gen_range(0..60);

        Utc::now()
            - chrono::Duration::days(days_ago)
            - chrono::Duration::hours(hours_ago)
            - chrono::Duration::minutes(minutes_ago)
    }

    /// Generate a realistic repository name
    ///
    /// # Returns
    /// GitHub-style repository name
    pub fn generate_repo_name() -> String {
        let prefixes = ["awesome", "super", "mega", "ultra", "hyper"];
        let subjects = ["tool", "lib", "app", "service", "utility"];
        let mut rng = rand::thread_rng();

        format!(
            "{}-{}",
            prefixes[rng.gen_range(0..prefixes.len())],
            subjects[rng.gen_range(0..subjects.len())]
        )
    }

    /// Generate a unique ID
    pub fn generate_id() -> u64 {
        let mut rng = rand::thread_rng();
        rng.gen_range(100000..999999)
    }

    /// Generate a full name for testing
    pub fn generate_full_name() -> String {
        let first_names = ["Alice", "Bob", "Charlie", "Diana", "Eve"];
        let last_names = ["Smith", "Johnson", "Williams", "Brown", "Jones"];
        let mut rng = rand::thread_rng();

        format!(
            "{} {}",
            first_names[rng.gen_range(0..first_names.len())],
            last_names[rng.gen_range(0..last_names.len())]
        )
    }

    /// Generate a PR number
    pub fn generate_pr_number() -> u32 {
        let mut rng = rand::thread_rng();
        rng.gen_range(1..9999)
    }

    /// Generate a PR title
    pub fn generate_pr_title() -> String {
        let prefixes = ["Add", "Fix", "Update", "Remove", "Refactor"];
        let subjects = [
            "authentication",
            "validation",
            "error handling",
            "documentation",
            "tests",
        ];
        let mut rng = rand::thread_rng();

        format!(
            "{} {}",
            prefixes[rng.gen_range(0..prefixes.len())],
            subjects[rng.gen_range(0..subjects.len())]
        )
    }

    /// Generate a PR description
    pub fn generate_pr_description() -> String {
        "This pull request implements important changes to improve the codebase.".to_string()
    }

    /// Generate release notes
    pub fn generate_release_notes() -> String {
        "## What's Changed\n\n* Bug fixes and improvements\n* Performance enhancements".to_string()
    }

    /// Generate a commit SHA (alias for git SHA)
    pub fn generate_commit_sha() -> String {
        generate_git_sha()
    }

    /// Generate an ISO timestamp string
    pub fn generate_iso_timestamp() -> String {
        generate_recent_timestamp().to_rfc3339()
    }
}
