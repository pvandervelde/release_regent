use super::*;

#[test]
fn test_configuration_validation_empty_main_branch() {
    let mut config = ReleaseRegentConfig::default();
    config.core.branches.main = "".to_string();

    let result = config.validate();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Main branch name cannot be empty"));
}

#[test]
fn test_configuration_validation_external_versioning_success() {
    let mut config = ReleaseRegentConfig::default();
    config.versioning.strategy = VersioningStrategy::External {
        command: "/usr/local/bin/version-tool".to_string(),
        env_vars: std::collections::HashMap::new(),
        timeout_ms: 30_000,
    };

    let result = config.validate();
    assert!(result.is_ok());
}

#[test]
fn test_configuration_validation_slack_missing() {
    let mut config = ReleaseRegentConfig::default();
    config.notifications.strategy = NotificationStrategy::Slack;
    config.notifications.slack = None;

    let result = config.validate();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Slack configuration required"));
}

#[test]
fn test_configuration_validation_success() {
    let config = ReleaseRegentConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn test_configuration_validation_version_prefix_whitespace() {
    let mut config = ReleaseRegentConfig::default();
    config.core.version_prefix = "v ".to_string();

    let result = config.validate();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Version prefix cannot contain whitespace"));
}

#[test]
fn test_configuration_validation_webhook_missing() {
    let mut config = ReleaseRegentConfig::default();
    config.notifications.strategy = NotificationStrategy::Webhook;
    config.notifications.webhook = None;

    let result = config.validate();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Webhook configuration required"));
}

#[test]
fn test_default_configuration() {
    let config = ReleaseRegentConfig::default();

    assert_eq!(config.core.version_prefix, "v");
    assert_eq!(config.core.branches.main, "main");
    assert!(!config.release_pr.draft);
    assert!(!config.releases.draft);
    assert!(config.releases.generate_notes);
    assert_eq!(config.error_handling.max_retries, 5);
    assert!(config.notifications.enabled);
    assert!(matches!(
        config.notifications.strategy,
        NotificationStrategy::GitHubIssue
    ));
    assert!(matches!(
        config.versioning.strategy,
        VersioningStrategy::Conventional
    ));
    assert!(config.versioning.allow_override);
    // Changelog section defaults to the built-in template renderer.
    assert!(matches!(
        config.changelog.strategy,
        crate::changelog::ChangelogStrategy::Internal
    ));
}

/// A `[changelog]` block in a TOML config is no longer silently ignored —
/// it is parsed into `ReleaseRegentConfig::changelog`.
#[test]
fn test_changelog_section_is_parsed_from_toml() {
    let toml_input = r#"
[changelog]
include_shas = false
include_links = false

[changelog.strategy.external]
command = "git-cliff"
env_vars = {}
timeout_ms = 45000
"#;
    let config: ReleaseRegentConfig = toml::from_str(toml_input).expect("should parse");
    assert!(!config.changelog.include_shas);
    assert!(!config.changelog.include_links);
    if let crate::changelog::ChangelogStrategy::External {
        command,
        timeout_ms,
        ..
    } = config.changelog.strategy
    {
        assert_eq!(command, "git-cliff");
        assert_eq!(timeout_ms, 45_000);
    } else {
        panic!(
            "expected External changelog strategy, got {:?}",
            config.changelog.strategy
        );
    }
}

/// A TOML config with no `[changelog]` block deserialises with the default
/// changelog config (strategy = Internal, booleans = true).
#[test]
fn test_changelog_section_defaults_when_absent() {
    let toml_input = r#"
[core]
version_prefix = "v"
"#;
    let config: ReleaseRegentConfig = toml::from_str(toml_input).expect("should parse");
    assert!(matches!(
        config.changelog.strategy,
        crate::changelog::ChangelogStrategy::Internal
    ));
    assert!(config.changelog.include_shas);
    assert!(config.changelog.include_links);
}

#[test]
fn test_notification_strategy_serialization() {
    #[derive(serde::Serialize, serde::Deserialize, Debug)]
    struct Wrapper {
        strategy: NotificationStrategy,
    }

    let strategies = vec![
        NotificationStrategy::GitHubIssue,
        NotificationStrategy::Webhook,
        NotificationStrategy::Slack,
        NotificationStrategy::None,
    ];

    for strategy in strategies {
        let wrapper = Wrapper {
            strategy: strategy.clone(),
        };
        let serialized = toml::to_string_pretty(&wrapper).unwrap();
        let deserialized: Wrapper = toml::from_str::<Wrapper>(&serialized).unwrap();

        match (strategy, deserialized.strategy) {
            (NotificationStrategy::GitHubIssue, NotificationStrategy::GitHubIssue) => {}
            (NotificationStrategy::Webhook, NotificationStrategy::Webhook) => {}
            (NotificationStrategy::Slack, NotificationStrategy::Slack) => {}
            (NotificationStrategy::None, NotificationStrategy::None) => {}
            _ => panic!("Serialization/deserialization failed"),
        }
    }
}

#[test]
fn test_versioning_strategy_serialization() {
    #[derive(serde::Serialize, serde::Deserialize, Debug)]
    struct Wrapper {
        strategy: VersioningStrategy,
    }

    let conventional = VersioningStrategy::Conventional;
    let serialized = toml::to_string_pretty(&Wrapper {
        strategy: conventional,
    })
    .unwrap();
    let deserialized = toml::from_str::<Wrapper>(&serialized).unwrap();
    assert!(matches!(
        deserialized.strategy,
        VersioningStrategy::Conventional
    ));

    let external = VersioningStrategy::External {
        command: "/usr/bin/version".to_string(),
        env_vars: std::collections::HashMap::new(),
        timeout_ms: 30_000,
    };
    let serialized = toml::to_string_pretty(&Wrapper { strategy: external }).unwrap();
    let deserialized = toml::from_str::<Wrapper>(&serialized).unwrap();
    assert!(matches!(
        deserialized.strategy,
        VersioningStrategy::External { .. }
    ));
}

#[test]
fn test_from_versioning_strategy_conventional() {
    use crate::traits::version_calculator::VersioningStrategy as TraitVersioningStrategy;

    let config_strategy = VersioningStrategy::Conventional;
    let trait_strategy = TraitVersioningStrategy::from(config_strategy);

    assert!(matches!(
        trait_strategy,
        TraitVersioningStrategy::ConventionalCommits {
            include_prerelease: false,
            ..
        }
    ));
    if let TraitVersioningStrategy::ConventionalCommits { custom_types, .. } = trait_strategy {
        assert!(custom_types.is_empty());
    } else {
        panic!("expected ConventionalCommits variant");
    }
}

#[test]
fn test_from_versioning_strategy_external() {
    use crate::traits::version_calculator::VersioningStrategy as TraitVersioningStrategy;

    let mut env_vars = std::collections::HashMap::new();
    env_vars.insert("MY_VAR".to_string(), "value".to_string());

    let config_strategy = VersioningStrategy::External {
        command: "/usr/bin/versioner".to_string(),
        env_vars: env_vars.clone(),
        timeout_ms: 30_000,
    };
    let trait_strategy = TraitVersioningStrategy::from(config_strategy);

    if let TraitVersioningStrategy::External {
        command,
        env_vars: result_env_vars,
        timeout_ms,
    } = trait_strategy
    {
        assert_eq!(command, "/usr/bin/versioner");
        assert_eq!(result_env_vars, env_vars);
        assert_eq!(timeout_ms, 30_000);
    } else {
        panic!("expected External variant");
    }
}

#[test]
fn test_github_issue_config_partial_labels_only() {
    // When a user provides `github_issue:` with only `labels`, `assignees` must
    // default to `[]` rather than causing a deserialization error.
    let input = r#"
[notifications]
strategy = "github_issue"

[notifications.github_issue]
labels = ["my-label"]
"#;

    let config: ReleaseRegentConfig =
        toml::from_str(input).expect("should deserialize with only labels");
    let gh = config
        .notifications
        .github_issue
        .expect("github_issue should be present");
    assert_eq!(gh.labels, vec!["my-label"]);
    assert!(gh.assignees.is_empty());
}

#[test]
fn test_github_issue_config_partial_assignees_only() {
    // When a user provides `github_issue:` with only `assignees`, `labels` must
    // default to `["release-regent", "bug"]` rather than causing a deserialization error.
    let input = r#"
[notifications]
strategy = "github_issue"

[notifications.github_issue]
assignees = ["alice"]
"#;

    let config: ReleaseRegentConfig =
        toml::from_str(input).expect("should deserialize with only assignees");
    let gh = config
        .notifications
        .github_issue
        .expect("github_issue should be present");
    assert_eq!(gh.labels, vec!["release-regent", "bug"]);
    assert_eq!(gh.assignees, vec!["alice"]);
}

#[test]
fn test_webhook_config_without_headers() {
    // When a user provides `webhook:` with only `url`, `headers` must default to
    // an empty map rather than causing a deserialization error.
    let input = r#"
[notifications]
strategy = "webhook"

[notifications.webhook]
url = "https://hooks.example.com/release-regent"
"#;

    let config: ReleaseRegentConfig =
        toml::from_str(input).expect("should deserialize without headers");
    let webhook = config
        .notifications
        .webhook
        .expect("webhook should be present");
    assert_eq!(webhook.url, "https://hooks.example.com/release-regent");
    assert!(webhook.headers.is_empty());
}

#[test]
fn test_webhook_config_headers_round_trip() {
    // headers must survive a serialize → deserialize round-trip.
    // skip_serializing_if suppresses empty maps on serialisation, but adding
    // #[serde(default)] ensures an absent key deserialises to an empty map.
    let input = r#"
[notifications]
strategy = "webhook"

[notifications.webhook]
url = "https://hooks.example.com/release-regent"
"#;

    let config: ReleaseRegentConfig = toml::from_str(input).unwrap();
    let serialized = toml::to_string_pretty(&config).unwrap();
    // `headers` is omitted when empty (skip_serializing_if)
    assert!(!serialized.contains("headers"));
    // Re-loading must not fail even though `headers` is absent in the YAML
    let reloaded: ReleaseRegentConfig = toml::from_str::<ReleaseRegentConfig>(&serialized).unwrap();
    assert!(reloaded.notifications.webhook.unwrap().headers.is_empty());
}

// ============================================================================
// ReleaseRegentConfig::group and locked_fields — ADR-007 extension
// ============================================================================

#[test]
fn test_release_regent_config_group_none_by_default() {
    assert!(ReleaseRegentConfig::default().group.is_none());
}

#[test]
fn test_release_regent_config_locked_fields_empty_by_default() {
    assert!(ReleaseRegentConfig::default().locked_fields.is_empty());
}

#[test]
fn test_release_regent_config_group_round_trip() {
    let input = r#"
group = "platform"
"#;
    let config: ReleaseRegentConfig =
        toml::from_str(input).expect("should deserialize group field");
    assert_eq!(config.group.as_deref(), Some("platform"));

    let serialized = toml::to_string_pretty(&config).expect("should serialize");
    let reloaded: ReleaseRegentConfig =
        toml::from_str::<ReleaseRegentConfig>(&serialized).expect("should re-deserialize");
    assert_eq!(reloaded.group.as_deref(), Some("platform"));
}

#[test]
fn test_release_regent_config_group_absent_deserializes_to_none() {
    // A TOML document without the `group` key must silently default to None.
    let input = r#"
[core]
version_prefix = "v"
"#;
    let config: ReleaseRegentConfig =
        toml::from_str(input).expect("should deserialize without group key");
    assert!(config.group.is_none());
}

#[test]
fn test_release_regent_config_locked_fields_round_trip() {
    let input = r#"
locked_fields = ["versioning.strategy", "releases.draft"]
"#;
    let config: ReleaseRegentConfig =
        toml::from_str(input).expect("should deserialize locked_fields");
    assert_eq!(
        config.locked_fields,
        vec!["versioning.strategy", "releases.draft"]
    );

    let serialized = toml::to_string_pretty(&config).expect("should serialize");
    let reloaded: ReleaseRegentConfig =
        toml::from_str::<ReleaseRegentConfig>(&serialized).expect("should re-deserialize");
    assert_eq!(
        reloaded.locked_fields,
        vec!["versioning.strategy", "releases.draft"]
    );
}

#[test]
fn test_release_regent_config_locked_fields_absent_deserializes_to_empty() {
    // A TOML document without `locked_fields` must silently default to [].
    let input = r#"
[core]
version_prefix = "v"
"#;
    let config: ReleaseRegentConfig =
        toml::from_str(input).expect("should deserialize without locked_fields key");
    assert!(config.locked_fields.is_empty());
}

#[test]
fn test_release_regent_config_group_toml_round_trip() {
    let toml_str = r#"
group = "backend"
"#;
    let config: ReleaseRegentConfig =
        toml::from_str(toml_str).expect("should deserialize group from TOML");
    assert_eq!(config.group.as_deref(), Some("backend"));

    let serialized = toml::to_string(&config).expect("should serialize to TOML");
    let reloaded: ReleaseRegentConfig = toml::from_str::<ReleaseRegentConfig>(&serialized)
        .expect("should re-deserialize from TOML");
    assert_eq!(reloaded.group.as_deref(), Some("backend"));
}

#[test]
fn test_release_regent_config_locked_fields_toml_round_trip() {
    let toml_str = r#"
locked_fields = ["versioning.strategy", "core.version_prefix"]
"#;
    let config: ReleaseRegentConfig =
        toml::from_str(toml_str).expect("should deserialize locked_fields from TOML");
    assert_eq!(
        config.locked_fields,
        vec!["versioning.strategy", "core.version_prefix"]
    );

    let serialized = toml::to_string(&config).expect("should serialize to TOML");
    let reloaded: ReleaseRegentConfig = toml::from_str::<ReleaseRegentConfig>(&serialized)
        .expect("should re-deserialize from TOML");
    assert_eq!(
        reloaded.locked_fields,
        vec!["versioning.strategy", "core.version_prefix"]
    );
}

// ============================================================================
// LoadOptions::default() — ADR-007 extension fields
// ============================================================================

#[test]
fn test_load_options_default_has_none_installation_id() {
    use crate::traits::configuration_provider::LoadOptions;
    assert!(LoadOptions::default().installation_id.is_none());
}

#[test]
fn test_load_options_default_has_none_default_branch() {
    use crate::traits::configuration_provider::LoadOptions;
    assert!(LoadOptions::default().default_branch.is_none());
}

#[test]
fn test_load_options_installation_id_can_be_set() {
    use crate::traits::configuration_provider::LoadOptions;
    let opts = LoadOptions {
        installation_id: Some(12_345_u64),
        ..Default::default()
    };
    assert_eq!(opts.installation_id, Some(12_345));
    // Other fields must still have their default values.
    assert!(opts.default_branch.is_none());
    assert!(!opts.apply_env_overrides);
    assert!(!opts.cache);
    assert!(!opts.validate);
}

#[test]
fn test_load_options_default_branch_can_be_set() {
    use crate::traits::configuration_provider::LoadOptions;
    let opts = LoadOptions {
        default_branch: Some("develop".to_string()),
        ..Default::default()
    };
    assert_eq!(opts.default_branch.as_deref(), Some("develop"));
    assert!(opts.installation_id.is_none());
}

/// The sample config file in samples/config/release-regent.toml must parse
/// without error.  This catches any drift between the sample and the real
/// config schema.
#[test]
fn test_sample_config_parses_without_error() {
    let sample = include_str!("../../../samples/config/release-regent.toml");
    let config: ReleaseRegentConfig =
        toml::from_str(sample).expect("samples/config/release-regent.toml should parse");

    // Spot-check a few fields documented in the sample.
    assert_eq!(config.core.branches.main, "main");
    assert!(matches!(
        config.versioning.strategy,
        VersioningStrategy::Conventional
    ));
    assert!(config.versioning.allow_override);
    assert!(!config.release_pr.draft);
    assert!(matches!(
        config.changelog.strategy,
        crate::changelog::ChangelogStrategy::Internal
    ));
    assert!(config.changelog.include_shas);
}

/// `validate()` must reject an `External` changelog strategy with an empty command.
#[test]
fn test_validate_rejects_external_changelog_strategy_with_empty_command() {
    let mut config = ReleaseRegentConfig::default();
    config.changelog.strategy = crate::changelog::ChangelogStrategy::External {
        command: "   ".to_string(),
        env_vars: Default::default(),
        timeout_ms: 5_000,
    };
    let result = config.validate();
    assert!(
        result.is_err(),
        "expected validation error for empty command"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("changelog.strategy.external.command"),
        "error should mention the field, got: {msg}"
    );
}

/// `validate()` must accept a well-formed `External` changelog strategy.
#[test]
fn test_validate_accepts_external_changelog_strategy_with_non_empty_command() {
    let mut config = ReleaseRegentConfig::default();
    config.changelog.strategy = crate::changelog::ChangelogStrategy::External {
        command: "/usr/local/bin/gen-changelog".to_string(),
        env_vars: Default::default(),
        timeout_ms: 30_000,
    };
    assert!(
        config.validate().is_ok(),
        "non-empty external command should pass validation"
    );
}
