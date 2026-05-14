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

    let changelog = generator
        .generate_changelog(&commits)
        .expect("changelog generation failed");

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

    let changelog = generator
        .generate_changelog(&commits)
        .expect("changelog generation failed");

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

    let changelog = generator
        .generate_changelog(&commits)
        .expect("changelog generation failed");

    assert!(changelog.contains("⚠️ BREAKING: remove deprecated API"));
    assert!(changelog.contains("⚠️ BREAKING: **auth**: change login flow"));
}

#[test]
fn test_changelog_generation_empty_commits() {
    let generator = ChangelogGenerator::new();
    let commits = vec![];

    let changelog = generator
        .generate_changelog(&commits)
        .expect("changelog generation failed");

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

    let changelog = generator
        .generate_changelog(&commits)
        .expect("changelog generation failed");

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
        strategy: ChangelogStrategy::Internal,
        include_authors: false,
        include_shas: false,
        include_links: false,
        section_template: "## {title}\n\n{entries}\n".to_string(),
        commit_template: "* {description}".to_string(),
        repository_path: None,
        remote_url: None,
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

    let changelog = generator
        .generate_changelog(&commits)
        .expect("changelog generation failed");

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

    let changelog = generator
        .generate_changelog(&commits)
        .expect("changelog generation failed");

    // Should be sorted: scoped items first (auth, ui), then unscoped items
    let auth_pos = changelog.find("**auth**: add login").unwrap();
    let ui_pos = changelog.find("**ui**: add button").unwrap();
    let core_pos = changelog.find("add core feature").unwrap();

    assert!(auth_pos < ui_pos);
    assert!(ui_pos < core_pos);
}

#[test]
fn test_changelog_generation_template_path_basic() {
    // Previously "test_enhanced_changelog_generation_basic": uses template renderer
    let config = ChangelogConfig {
        strategy: ChangelogStrategy::Internal,
        ..Default::default()
    };
    let generator = ChangelogGenerator::with_config(config);
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
fn test_changelog_config_defaults() {
    let config = ChangelogConfig::default();
    assert!(config.strategy == ChangelogStrategy::Internal); // template renderer is the default
    assert!(config.include_authors);
    assert!(config.include_shas);
    assert!(config.include_links);
    assert_eq!(config.section_template, "### {title}\n\n{entries}\n");
    assert_eq!(config.commit_template, "- {description} [{sha}]");
}

#[test]
fn test_changelog_generator_creation() {
    let generator = ChangelogGenerator::new();
    assert!(generator.config.strategy == ChangelogStrategy::Internal); // default is template renderer

    let custom_config = ChangelogConfig {
        strategy: ChangelogStrategy::GitCliff,
        include_authors: false,
        ..Default::default()
    };
    let custom_generator = ChangelogGenerator::with_config(custom_config);
    assert!(custom_generator.config.strategy == ChangelogStrategy::GitCliff);
    assert!(!custom_generator.config.include_authors);
}

#[test]
fn test_changelog_empty_commits() {
    let generator = ChangelogGenerator::new();
    let result = generator.generate_changelog(&[]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "No changes in this release.");
}

#[test]
fn test_changelog_with_git_cliff_enabled() {
    let config = ChangelogConfig {
        strategy: ChangelogStrategy::GitCliff,
        include_authors: true,
        include_shas: true,
        include_links: false, // Disable links to avoid remote dependency in tests
        ..Default::default()
    };
    let generator = ChangelogGenerator::with_config(config);
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
    assert!(!changelog.is_empty());
    assert!(!changelog.contains("No changes in this release."));
}

#[test]
fn test_changelog_error_handling() {
    let generator = ChangelogGenerator::new();
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

// ─────────────────────────────────────────────────────────────────────────────
// git-cliff path coverage
// ─────────────────────────────────────────────────────────────────────────────

/// Verify that `generate_with_git_cliff` returns a non-empty UTF-8 string for
/// a typical set of conventional commits when `include_links = false` (no
/// remote-context call, which would require network access in tests).
#[test]
fn test_generate_with_git_cliff_happy_path() {
    let config = ChangelogConfig {
        strategy: ChangelogStrategy::GitCliff,
        include_links: false,
        ..Default::default()
    };
    let generator = ChangelogGenerator::with_config(config);
    let commits = vec![
        ConventionalCommit {
            commit_type: "feat".to_string(),
            scope: None,
            description: "add new capability".to_string(),
            breaking_change: false,
            message: "feat: add new capability".to_string(),
            sha: "abc123456789abcd".to_string(),
        },
        ConventionalCommit {
            commit_type: "fix".to_string(),
            scope: Some("core".to_string()),
            description: "resolve off-by-one error".to_string(),
            breaking_change: false,
            message: "fix(core): resolve off-by-one error".to_string(),
            sha: "def456789012abcd".to_string(),
        },
    ];

    let result = generator.generate_changelog(&commits);
    assert!(
        result.is_ok(),
        "git-cliff path should not error: {result:?}"
    );
    let text = result.unwrap();
    assert!(
        !text.is_empty(),
        "git-cliff should produce non-empty output for non-empty commit list"
    );
}

/// Verify that `generate_with_git_cliff` returns `Ok` for a single commit,
/// exercising the boundary where `commits.len() == 1`.
#[test]
fn test_generate_with_git_cliff_single_commit() {
    let config = ChangelogConfig {
        strategy: ChangelogStrategy::GitCliff,
        include_links: false,
        ..Default::default()
    };
    let generator = ChangelogGenerator::with_config(config);
    let commits = vec![ConventionalCommit {
        commit_type: "chore".to_string(),
        scope: None,
        description: "update Cargo.lock".to_string(),
        breaking_change: false,
        message: "chore: update Cargo.lock".to_string(),
        sha: "aabbccddeeff0011".to_string(),
    }];

    let result = generator.generate_changelog(&commits);
    assert!(
        result.is_ok(),
        "git-cliff path should handle a single commit: {result:?}"
    );
}

/// Verify that `generate_with_git_cliff` returns `Ok` when `include_links = true`
/// but the `remote_url` is `None`.  The code calls `add_remote_context()` only
/// when `include_links` is set; when that call fails it logs and continues rather
/// than propagating the error, so the overall result should still be `Ok`.
#[test]
fn test_generate_with_git_cliff_links_without_remote_url() {
    let config = ChangelogConfig {
        strategy: ChangelogStrategy::GitCliff,
        include_links: true,
        remote_url: None,
        ..Default::default()
    };
    let generator = ChangelogGenerator::with_config(config);
    let commits = vec![ConventionalCommit {
        commit_type: "feat".to_string(),
        scope: None,
        description: "add widget".to_string(),
        breaking_change: false,
        message: "feat: add widget".to_string(),
        sha: "1122334455667788".to_string(),
    }];

    // add_remote_context failure is swallowed; result should still be Ok.
    let result = generator.generate_changelog(&commits);
    assert!(
        result.is_ok(),
        "git-cliff path should not propagate remote-context errors: {result:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// ChangelogStrategy serialization / round-trip tests
// ─────────────────────────────────────────────────────────────────────────────

/// `ChangelogStrategy::Internal` round-trips through TOML unchanged.
#[test]
fn test_changelog_strategy_internal_roundtrip() {
    let config = ChangelogConfig {
        strategy: ChangelogStrategy::Internal,
        ..Default::default()
    };
    let toml = toml::to_string_pretty(&config).expect("should serialize");
    let back: ChangelogConfig = toml::from_str(&toml).expect("should deserialize");
    assert_eq!(back.strategy, ChangelogStrategy::Internal);
}

/// `ChangelogStrategy::GitCliff` round-trips through TOML unchanged.
#[test]
fn test_changelog_strategy_git_cliff_roundtrip() {
    let config = ChangelogConfig {
        strategy: ChangelogStrategy::GitCliff,
        ..Default::default()
    };
    let toml = toml::to_string_pretty(&config).expect("should serialize");
    let back: ChangelogConfig = toml::from_str(&toml).expect("should deserialize");
    assert_eq!(back.strategy, ChangelogStrategy::GitCliff);
}

/// `ChangelogStrategy::External` round-trips through TOML with command, env_vars, and timeout.
#[test]
fn test_changelog_strategy_external_roundtrip() {
    let mut env_vars = std::collections::HashMap::new();
    env_vars.insert("FOO".to_string(), "bar".to_string());
    let config = ChangelogConfig {
        strategy: ChangelogStrategy::External {
            command: "git-cliff --config cliff.toml".to_string(),
            env_vars,
            timeout_ms: 60_000,
        },
        ..Default::default()
    };
    let toml = toml::to_string_pretty(&config).expect("should serialize");
    let back: ChangelogConfig = toml::from_str(&toml).expect("should deserialize");
    if let ChangelogStrategy::External {
        command,
        env_vars,
        timeout_ms,
    } = back.strategy
    {
        assert_eq!(command, "git-cliff --config cliff.toml");
        assert_eq!(env_vars.get("FOO").map(String::as_str), Some("bar"));
        assert_eq!(timeout_ms, 60_000);
    } else {
        panic!("expected External strategy after round-trip");
    }
}

/// `ChangelogStrategy::External` uses the default timeout when `timeout_ms` is omitted.
#[test]
fn test_changelog_strategy_external_default_timeout() {
    let toml_input = r#"
[strategy.external]
command = "my-tool"
env_vars = {}
"#;
    let config: ChangelogConfig =
        toml::from_str(toml_input).expect("should parse external config without timeout_ms");
    if let ChangelogStrategy::External { timeout_ms, .. } = config.strategy {
        assert_eq!(timeout_ms, 30_000, "default timeout should be 30 000 ms");
    } else {
        panic!("expected External strategy");
    }
}

/// An empty command returns a `ChangelogGeneration` error rather than panicking.
#[test]
fn test_generate_with_external_empty_command_returns_error() {
    let config = ChangelogConfig {
        strategy: ChangelogStrategy::External {
            command: "".to_string(),
            env_vars: std::collections::HashMap::new(),
            timeout_ms: 5_000,
        },
        ..Default::default()
    };
    let generator = ChangelogGenerator::with_config(config);
    let commits = vec![ConventionalCommit {
        commit_type: "feat".to_string(),
        scope: None,
        description: "add thing".to_string(),
        breaking_change: false,
        message: "feat: add thing".to_string(),
        sha: "abc123".to_string(),
    }];

    let result = generator.generate_changelog(&commits);
    assert!(result.is_err(), "empty command should return an error");
    let err_msg = result.unwrap_err().to_string().to_lowercase();
    assert!(
        err_msg.contains("empty") || err_msg.contains("command"),
        "error should mention empty command; got: {err_msg}"
    );
}
