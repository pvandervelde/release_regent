use super::*;
use serde_json::{from_str, to_string};

#[test]
fn test_label_deserialization() {
    // Create JSON
    let json_str = r#"{"name": "feature"}"#;

    // Deserialize from JSON
    let label: Label = from_str(json_str).expect("Failed to deserialize Label");

    // Verify fields
    assert_eq!(label.name, "feature");
}

#[test]
fn test_label_serialization() {
    // Create a label
    let label = Label {
        name: "bug".to_string(),
    };

    // Serialize to JSON
    let json_str = to_string(&label).expect("Failed to serialize Label");

    // Verify JSON structure
    let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("Failed to parse JSON");
    assert_eq!(parsed["name"], "bug");
}


#[test]
fn test_user_serialization() {
    // Create a user
    let user = User {
        id: 303,
        login: "developer".to_string(),
    };

    // Serialize to JSON
    let json_str = to_string(&user).expect("Failed to serialize User");

    // Verify JSON structure
    let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("Failed to parse JSON");
    assert_eq!(parsed["id"], 303);
    assert_eq!(parsed["login"], "developer");
}

#[test]
fn test_user_deserialization() {
    // Create JSON
    let json_str = r#"{
        "id": 404,
        "login": "contributor"
    }"#;

    // Deserialize from JSON
    let user: User = from_str(json_str).expect("Failed to deserialize User");

    // Verify fields
    assert_eq!(user.id, 404);
    assert_eq!(user.login, "contributor");
}
