//! Changelog generation for Release Regent
//!
//! This module handles generating formatted markdown changelogs from conventional commits
//! with proper categorization and formatting.

use crate::versioning::ConventionalCommit;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

// git-cliff-core integration
use git_cliff_core::{
    changelog::Changelog as GitCliffChangelog, commit::Commit as GitCliffCommit,
    config::Config as GitCliffConfig, release::Release as GitCliffRelease,
};

/// Configuration for changelog generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogConfig {
    /// Whether to include commit authors
    pub include_authors: bool,
    /// Whether to include commit SHAs
    pub include_shas: bool,
    /// Template for changelog sections
    pub section_template: String,
    /// Template for individual commit entries
    pub commit_template: String,
}

impl Default for ChangelogConfig {
    fn default() -> Self {
        Self {
            include_authors: true,
            include_shas: true,
            section_template: "### {title}\n\n{entries}\n".to_string(),
            commit_template: "- {description} [{sha}]".to_string(),
        }
    }
}

/// Changelog generator that creates formatted markdown from conventional commits
pub struct ChangelogGenerator {
    config: ChangelogConfig,
}

impl ChangelogGenerator {
    /// Create a new changelog generator with default configuration
    pub fn new() -> Self {
        Self {
            config: ChangelogConfig::default(),
        }
    }

    /// Create a new changelog generator with custom configuration
    pub fn with_config(config: ChangelogConfig) -> Self {
        Self { config }
    }

    /// Generate a changelog from conventional commits
    pub fn generate_changelog(&self, commits: &[ConventionalCommit]) -> String {
        debug!("Generating changelog from {} commits", commits.len());

        if commits.is_empty() {
            return "No changes in this release.".to_string();
        }

        let sections = self.organize_commits_by_type(commits);
        let mut changelog = String::new();

        // Order sections by importance
        let section_order = vec![
            ("feat", "Features"),
            ("fix", "Bug Fixes"),
            ("perf", "Performance Improvements"),
            ("revert", "Reverts"),
            ("docs", "Documentation"),
            ("style", "Styles"),
            ("refactor", "Code Refactoring"),
            ("test", "Tests"),
            ("build", "Build System"),
            ("ci", "Continuous Integration"),
            ("chore", "Chores"),
        ];

        for (commit_type, title) in &section_order {
            if let Some(commits) = sections.get(*commit_type) {
                let section_content = self.generate_section(title, commits);
                changelog.push_str(&section_content);
            }
        }

        // Add any other commit types not in the standard list
        for (commit_type, commits) in &sections {
            if !section_order.iter().any(|(t, _)| t == commit_type) {
                let title = self.format_commit_type_title(commit_type);
                let section_content = self.generate_section(&title, commits);
                changelog.push_str(&section_content);
            }
        }

        changelog.trim_end().to_string()
    }

    /// Organize commits by their type
    fn organize_commits_by_type<'a>(
        &self,
        commits: &'a [ConventionalCommit],
    ) -> HashMap<String, Vec<&'a ConventionalCommit>> {
        let mut sections = HashMap::new();

        for commit in commits {
            let entry = sections
                .entry(commit.commit_type.clone())
                .or_insert_with(Vec::new);
            entry.push(commit);
        }

        // Sort commits within each section by scope, then by description
        for commits in sections.values_mut() {
            commits.sort_by(|a, b| match (&a.scope, &b.scope) {
                (Some(a_scope), Some(b_scope)) => a_scope
                    .cmp(b_scope)
                    .then_with(|| a.description.cmp(&b.description)),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.description.cmp(&b.description),
            });
        }

        sections
    }

    /// Generate a section of the changelog
    fn generate_section(&self, title: &str, commits: &[&ConventionalCommit]) -> String {
        let mut entries = String::new();

        for commit in commits {
            let entry = self.format_commit_entry(commit);
            entries.push_str(&entry);
            entries.push('\n');
        }

        self.config
            .section_template
            .replace("{title}", title)
            .replace("{entries}", entries.trim_end())
            + "\n\n"
    }

    /// Format a single commit entry
    fn format_commit_entry(&self, commit: &ConventionalCommit) -> String {
        let mut description = commit.description.clone();

        // Add scope if present
        if let Some(scope) = &commit.scope {
            description = format!("**{}**: {}", scope, description);
        }

        // Add breaking change indicator
        if commit.breaking_change {
            description = format!("⚠️ BREAKING: {}", description);
        }

        let mut entry = self
            .config
            .commit_template
            .replace("{description}", &description);

        if self.config.include_shas {
            let short_sha = if commit.sha.len() > 7 {
                &commit.sha[..7]
            } else {
                &commit.sha
            };
            entry = entry.replace("{sha}", short_sha);
        } else {
            entry = entry.replace(" [{sha}]", "");
            entry = entry.replace("[{sha}]", "");
        }

        entry
    }

    /// Format commit type as a title
    fn format_commit_type_title(&self, commit_type: &str) -> String {
        match commit_type {
            "feat" => "Features".to_string(),
            "fix" => "Bug Fixes".to_string(),
            "perf" => "Performance Improvements".to_string(),
            "revert" => "Reverts".to_string(),
            "docs" => "Documentation".to_string(),
            "style" => "Styles".to_string(),
            "refactor" => "Code Refactoring".to_string(),
            "test" => "Tests".to_string(),
            "build" => "Build System".to_string(),
            "ci" => "Continuous Integration".to_string(),
            "chore" => "Chores".to_string(),
            _ => {
                // Capitalize first letter for unknown types
                let mut chars = commit_type.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            }
        }
    }
}

impl Default for ChangelogGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Enhanced configuration for changelog generation with git-cliff-core support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedChangelogConfig {
    /// Whether to use git-cliff-core for advanced features
    pub use_git_cliff: bool,
    /// Whether to include commit authors
    pub include_authors: bool,
    /// Whether to include commit SHAs
    pub include_shas: bool,
    /// Whether to include links to commits/PRs
    pub include_links: bool,
    /// Template for changelog sections
    pub section_template: String,
    /// Template for individual commit entries
    pub commit_template: String,
    /// Git repository path for git-cliff-core (optional)
    pub repository_path: Option<String>,
    /// Remote repository URL for link generation
    pub remote_url: Option<String>,
}

impl Default for EnhancedChangelogConfig {
    fn default() -> Self {
        Self {
            use_git_cliff: true,
            include_authors: true,
            include_shas: true,
            include_links: true,
            section_template: "### {title}\n\n{entries}\n".to_string(),
            commit_template: "- {description} [{sha}]".to_string(),
            repository_path: None,
            remote_url: None,
        }
    }
}

/// Enhanced changelog generator with git-cliff-core integration
pub struct EnhancedChangelogGenerator {
    config: EnhancedChangelogConfig,
}

impl EnhancedChangelogGenerator {
    /// Create a new enhanced changelog generator with default configuration
    pub fn new() -> Self {
        Self {
            config: EnhancedChangelogConfig::default(),
        }
    }

    /// Create a new enhanced changelog generator with custom configuration
    pub fn with_config(config: EnhancedChangelogConfig) -> Self {
        Self { config }
    }

    /// Generate a changelog using git-cliff-core if enabled, fallback to basic implementation
    pub fn generate_changelog(
        &self,
        commits: &[ConventionalCommit],
    ) -> crate::errors::CoreResult<String> {
        debug!(
            "Generating enhanced changelog from {} commits",
            commits.len()
        );

        if commits.is_empty() {
            return Ok("No changes in this release.".to_string());
        }

        if self.config.use_git_cliff {
            self.generate_with_git_cliff(commits)
        } else {
            // Fallback to basic implementation
            let basic_generator = ChangelogGenerator::with_config(ChangelogConfig {
                include_authors: self.config.include_authors,
                include_shas: self.config.include_shas,
                section_template: self.config.section_template.clone(),
                commit_template: self.config.commit_template.clone(),
            });
            Ok(basic_generator.generate_changelog(commits))
        }
    }

    /// Generate changelog using git-cliff-core
    fn generate_with_git_cliff(
        &self,
        commits: &[ConventionalCommit],
    ) -> crate::errors::CoreResult<String> {
        // Convert our commits to git-cliff format
        let git_cliff_commits: Vec<GitCliffCommit> = commits
            .iter()
            .map(|commit| self.convert_to_git_cliff_commit(commit))
            .collect::<Result<Vec<_>, _>>()?;

        // Create git-cliff configuration
        let git_cliff_config = self.create_git_cliff_config()?;

        // Create a release with our commits
        let mut release = GitCliffRelease::default();
        release.version = Some("Unreleased".to_string());
        release.commits = git_cliff_commits;
        release.timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        // Create changelog
        let mut changelog = GitCliffChangelog::new(vec![release], &git_cliff_config, None)
            .map_err(|e| crate::errors::CoreError::changelog_generation(e.to_string()))?;

        // Add remote context for link generation if configured
        if self.config.include_links {
            if let Err(e) = changelog.add_remote_context() {
                debug!("Failed to add remote context: {}", e);
                // Continue without remote context
            }
        }

        // Generate the changelog
        let mut output = Vec::new();
        changelog
            .generate(&mut output)
            .map_err(|e| crate::errors::CoreError::changelog_generation(e.to_string()))?;

        let changelog_string = String::from_utf8(output).map_err(|e| {
            crate::errors::CoreError::changelog_generation(format!("UTF-8 conversion error: {}", e))
        })?;

        Ok(changelog_string)
    }

    /// Convert our ConventionalCommit to git-cliff-core Commit
    fn convert_to_git_cliff_commit(
        &self,
        commit: &ConventionalCommit,
    ) -> crate::errors::CoreResult<GitCliffCommit> {
        let git_cliff_commit = GitCliffCommit::new(commit.sha.clone(), commit.message.clone());

        // Set additional fields if available
        // Note: git-cliff-core Commit might have additional fields we can populate
        // This is a basic conversion - we might need to adjust based on available fields

        Ok(git_cliff_commit)
    }

    /// Create git-cliff-core configuration
    fn create_git_cliff_config(&self) -> crate::errors::CoreResult<GitCliffConfig> {
        // Create a basic git-cliff configuration
        // This uses a simplified template that works with our conventional commits
        let config_toml = r#"
[changelog]
header = """
"""
body = """
{%- for group, commits in commits | group_by(attribute="group") %}
### {{ group | title }}
{%- for commit in commits %}
- {{ commit.message | split(pat=":") | last | trim }} [{{ commit.id | truncate(length=7, end="") }}]
{%- endfor %}

{%- endfor %}
"""
trim = true

[git]
conventional_commits = true
filter_unconventional = false
split_commits = false
commit_parsers = [
    { message = "^feat", group = "Features" },
    { message = "^fix", group = "Bug Fixes" },
    { message = "^perf", group = "Performance Improvements" },
    { message = "^revert", group = "Reverts" },
    { message = "^docs", group = "Documentation" },
    { message = "^style", group = "Styles" },
    { message = "^refactor", group = "Code Refactoring" },
    { message = "^test", group = "Tests" },
    { message = "^build", group = "Build System" },
    { message = "^ci", group = "Continuous Integration" },
    { message = "^chore", group = "Chores" },
]
"#;

        let config: GitCliffConfig = toml::from_str(&config_toml).map_err(|e| {
            crate::errors::CoreError::changelog_generation(format!("Config parsing error: {}", e))
        })?;

        Ok(config)
    }
}

impl Default for EnhancedChangelogGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "changelog_tests.rs"]
mod tests;
