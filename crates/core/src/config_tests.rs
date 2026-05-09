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
}

#[test]
fn test_notification_strategy_serialization() {
    let strategies = vec![
        NotificationStrategy::GitHubIssue,
        NotificationStrategy::Webhook,
        NotificationStrategy::Slack,
        NotificationStrategy::None,
    ];

    for strategy in strategies {
        let serialized = serde_yaml::to_string(&strategy).unwrap();
        let deserialized: NotificationStrategy = serde_yaml::from_str(&serialized).unwrap();

        match (strategy, deserialized) {
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
    let conventional = VersioningStrategy::Conventional;
    let serialized = serde_yaml::to_string(&conventional).unwrap();
    let deserialized: VersioningStrategy = serde_yaml::from_str(&serialized).unwrap();
    assert!(matches!(deserialized, VersioningStrategy::Conventional));

    let external = VersioningStrategy::External {
        command: "/usr/bin/version".to_string(),
        env_vars: std::collections::HashMap::new(),
        timeout_ms: 30_000,
    };
    let serialized = serde_yaml::to_string(&external).unwrap();
    let deserialized: VersioningStrategy = serde_yaml::from_str(&serialized).unwrap();
    assert!(matches!(deserialized, VersioningStrategy::External { .. }));
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
    let yaml = r#"
notifications:
  strategy: "github_issue"
  github_issue:
    labels:
      - "my-label"
"#;

    let config: ReleaseRegentConfig =
        serde_yaml::from_str(yaml).expect("should deserialize with only labels");
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
    let yaml = r#"
notifications:
  strategy: "github_issue"
  github_issue:
    assignees:
      - "alice"
"#;

    let config: ReleaseRegentConfig =
        serde_yaml::from_str(yaml).expect("should deserialize with only assignees");
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
    let yaml = r#"
notifications:
  strategy: "webhook"
  webhook:
    url: "https://hooks.example.com/release-regent"
"#;

    let config: ReleaseRegentConfig =
        serde_yaml::from_str(yaml).expect("should deserialize without headers");
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
    let yaml = r#"
notifications:
  strategy: "webhook"
  webhook:
    url: "https://hooks.example.com/release-regent"
"#;

    let config: ReleaseRegentConfig = serde_yaml::from_str(yaml).unwrap();
    let serialized = serde_yaml::to_string(&config).unwrap();
    // `headers` is omitted when empty (skip_serializing_if)
    assert!(!serialized.contains("headers"));
    // Re-loading must not fail even though `headers` is absent in the YAML
    let reloaded: ReleaseRegentConfig = serde_yaml::from_str(&serialized).unwrap();
    assert!(reloaded.notifications.webhook.unwrap().headers.is_empty());
}
