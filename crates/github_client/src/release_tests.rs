use super::*;

#[test]
fn test_create_release_options() {
    let options = CreateReleaseOptions {
        tag_name: "v1.0.0".to_string(),
        name: "Release v1.0.0".to_string(),
        body: "Initial release".to_string(),
        target_commitish: Some("main".to_string()),
        draft: false,
        prerelease: false,
    };

    assert_eq!(options.tag_name, "v1.0.0");
    assert_eq!(options.name, "Release v1.0.0");
    assert_eq!(options.body, "Initial release");
    assert_eq!(options.target_commitish, Some("main".to_string()));
    assert!(!options.draft);
    assert!(!options.prerelease);
}

#[test]
fn test_prerelease_options() {
    let options = CreateReleaseOptions {
        tag_name: "v2.0.0-beta.1".to_string(),
        name: "v2.0.0 Beta 1".to_string(),
        body: "Beta release".to_string(),
        target_commitish: None,
        draft: false,
        prerelease: true,
    };

    assert!(options.prerelease);
    assert!(!options.draft);
    assert!(options.target_commitish.is_none());
}

#[test]
fn test_release_struct() {
    let release = Release {
        id: 123,
        tag_name: "v1.0.0".to_string(),
        name: "Release v1.0.0".to_string(),
        body: "Release notes".to_string(),
        draft: true,
        prerelease: false,
    };

    assert_eq!(release.id, 123);
    assert_eq!(release.tag_name, "v1.0.0");
    assert_eq!(release.name, "Release v1.0.0");
    assert_eq!(release.body, "Release notes");
    assert!(release.draft);
    assert!(!release.prerelease);
}
