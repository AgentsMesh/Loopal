use std::fs;

use loopal_agent::config::load_agent_configs;

#[test]
fn skips_non_markdown_and_unreadable_markdown_entries() {
    let root = tempfile::tempdir().unwrap();
    let agents = root.path().join(".loopal/agents");
    fs::create_dir_all(&agents).unwrap();
    fs::write(agents.join("branch-valid.md"), "valid prompt").unwrap();
    fs::write(agents.join("branch-ignored.txt"), "ignored").unwrap();
    fs::create_dir(agents.join("branch-unreadable.md")).unwrap();

    let configs = load_agent_configs(root.path());

    assert_eq!(configs["branch-valid"].system_prompt, "valid prompt");
    assert!(!configs.contains_key("branch-ignored"));
    assert!(!configs.contains_key("branch-unreadable"));
}

#[test]
fn opening_fence_without_newline_and_empty_model_fall_back_cleanly() {
    let root = tempfile::tempdir().unwrap();
    let agents = root.path().join(".loopal/agents");
    fs::create_dir_all(&agents).unwrap();
    fs::write(agents.join("branch-fence.md"), "---").unwrap();
    fs::write(
        agents.join("branch-empty-model.md"),
        "---\nmodel:   \nunknown_key: ignored\n---\nbody",
    )
    .unwrap();

    let configs = load_agent_configs(root.path());

    assert_eq!(configs["branch-fence"].system_prompt, "---");
    assert_eq!(configs["branch-empty-model"].system_prompt, "body");
    assert!(configs["branch-empty-model"].model.is_none());
}
