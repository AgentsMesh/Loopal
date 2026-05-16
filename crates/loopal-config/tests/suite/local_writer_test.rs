use tempfile::TempDir;

use loopal_config::{project_local_settings_path, update_local_settings_field};

fn read(tmp: &TempDir) -> serde_json::Value {
    let path = project_local_settings_path(tmp.path());
    let text = std::fs::read_to_string(&path).expect("settings.local.json exists");
    serde_json::from_str(&text).expect("valid json")
}

#[test]
fn creates_file_and_directory_when_missing() {
    let tmp = TempDir::new().unwrap();
    update_local_settings_field(
        tmp.path(),
        "thinking",
        serde_json::json!({"type": "effort", "level": "high"}),
    )
    .expect("write should succeed");

    let v = read(&tmp);
    assert_eq!(
        v,
        serde_json::json!({"thinking": {"type": "effort", "level": "high"}})
    );
}

#[test]
fn preserves_existing_fields() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join(".loopal");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("settings.local.json"),
        r#"{"model": "claude-opus-4-7", "permission_mode": "ask_dangerous"}"#,
    )
    .unwrap();

    update_local_settings_field(
        tmp.path(),
        "thinking",
        serde_json::json!({"type": "disabled"}),
    )
    .unwrap();

    let v = read(&tmp);
    assert_eq!(v["model"], "claude-opus-4-7");
    assert_eq!(v["permission_mode"], "ask_dangerous");
    assert_eq!(v["thinking"], serde_json::json!({"type": "disabled"}));
}

#[test]
fn overwrites_existing_key() {
    let tmp = TempDir::new().unwrap();
    update_local_settings_field(tmp.path(), "thinking", serde_json::json!({"type": "auto"}))
        .unwrap();
    update_local_settings_field(
        tmp.path(),
        "thinking",
        serde_json::json!({"type": "effort", "level": "low"}),
    )
    .unwrap();

    let v = read(&tmp);
    assert_eq!(
        v["thinking"],
        serde_json::json!({"type": "effort", "level": "low"})
    );
    assert_eq!(v.as_object().unwrap().len(), 1, "no duplicate key entries");
}

#[test]
fn rejects_non_object_root() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join(".loopal");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("settings.local.json"), "[1, 2, 3]").unwrap();

    let err = update_local_settings_field(tmp.path(), "thinking", serde_json::json!({}))
        .expect_err("should fail on non-object root");
    let msg = format!("{err}");
    assert!(
        msg.contains("not an object"),
        "error should mention object: {msg}"
    );
}

#[test]
fn writes_model_as_string() {
    let tmp = TempDir::new().unwrap();
    update_local_settings_field(
        tmp.path(),
        "model",
        serde_json::Value::String("gpt-5.5".into()),
    )
    .unwrap();

    let v = read(&tmp);
    assert_eq!(v["model"], "gpt-5.5");
}
