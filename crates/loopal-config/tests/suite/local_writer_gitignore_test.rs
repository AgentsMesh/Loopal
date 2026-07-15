use loopal_config::{project_local_settings_path, update_local_settings_field};

#[test]
fn local_settings_writer_preserves_custom_rules_and_finishes_with_an_ignore_rule() {
    let root = tempfile::tempdir().unwrap();
    let loopal = root.path().join(".loopal");
    std::fs::create_dir_all(&loopal).unwrap();
    std::fs::write(
        loopal.join(".gitignore"),
        "custom-cache/\n!/settings.local.json\n",
    )
    .unwrap();

    update_local_settings_field(root.path(), "model", serde_json::json!("first")).unwrap();
    update_local_settings_field(root.path(), "model", serde_json::json!("second")).unwrap();

    let ignore = std::fs::read_to_string(loopal.join(".gitignore")).unwrap();
    assert!(ignore.starts_with("custom-cache/\n!/settings.local.json\n"));
    assert!(ignore.ends_with("# Loopal local-only settings\n/settings.local.json\n"));
    assert_eq!(ignore.matches("# Loopal local-only settings").count(), 1);
    assert!(project_local_settings_path(root.path()).is_file());
}
