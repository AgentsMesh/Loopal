use tempfile::TempDir;

use loopal_config::{
    LocalSettingsFieldPatch, patch_local_settings_fields, project_local_settings_path,
    update_local_settings_field, update_local_settings_fields,
};

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

#[test]
fn updates_multiple_fields_in_one_document_commit() {
    let tmp = TempDir::new().unwrap();
    update_local_settings_field(
        tmp.path(),
        "providers",
        serde_json::json!({"anthropic": {"api_key": "keep-secret"}}),
    )
    .unwrap();
    update_local_settings_fields(
        tmp.path(),
        [
            ("model".into(), serde_json::json!("gpt-5.5")),
            ("sandbox".into(), serde_json::json!({"policy": "read_only"})),
            ("memory".into(), serde_json::json!({"enabled": false})),
        ],
    )
    .unwrap();

    let v = read(&tmp);
    assert_eq!(v["model"], "gpt-5.5");
    assert_eq!(v["sandbox"]["policy"], "read_only");
    assert_eq!(v["memory"]["enabled"], false);
    assert_eq!(v["providers"]["anthropic"]["api_key"], "keep-secret");
}

#[test]
fn dotted_fields_preserve_advanced_nested_settings() {
    let tmp = TempDir::new().unwrap();
    update_local_settings_field(
        tmp.path(),
        "sandbox",
        serde_json::json!({"policy": "default_write", "network": {"denied_domains": ["x"]}}),
    )
    .unwrap();
    update_local_settings_fields(
        tmp.path(),
        [("sandbox.policy".into(), serde_json::json!("read_only"))],
    )
    .unwrap();

    let v = read(&tmp);
    assert_eq!(v["sandbox"]["policy"], "read_only");
    assert_eq!(v["sandbox"]["network"]["denied_domains"][0], "x");
}

#[test]
fn atomic_patch_can_clear_secrets_and_remove_local_overrides() {
    let tmp = TempDir::new().unwrap();
    update_local_settings_field(
        tmp.path(),
        "providers",
        serde_json::json!({
            "anthropic": {"api_key": "secret", "unknown": true},
            "openai": null
        }),
    )
    .unwrap();
    patch_local_settings_fields(
        tmp.path(),
        [
            LocalSettingsFieldPatch::Set(
                "providers.anthropic.api_key".into(),
                serde_json::Value::Null,
            ),
            LocalSettingsFieldPatch::Set(
                "providers.openai.api_key_env".into(),
                serde_json::json!("OPENAI_KEY"),
            ),
            LocalSettingsFieldPatch::Remove("providers.anthropic.unknown".into()),
            LocalSettingsFieldPatch::EnsureObject("providers.google".into()),
        ],
    )
    .unwrap();

    let v = read(&tmp);
    assert!(v["providers"]["anthropic"]["api_key"].is_null());
    assert!(v["providers"]["anthropic"].get("unknown").is_none());
    assert_eq!(v["providers"]["openai"]["api_key_env"], "OPENAI_KEY");
    assert_eq!(v["providers"]["google"], serde_json::json!({}));
}
