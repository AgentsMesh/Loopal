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
allowed_tools: [Read, Glob, Grep]
---
You explore code.
"#,
    )
    .unwrap();

    let configs = load_agent_configs(dir.path());
    assert_eq!(configs.len(), 1);

    let config = configs.get("explorer").unwrap();
    assert_eq!(config.description, "Code explorer");
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
    assert!(config.allowed_tools.is_none());
    assert!(config.model.is_none());
}

#[test]
fn no_frontmatter_keeps_full_body_as_system_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let agents_dir = dir.path().join(".loopal").join("agents");
    fs::create_dir_all(&agents_dir).unwrap();

    fs::write(agents_dir.join("simple.md"), "Just a prompt.").unwrap();

    let configs = load_agent_configs(dir.path());
    let config = configs.get("simple").unwrap();
    assert_eq!(config.system_prompt, "Just a prompt.");
    assert!(config.allowed_tools.is_none());
}

#[test]
fn frontmatter_parses_model_override() {
    let dir = tempfile::tempdir().unwrap();
    let agents_dir = dir.path().join(".loopal").join("agents");
    fs::create_dir_all(&agents_dir).unwrap();

    fs::write(
        agents_dir.join("fast.md"),
        r#"---
description: Fast model agent
model: claude-haiku-4-5
---
Quick responses.
"#,
    )
    .unwrap();

    let configs = load_agent_configs(dir.path());
    let config = configs.get("fast").unwrap();
    assert_eq!(config.model.as_deref(), Some("claude-haiku-4-5"));
    assert!(config.system_prompt.contains("Quick responses"));
}

#[test]
fn malformed_frontmatter_falls_back_to_full_body() {
    let dir = tempfile::tempdir().unwrap();
    let agents_dir = dir.path().join(".loopal").join("agents");
    fs::create_dir_all(&agents_dir).unwrap();

    // Opens with --- but no closing delimiter
    fs::write(
        agents_dir.join("broken.md"),
        "---\ndescription: oops\nNo closing fence here",
    )
    .unwrap();

    let configs = load_agent_configs(dir.path());
    let config = configs.get("broken").unwrap();
    // Whole content (including the ---) becomes system_prompt
    assert!(config.system_prompt.contains("No closing fence"));
    assert_eq!(config.description, "General-purpose agent"); // default
}
