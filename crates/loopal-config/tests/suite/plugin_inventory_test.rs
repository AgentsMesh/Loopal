use std::fs;

use loopal_config::list_plugins_from_user_dir;

#[test]
fn plugin_inventory_projects_capabilities_without_contents() {
    let root = tempfile::tempdir().unwrap();
    let plugin = root.path().join("plugins/desktop-pack");
    fs::create_dir_all(plugin.join("skills")).unwrap();
    fs::create_dir_all(plugin.join("memory")).unwrap();
    fs::write(
        plugin.join("settings.json"),
        r#"{
          "mcp_servers":{"alpha":{"type":"stdio","command":"alpha-server"}},
          "hooks":[
            {"event":"pre_tool_use","command":"first"},
            {"event":"post_input","command":"second"}
          ],
          "secret":"hidden"
        }"#,
    )
    .unwrap();
    let oversized_name = "x".repeat(129);
    fs::write(
        plugin.join(".mcp.json"),
        format!(
            r#"{{"mcpServers":{{
              "beta":{{"command":"beta-server"}},
              "alpha":{{"command":"alpha-override"}},
              "":{{"command":"empty-server"}},
              "{oversized_name}":{{"command":"oversized-server"}}
            }}}}"#
        ),
    )
    .unwrap();
    fs::write(plugin.join("skills/check.md"), "Check body").unwrap();
    fs::write(plugin.join("LOOPAL.md"), "private instructions").unwrap();
    fs::write(plugin.join("memory/MEMORY.md"), "private memory").unwrap();

    let plugins = list_plugins_from_user_dir(root.path()).unwrap();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].name, "desktop-pack");
    assert_eq!(plugins[0].skills, ["/check"]);
    assert_eq!(plugins[0].mcp_servers, ["alpha", "beta"]);
    assert_eq!(plugins[0].hook_count, 2);
    assert!(plugins[0].has_settings);
    assert!(plugins[0].has_instructions);
    assert!(plugins[0].has_memory);
    assert!(!format!("{:?}", plugins[0]).contains("hidden"));
}

#[cfg(unix)]
#[test]
fn plugin_inventory_matches_loader_symlink_directory_semantics() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("target");
    fs::create_dir_all(target.join("skills")).unwrap();
    fs::write(target.join("skills/linked.md"), "Linked").unwrap();
    fs::create_dir(root.path().join("plugins")).unwrap();
    symlink(&target, root.path().join("plugins/linked-pack")).unwrap();

    let plugins = list_plugins_from_user_dir(root.path()).unwrap();
    assert_eq!(plugins[0].name, "linked-pack");
    assert_eq!(plugins[0].skills, ["/linked"]);
}
