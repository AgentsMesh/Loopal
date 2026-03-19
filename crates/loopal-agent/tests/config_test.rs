use loopal_agent::config::{AgentConfig, load_agent_configs};
use std::fs;

#[test]
fn test_load_agent_configs_from_dir() {
    let dir = tempfile::tempdir().unwrap();
    let agents_dir = dir.path().join(".loopal").join("agents");
    fs::create_dir_all(&agents_dir).unwrap();

    fs::write(
        agents_dir.join("explorer.md"),
        r#"---
description: Code explorer
permission_mode: accept-edits
allowed_tools: [Read, Glob, Grep]
max_turns: 15
---
You explore code.
"#,
    )
    .unwrap();

    let configs = load_agent_configs(dir.path());
    assert_eq!(configs.len(), 1);

    let config = configs.get("explorer").unwrap();
    assert_eq!(config.description, "Code explorer");
    assert_eq!(config.max_turns, 15);
    assert_eq!(
        config.allowed_tools.as_ref().unwrap(),
        &["Read", "Glob", "Grep"]
    );
    assert!(config.system_prompt.contains("explore code"));
}

#[test]
fn test_load_empty_dir_returns_empty() {
    let dir = tempfile::tempdir().unwrap();
    let configs = load_agent_configs(dir.path());
    assert!(configs.is_empty());
}

#[test]
fn test_default_agent_config() {
    let config = AgentConfig::default();
    assert_eq!(config.max_turns, 30);
    assert!(config.allowed_tools.is_none());
    assert!(config.model.is_none());
}
