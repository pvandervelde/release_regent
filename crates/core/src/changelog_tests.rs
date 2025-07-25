use super::*;
use crate::versioning::ConventionalCommit;

#[test]
fn test_changelog_generation_basic() {
    let generator = ChangelogGenerator::new();
    let commits = vec![
        ConventionalCommit {
            commit_type: "feat".to_string(),
            scope: None,
            description: "add user authentication".to_string(),
            breaking_change: false,
            message: "feat: add user authentication".to_string(),
            sha: "abc123456789".to_string(),
        },
        ConventionalCommit {
            commit_type: "fix".to_string(),
            scope: None,
            description: "resolve login bug".to_string(),
            breaking_change: false,
            message: "fix: resolve login bug".to_string(),
            sha: "def456789012".to_string(),
        },
    ];

    let changelog = generator.generate_changelog(&commits);

    assert!(changelog.contains("### Features"));
    assert!(changelog.contains("add user authentication"));
    assert!(changelog.contains("abc1234"));
    assert!(changelog.contains("### Bug Fixes"));
    assert!(changelog.contains("resolve login bug"));
    assert!(changelog.contains("def4567"));
}

#[test]
fn test_changelog_generation_with_scope() {
    let generator = ChangelogGenerator::new();
    let commits = vec![
        ConventionalCommit {
            commit_type: "feat".to_string(),
            scope: Some("auth".to_string()),
            description: "add OAuth support".to_string(),
            breaking_change: false,
            message: "feat(auth): add OAuth support".to_string(),
            sha: "abc123456789".to_string(),
        },
        ConventionalCommit {
            commit_type: "fix".to_string(),
            scope: Some("ui".to_string()),
            description: "button alignment".to_string(),
            breaking_change: false,
            message: "fix(ui): button alignment".to_string(),
            sha: "def456789012".to_string(),
        },
    ];

    let changelog = generator.generate_changelog(&commits);

    assert!(changelog.contains("**auth**: add OAuth support"));
    assert!(changelog.contains("**ui**: button alignment"));
}

#[test]
fn test_changelog_generation_breaking_changes() {
    let generator = ChangelogGenerator::new();
    let commits = vec![
        ConventionalCommit {
            commit_type: "feat".to_string(),
            scope: None,
            description: "remove deprecated API".to_string(),
            breaking_change: true,
            message: "feat!: remove deprecated API".to_string(),
            sha: "abc123456789".to_string(),
        },
        ConventionalCommit {
            commit_type: "fix".to_string(),
            scope: Some("auth".to_string()),
            description: "change login flow".to_string(),
            breaking_change: true,
            message: "fix(auth): change login flow\n\nBREAKING CHANGE: Login flow changed"
                .to_string(),
            sha: "def456789012".to_string(),
        },
    ];

    let changelog = generator.generate_changelog(&commits);

    assert!(changelog.contains("⚠️ BREAKING: remove deprecated API"));
    assert!(changelog.contains("⚠️ BREAKING: **auth**: change login flow"));
}

#[test]
fn test_changelog_generation_empty_commits() {
    let generator = ChangelogGenerator::new();
    let commits = vec![];

    let changelog = generator.generate_changelog(&commits);

    assert_eq!(changelog, "No changes in this release.");
}

#[test]
fn test_changelog_generation_section_ordering() {
    let generator = ChangelogGenerator::new();
    let commits = vec![
        ConventionalCommit {
            commit_type: "chore".to_string(),
            scope: None,
            description: "update dependencies".to_string(),
            breaking_change: false,
            message: "chore: update dependencies".to_string(),
            sha: "abc123456789".to_string(),
        },
        ConventionalCommit {
            commit_type: "feat".to_string(),
            scope: None,
            description: "add new feature".to_string(),
            breaking_change: false,
            message: "feat: add new feature".to_string(),
            sha: "def456789012".to_string(),
        },
        ConventionalCommit {
            commit_type: "fix".to_string(),
            scope: None,
            description: "fix bug".to_string(),
            breaking_change: false,
            message: "fix: fix bug".to_string(),
            sha: "ghi789012345".to_string(),
        },
    ];

    let changelog = generator.generate_changelog(&commits);

    // Features should come before Bug Fixes, which should come before Chores
    let feat_pos = changelog.find("### Features").unwrap();
    let fix_pos = changelog.find("### Bug Fixes").unwrap();
    let chore_pos = changelog.find("### Chores").unwrap();

    assert!(feat_pos < fix_pos);
    assert!(fix_pos < chore_pos);
}

#[test]
fn test_changelog_generation_custom_config() {
    let config = ChangelogConfig {
        include_authors: false,
        include_shas: false,
        section_template: "## {title}\n\n{entries}\n".to_string(),
        commit_template: "* {description}".to_string(),
    };

    let generator = ChangelogGenerator::with_config(config);
    let commits = vec![ConventionalCommit {
        commit_type: "feat".to_string(),
        scope: None,
        description: "add feature".to_string(),
        breaking_change: false,
        message: "feat: add feature".to_string(),
        sha: "abc123456789".to_string(),
    }];

    let changelog = generator.generate_changelog(&commits);

    assert!(changelog.contains("## Features"));
    assert!(changelog.contains("* add feature"));
    assert!(!changelog.contains("abc1234"));
}

#[test]
fn test_changelog_generation_scope_sorting() {
    let generator = ChangelogGenerator::new();
    let commits = vec![
        ConventionalCommit {
            commit_type: "feat".to_string(),
            scope: Some("ui".to_string()),
            description: "add button".to_string(),
            breaking_change: false,
            message: "feat(ui): add button".to_string(),
            sha: "abc123456789".to_string(),
        },
        ConventionalCommit {
            commit_type: "feat".to_string(),
            scope: Some("auth".to_string()),
            description: "add login".to_string(),
            breaking_change: false,
            message: "feat(auth): add login".to_string(),
            sha: "def456789012".to_string(),
        },
        ConventionalCommit {
            commit_type: "feat".to_string(),
            scope: None,
            description: "add core feature".to_string(),
            breaking_change: false,
            message: "feat: add core feature".to_string(),
            sha: "ghi789012345".to_string(),
        },
    ];

    let changelog = generator.generate_changelog(&commits);

    // Should be sorted: scoped items first (auth, ui), then unscoped items
    let auth_pos = changelog.find("**auth**: add login").unwrap();
    let ui_pos = changelog.find("**ui**: add button").unwrap();
    let core_pos = changelog.find("add core feature").unwrap();

    assert!(auth_pos < ui_pos);
    assert!(ui_pos < core_pos);
}

#[test]
fn test_enhanced_changelog_generation_basic() {
    let config = EnhancedChangelogConfig {
        use_git_cliff: false, // Use fallback for this test
        ..Default::default()
    };
    let generator = EnhancedChangelogGenerator::with_config(config);
    let commits = vec![
        ConventionalCommit {
            commit_type: "feat".to_string(),
            scope: None,
            description: "add user authentication".to_string(),
            breaking_change: false,
            message: "feat: add user authentication".to_string(),
            sha: "abc123456789".to_string(),
        },
        ConventionalCommit {
            commit_type: "fix".to_string(),
            scope: None,
            description: "resolve login bug".to_string(),
            breaking_change: false,
            message: "fix: resolve login bug".to_string(),
            sha: "def456789012".to_string(),
        },
    ];

    let result = generator.generate_changelog(&commits);
    assert!(result.is_ok());

    let changelog = result.unwrap();
    assert!(changelog.contains("### Features"));
    assert!(changelog.contains("add user authentication"));
    assert!(changelog.contains("abc1234"));
    assert!(changelog.contains("### Bug Fixes"));
    assert!(changelog.contains("resolve login bug"));
    assert!(changelog.contains("def4567"));
}

#[test]
fn test_enhanced_changelog_config_defaults() {
    let config = EnhancedChangelogConfig::default();
    assert!(config.use_git_cliff);
    assert!(config.include_authors);
    assert!(config.include_shas);
    assert!(config.include_links);
    assert_eq!(config.section_template, "### {title}\n\n{entries}\n");
    assert_eq!(config.commit_template, "- {description} [{sha}]");
}

#[test]
fn test_enhanced_changelog_generator_creation() {
    let generator = EnhancedChangelogGenerator::new();
    assert!(generator.config.use_git_cliff);

    let custom_config = EnhancedChangelogConfig {
        use_git_cliff: false,
        include_authors: false,
        ..Default::default()
    };
    let custom_generator = EnhancedChangelogGenerator::with_config(custom_config);
    assert!(!custom_generator.config.use_git_cliff);
    assert!(!custom_generator.config.include_authors);
}

#[test]
fn test_enhanced_changelog_empty_commits() {
    let generator = EnhancedChangelogGenerator::new();
    let result = generator.generate_changelog(&[]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "No changes in this release.");
}

#[test]
#[ignore] // Temporarily disabled while git-cliff configuration is being finalized
fn test_enhanced_changelog_with_git_cliff_enabled() {
    let config = EnhancedChangelogConfig {
        use_git_cliff: true,
        include_authors: true,
        include_shas: true,
        include_links: false, // Disable links to avoid remote dependency issues in tests
        ..Default::default()
    };
    let generator = EnhancedChangelogGenerator::with_config(config);
    let commits = vec![
        ConventionalCommit {
            commit_type: "feat".to_string(),
            scope: Some("auth".to_string()),
            description: "add OAuth support".to_string(),
            breaking_change: false,
            message: "feat(auth): add OAuth support".to_string(),
            sha: "abc123456789".to_string(),
        },
        ConventionalCommit {
            commit_type: "fix".to_string(),
            scope: Some("ui".to_string()),
            description: "button alignment".to_string(),
            breaking_change: false,
            message: "fix(ui): button alignment".to_string(),
            sha: "def456789012".to_string(),
        },
    ];

    let result = generator.generate_changelog(&commits);
    assert!(result.is_ok());

    let changelog = result.unwrap();
    // git-cliff should generate structured output
    assert!(changelog.len() > 0);
    assert!(!changelog.contains("No changes in this release."));
}

#[test]
fn test_enhanced_changelog_error_handling() {
    let generator = EnhancedChangelogGenerator::new();
    let commits = vec![ConventionalCommit {
        commit_type: "feat".to_string(),
        scope: None,
        description: "test feature".to_string(),
        breaking_change: false,
        message: "feat: test feature".to_string(),
        sha: "".to_string(), // Empty SHA to potentially trigger errors
    }];

    let result = generator.generate_changelog(&commits);
    // Should handle gracefully - either succeed or return meaningful error
    match result {
        Ok(_) => {
            // git-cliff handled it gracefully
        }
        Err(e) => {
            // Should be a ChangelogGeneration error
            assert!(
                e.to_string().contains("changelog generation")
                    || e.to_string().contains("Changelog generation")
            );
        }
    }
}
