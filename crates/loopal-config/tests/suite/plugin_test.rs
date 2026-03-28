use std::fs;

use loopal_config::layer::LayerSource;
use loopal_config::loader::load_layer_from_dir;
use loopal_config::plugin::load_plugin_layers;
use loopal_config::settings::McpServerConfig;

#[test]
fn test_load_layer_from_dir_empty() {
    let dir = tempfile::tempdir().unwrap();
    let layer = load_layer_from_dir(dir.path(), LayerSource::Global, None).unwrap();
    assert!(layer.settings.is_null());
    assert!(layer.mcp_servers.is_empty());
    assert!(layer.skills.is_empty());
    assert!(layer.hooks.is_empty());
    assert!(layer.instructions.is_none());
    assert!(layer.memory.is_none());
}

#[test]
fn test_load_layer_from_dir_full() {
    let dir = tempfile::tempdir().unwrap();

    // settings.json with mcp + hooks
    fs::write(
        dir.path().join("settings.json"),
        r#"{
            "model": "gpt-4",
            "mcp_servers": {
                "test": {"type": "stdio", "command": "test-server"}
            },
            "hooks": [
                {"event": "pre_tool_use", "command": "echo hook"}
            ]
        }"#,
    )
    .unwrap();

    // skills/
    let skills_dir = dir.path().join("skills");
    fs::create_dir_all(&skills_dir).unwrap();
    fs::write(skills_dir.join("commit.md"), "Commit skill.").unwrap();

    // LOOPAL.md
    fs::write(dir.path().join("LOOPAL.md"), "# Instructions").unwrap();

    let layer = load_layer_from_dir(dir.path(), LayerSource::Project, None).unwrap();

    // mcp_servers extracted
    assert_eq!(layer.mcp_servers.len(), 1);
    let McpServerConfig::Stdio { command, .. } = &layer.mcp_servers["test"] else {
        panic!("expected Stdio config");
    };
    assert_eq!(command, "test-server");

    // hooks extracted
    assert_eq!(layer.hooks.len(), 1);
    assert_eq!(layer.hooks[0].command, "echo hook");

    // settings without mcp/hooks
    assert!(layer.settings.get("mcp_servers").is_none());
    assert!(layer.settings.get("hooks").is_none());
    assert_eq!(layer.settings["model"], "gpt-4");

    // skills
    assert_eq!(layer.skills.len(), 1);
    assert_eq!(layer.skills[0].name, "/commit");

    // instructions
    assert_eq!(layer.instructions.as_deref(), Some("# Instructions"));

    // memory — not present
    assert!(layer.memory.is_none());
}

#[test]
fn test_load_layer_with_memory() {
    let dir = tempfile::tempdir().unwrap();
    let memory_dir = dir.path().join("memory");
    fs::create_dir_all(&memory_dir).unwrap();
    fs::write(memory_dir.join("MEMORY.md"), "## Key Patterns\n- Use DI").unwrap();

    let layer = load_layer_from_dir(dir.path(), LayerSource::Project, None).unwrap();
    assert_eq!(layer.memory.as_deref(), Some("## Key Patterns\n- Use DI"));
}

#[test]
fn test_load_layer_custom_instructions_path() {
    let dir = tempfile::tempdir().unwrap();
    let instr_path = dir.path().join("CUSTOM.md");
    fs::write(&instr_path, "Custom instructions").unwrap();

    let layer = load_layer_from_dir(dir.path(), LayerSource::Global, Some(&instr_path)).unwrap();

    assert_eq!(layer.instructions.as_deref(), Some("Custom instructions"));
}

#[test]
fn test_load_plugin_layers_empty_dir() {
    // This test verifies that an empty/missing plugins dir is handled gracefully.
    // The actual plugins dir is ~/.loopal/plugins/, which may or may not exist.
    // We're testing the code path — the function should not panic.
    let _layers = load_plugin_layers();
}

#[test]
fn test_load_plugin_layers_with_plugins() {
    let dir = tempfile::tempdir().unwrap();
    let plugins_dir = dir.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();

    // Create two plugin directories
    let plugin_a = plugins_dir.join("alpha-plugin");
    fs::create_dir_all(plugin_a.join("skills")).unwrap();
    fs::write(
        plugin_a.join("settings.json"),
        r#"{"mcp_servers": {"alpha-mcp": {"type": "stdio", "command": "alpha-server"}}}"#,
    )
    .unwrap();
    fs::write(plugin_a.join("LOOPAL.md"), "Alpha instructions").unwrap();
    fs::write(plugin_a.join("skills").join("alpha.md"), "Alpha skill.").unwrap();

    let plugin_b = plugins_dir.join("beta-plugin");
    fs::create_dir_all(&plugin_b).unwrap();
    fs::write(plugin_b.join("LOOPAL.md"), "Beta instructions").unwrap();

    // We can't easily test load_plugin_layers since it reads from ~/.loopal/plugins,
    // but we can test the isomorphic loader on plugin directories directly.
    let layer_a =
        load_layer_from_dir(&plugin_a, LayerSource::Plugin("alpha-plugin".into()), None).unwrap();
    assert_eq!(layer_a.mcp_servers.len(), 1);
    assert_eq!(layer_a.skills.len(), 1);
    assert_eq!(layer_a.instructions.as_deref(), Some("Alpha instructions"));

    let layer_b =
        load_layer_from_dir(&plugin_b, LayerSource::Plugin("beta-plugin".into()), None).unwrap();
    assert!(layer_b.mcp_servers.is_empty());
    assert!(layer_b.skills.is_empty());
    assert_eq!(layer_b.instructions.as_deref(), Some("Beta instructions"));
}
