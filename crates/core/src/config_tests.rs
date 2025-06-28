use super::*;

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
fn test_configuration_validation_success() {
    let config = ReleaseRegentConfig::default();
    assert!(config.validate().is_ok());
}

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
fn test_configuration_validation_external_versioning_missing() {
    let mut config = ReleaseRegentConfig::default();
    config.versioning.strategy = VersioningStrategy::External;
    config.versioning.external = None;

    let result = config.validate();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("External versioning configuration required"));
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
    let strategies = vec![
        VersioningStrategy::Conventional,
        VersioningStrategy::External,
    ];

    for strategy in strategies {
        let serialized = serde_yaml::to_string(&strategy).unwrap();
        let deserialized: VersioningStrategy = serde_yaml::from_str(&serialized).unwrap();

        match (strategy, deserialized) {
            (VersioningStrategy::Conventional, VersioningStrategy::Conventional) => {}
            (VersioningStrategy::External, VersioningStrategy::External) => {}
            _ => panic!("Serialization/deserialization failed"),
        }
    }
}
