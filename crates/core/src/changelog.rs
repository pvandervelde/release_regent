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

/// Strategy for changelog generation.
///
/// Controls how [`ChangelogGenerator`] produces formatted release notes:
/// - [`ChangelogStrategy::Internal`] — built-in template renderer (default).
/// - [`ChangelogStrategy::GitCliff`] — delegates to git-cliff-core for
///   advanced Tera-based templating.
/// - [`ChangelogStrategy::External`] — runs a subprocess and captures stdout.
///   Commits are passed as `{sha} {message}` lines on stdin.
///
/// Example TOML (external):
/// ```toml
/// [changelog.strategy.external]
/// command = "git-cliff"
/// env_vars = { GIT_CLIFF_CONFIG = "/path/to/cliff.toml" }
/// timeout_ms = 30000
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChangelogStrategy {
    /// Built-in template renderer.
    #[default]
    Internal,
    /// Delegate to git-cliff-core for advanced Tera-based templating.
    GitCliff,
    /// Run an external subprocess.
    ///
    /// Commits are passed as `{sha} {message}\n` lines on stdin.
    /// The command's stdout becomes the changelog body.
    External {
        /// Command to execute (space-separated; first token is the binary).
        command: String,
        /// Additional environment variables passed to the command.
        env_vars: HashMap<String, String>,
        /// Maximum wall-clock time in milliseconds before the process is
        /// terminated.  Defaults to 30 000 ms (30 seconds).
        ///
        /// Note: timeout enforcement uses a background thread; the function
        /// blocks the calling thread until the process exits or the deadline
        /// elapses.
        #[serde(default = "default_external_timeout_ms")]
        timeout_ms: u64,
    },
}

pub(crate) fn default_external_timeout_ms() -> u64 {
    30_000
}

fn default_true() -> bool {
    true
}

fn default_section_template() -> String {
    "### {title}\n\n{entries}\n".to_string()
}

fn default_commit_template() -> String {
    "- {description} [{sha}]".to_string()
}

/// Configuration for changelog generation.
///
/// The `strategy` field selects the rendering back-end; the remaining fields
/// control the built-in template renderer and apply only when
/// `strategy == ChangelogStrategy::Internal`.
///
/// Uses multiple boolean flags because each controls an independent, orthogonal
/// rendering option; converting them to enums would add complexity without benefit.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogConfig {
    /// Rendering strategy to use.
    #[serde(default)]
    pub strategy: ChangelogStrategy,
    /// Whether to include commit authors
    #[serde(default = "default_true")]
    pub include_authors: bool,
    /// Whether to include commit SHAs
    #[serde(default = "default_true")]
    pub include_shas: bool,
    /// Whether to include links to commits/PRs
    #[serde(default = "default_true")]
    pub include_links: bool,
    /// Template for changelog sections
    #[serde(default = "default_section_template")]
    pub section_template: String,
    /// Template for individual commit entries
    #[serde(default = "default_commit_template")]
    pub commit_template: String,
    /// Git repository path for git-cliff-core (optional)
    #[serde(default)]
    pub repository_path: Option<String>,
    /// Remote repository URL for link generation
    #[serde(default)]
    pub remote_url: Option<String>,
}

impl Default for ChangelogConfig {
    fn default() -> Self {
        Self {
            strategy: ChangelogStrategy::default(),
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

/// Changelog generator that creates formatted markdown from conventional commits.
///
/// The rendering back-end is selected by [`ChangelogConfig::strategy`]:
/// - [`ChangelogStrategy::Internal`] — built-in ordered template renderer.
/// - [`ChangelogStrategy::GitCliff`] — git-cliff-core Tera templating.
/// - [`ChangelogStrategy::External`] — subprocess (e.g. `git-cliff` CLI).
///
/// All paths return `CoreResult<String>` so callers handle errors uniformly.
pub struct ChangelogGenerator {
    config: ChangelogConfig,
}

impl ChangelogGenerator {
    /// Create a new changelog generator with default configuration
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: ChangelogConfig::default(),
        }
    }

    /// Create a new changelog generator with custom configuration
    #[must_use]
    pub fn with_config(config: ChangelogConfig) -> Self {
        Self { config }
    }

    /// Generate a changelog from conventional commits.
    ///
    /// The rendering back-end is selected by [`ChangelogConfig::strategy`]:
    /// - [`ChangelogStrategy::Internal`] \u2014 built-in template renderer.
    /// - [`ChangelogStrategy::GitCliff`] \u2014 git-cliff-core.
    /// - [`ChangelogStrategy::External`] \u2014 subprocess.
    ///
    /// # Errors
    ///
    /// Returns [`crate::errors::CoreError::ChangelogGeneration`] when the
    /// selected back-end fails (git-cliff processing error, subprocess failure, etc.).
    // CoreError is intentionally large; this is the established pattern throughout the codebase.
    #[allow(clippy::result_large_err)]
    pub fn generate_changelog(
        &self,
        commits: &[ConventionalCommit],
    ) -> crate::errors::CoreResult<String> {
        debug!("Generating changelog from {} commits", commits.len());

        if commits.is_empty() {
            return Ok("No changes in this release.".to_string());
        }

        match &self.config.strategy {
            ChangelogStrategy::Internal => Ok(self.generate_with_template(commits)),
            ChangelogStrategy::GitCliff => self.generate_with_git_cliff(commits),
            ChangelogStrategy::External {
                command,
                env_vars,
                timeout_ms,
            } => self.generate_with_external(command, env_vars, *timeout_ms, commits),
        }
    }

    /// Delegate changelog generation to an external subprocess.
    ///
    /// Commits are written to the child's stdin as `{sha} {message}\n` lines.
    /// The child's stdout is captured and returned as the changelog body.
    ///
    /// A background thread enforces `timeout_ms`: if the process has not exited
    /// within the deadline, the child is killed and an error is returned.
    ///
    /// # Errors
    ///
    /// Returns [`crate::errors::CoreError::ChangelogGeneration`] when:
    /// - the command string is empty or cannot be split,
    /// - the process cannot be spawned,
    /// - the process exits with a non-zero status,
    /// - the deadline elapses before the process exits, or
    /// - the stdout is not valid UTF-8.
    // CoreError is intentionally large; this is the established pattern throughout the codebase.
    #[allow(clippy::result_large_err)]
    fn generate_with_external(
        &self,
        command: &str,
        env_vars: &HashMap<String, String>,
        timeout_ms: u64,
        commits: &[ConventionalCommit],
    ) -> crate::errors::CoreResult<String> {
        use std::io::Write as _;
        use std::process::{Command, Stdio};

        let parts: Vec<&str> = command.split_whitespace().collect();
        let (prog, args) = parts.split_first().ok_or_else(|| {
            crate::errors::CoreError::changelog_generation(
                "changelog.strategy.external.command is empty".to_string(),
            )
        })?;

        // Build stdin: one line per commit.
        let stdin_content: String = commits
            .iter()
            .map(|c| format!("{} {}", c.sha, c.message))
            .collect::<Vec<_>>()
            .join("\n");

        let mut child = Command::new(prog)
            .args(args)
            .envs(env_vars)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                crate::errors::CoreError::changelog_generation(format!(
                    "Failed to start changelog command '{command}': {e}"
                ))
            })?;

        // Write commits to stdin then drop to signal EOF.
        // A BrokenPipe error means the process exited before reading all input,
        // which is valid (e.g. a tool that ignores stdin). We do not treat that
        // as fatal; we proceed to collect the process exit status and stdout.
        if let Some(mut stdin) = child.stdin.take() {
            if let Err(e) = stdin.write_all(stdin_content.as_bytes()) {
                if e.kind() != std::io::ErrorKind::BrokenPipe {
                    return Err(crate::errors::CoreError::changelog_generation(format!(
                        "Failed to write commits to changelog command stdin: {e}"
                    )));
                }
            }
        }

        // Enforce the timeout via a channel: `wait_with_output` runs on a
        // background thread and the result is sent back to the calling thread.
        // The calling thread blocks on `recv_timeout`; if the deadline elapses
        // before the process exits an error is returned.  The child process may
        // become an orphan in that case — consistent with how
        // `DefaultVersionCalculator::External` handles the same situation.
        let (tx, rx) = std::sync::mpsc::channel();
        let command_for_thread = command.to_string();
        std::thread::spawn(move || {
            let result = child.wait_with_output().map_err(|e| {
                crate::errors::CoreError::changelog_generation(format!(
                    "Changelog command '{command_for_thread}' failed while waiting: {e}"
                ))
            });
            // Ignore send error — receiver may have already timed out.
            let _ = tx.send(result);
        });

        let output = rx
            .recv_timeout(std::time::Duration::from_millis(timeout_ms))
            .map_err(|_| {
                crate::errors::CoreError::changelog_generation(format!(
                    "Changelog command '{command}' timed out after {timeout_ms} ms"
                ))
            })??;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::errors::CoreError::changelog_generation(format!(
                "Changelog command '{command}' exited with {}: {stderr}",
                output.status
            )));
        }

        let raw = String::from_utf8(output.stdout).map_err(|e| {
            crate::errors::CoreError::changelog_generation(format!(
                "Changelog command '{command}' produced non-UTF-8 output: {e}"
            ))
        })?;

        let trimmed = raw.trim().to_string();
        if trimmed.is_empty() {
            Ok("No changes in this release.".to_string())
        } else {
            Ok(trimmed)
        }
    }

    /// Generate changelog using git-cliff-core.
    // CoreError is intentionally large; this is the established pattern throughout the codebase.
    #[allow(clippy::result_large_err)]
    fn generate_with_git_cliff(
        &self,
        commits: &[ConventionalCommit],
    ) -> crate::errors::CoreResult<String> {
        let git_cliff_commits: Vec<GitCliffCommit> = commits
            .iter()
            .map(|commit| Self::convert_to_git_cliff_commit(commit))
            .collect();

        let git_cliff_config = Self::create_git_cliff_config()?;

        let release = GitCliffRelease {
            version: Some("Unreleased".to_string()),
            commits: git_cliff_commits,
            timestamp: Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
                    .unwrap_or(0),
            ),
            ..GitCliffRelease::default()
        };

        let mut changelog = GitCliffChangelog::new(vec![release], git_cliff_config, None)
            .map_err(|e| crate::errors::CoreError::changelog_generation(e.to_string()))?;

        if self.config.include_links {
            if let Err(e) = changelog.add_remote_context() {
                debug!(error = %e, "Failed to add remote context");
                // Continue without remote context
            }
        }

        let mut output = Vec::new();
        changelog
            .generate(&mut output)
            .map_err(|e| crate::errors::CoreError::changelog_generation(e.to_string()))?;

        let changelog_string = String::from_utf8(output).map_err(|e| {
            crate::errors::CoreError::changelog_generation(format!("UTF-8 conversion error: {e}"))
        })?;

        let trimmed = changelog_string.trim().to_string();
        if trimmed.is_empty() {
            // All commits were filtered (e.g. only merge commits with filter_unconventional=true).
            // Return the same sentinel the public API uses for an empty commit list so callers
            // get a meaningful message rather than a blank PR body.
            Ok("No changes in this release.".to_string())
        } else {
            Ok(trimmed)
        }
    }

    /// Generate changelog using the built-in template renderer.
    fn generate_with_template(&self, commits: &[ConventionalCommit]) -> String {
        let sections = Self::organize_commits_by_type(commits);
        let mut changelog = String::new();

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
                let title = Self::format_commit_type_title(commit_type);
                let section_content = self.generate_section(&title, commits);
                changelog.push_str(&section_content);
            }
        }

        changelog.trim_end().to_string()
    }

    /// Convert a [`ConventionalCommit`] to a git-cliff-core `Commit`.
    fn convert_to_git_cliff_commit(commit: &ConventionalCommit) -> GitCliffCommit<'_> {
        GitCliffCommit::new(commit.sha.clone(), commit.message.clone())
    }

    /// Create git-cliff-core configuration.
    ///
    /// # Errors
    ///
    /// Returns [`crate::errors::CoreError::ChangelogGeneration`] when the TOML template
    /// cannot be parsed by git-cliff-core.
    // CoreError is intentionally large; this is the established pattern throughout the codebase.
    #[allow(clippy::result_large_err)]
    fn create_git_cliff_config() -> crate::errors::CoreResult<GitCliffConfig> {
        let config_toml = r#"
[changelog]
body = """
{%- for group, commits in commits | group_by(attribute="group") %}
### {{ group | title }}
{%- for commit in commits %}
- {{ commit.message | split(pat=":") | last | trim }} [{{ commit.id }}]
{%- endfor %}

{%- endfor %}
"""
trim = true
render_always = true
postprocessors = []

[git]
conventional_commits = true
filter_unconventional = true
split_commits = false
require_conventional = false
commit_preprocessors = []
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
link_parsers = []
protect_breaking_commits = false
filter_commits = false
fail_on_unmatched_commit = false
topo_order = false
topo_order_commits = false
sort_commits = "newest"
use_branch_tags = false
include_paths = []
exclude_paths = []
"#;

        let config: GitCliffConfig = toml::from_str(config_toml).map_err(|e| {
            crate::errors::CoreError::changelog_generation(format!("Config parsing error: {e}"))
        })?;

        Ok(config)
    }

    /// Organize commits by their type
    fn organize_commits_by_type(
        commits: &[ConventionalCommit],
    ) -> HashMap<String, Vec<&ConventionalCommit>> {
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
            description = format!("**{scope}**: {description}");
        }

        // Add breaking change indicator
        if commit.breaking_change {
            description = format!("⚠️ BREAKING: {description}");
        }

        let mut entry = self
            .config
            .commit_template
            .replace("{description}", &description);

        if self.config.include_shas {
            entry = entry.replace("{sha}", &commit.sha);
        } else {
            entry = entry.replace(" [{sha}]", "");
            entry = entry.replace("[{sha}]", "");
        }

        entry
    }

    /// Format commit type as a title
    fn format_commit_type_title(commit_type: &str) -> String {
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

#[cfg(test)]
#[path = "changelog_tests.rs"]
mod tests;
