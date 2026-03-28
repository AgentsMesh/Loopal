use std::collections::HashMap;

use loopal_config::hook::{HookConfig, HookEvent};
use loopal_config::layer::{ConfigLayer, LayerSource};
use loopal_config::resolver::ConfigResolver;
use loopal_config::settings::McpServerConfig;
use loopal_config::skills::parse_skill;

fn mcp_config(command: &str) -> McpServerConfig {
    McpServerConfig::Stdio {
        command: command.to_string(),
        args: Vec::new(),
        env: HashMap::new(),
        enabled: true,
        timeout_ms: 30_000,
    }
}

#[test]
fn test_resolve_empty_produces_defaults() {
    let resolver = ConfigResolver::new();
    let config = resolver.resolve().unwrap();
    assert_eq!(config.settings.model, "claude-sonnet-4-20250514");
    assert!(config.mcp_servers.is_empty());
    assert!(config.skills.is_empty());
    assert!(config.hooks.is_empty());
    assert!(config.instructions.is_empty());
}

#[test]
fn test_resolve_settings_deep_merge() {
    let mut resolver = ConfigResolver::new();

    let mut layer1 = ConfigLayer {
        source: LayerSource::Global,
        ..Default::default()
    };
    layer1.settings = serde_json::json!({"model": "gpt-4", "max_turns": 100});

    let mut layer2 = ConfigLayer {
        source: LayerSource::Project,
        ..Default::default()
    };
    layer2.settings = serde_json::json!({"max_turns": 200});

    resolver.add_layer(layer1);
    resolver.add_layer(layer2);

    let config = resolver.resolve().unwrap();
    assert_eq!(config.settings.model, "gpt-4");
    assert_eq!(config.settings.max_turns, 200);
}

#[test]
fn test_resolve_mcp_override_by_name() {
    let mut resolver = ConfigResolver::new();

    let mut layer1 = ConfigLayer {
        source: LayerSource::Global,
        ..Default::default()
    };
    layer1
        .mcp_servers
        .insert("github".into(), mcp_config("mcp-github-v1"));
    layer1
        .mcp_servers
        .insert("sqlite".into(), mcp_config("mcp-sqlite"));

    let mut layer2 = ConfigLayer {
        source: LayerSource::Project,
        ..Default::default()
    };
    layer2
        .mcp_servers
        .insert("github".into(), mcp_config("mcp-github-v2"));

    resolver.add_layer(layer1);
    resolver.add_layer(layer2);

    let config = resolver.resolve().unwrap();
    assert_eq!(config.mcp_servers.len(), 2);
    let McpServerConfig::Stdio { command, .. } = &config.mcp_servers["github"].config else {
        panic!("expected Stdio config");
    };
    assert_eq!(command, "mcp-github-v2");
    assert_eq!(config.mcp_servers["github"].source, LayerSource::Project);
    let McpServerConfig::Stdio { command, .. } = &config.mcp_servers["sqlite"].config else {
        panic!("expected Stdio config");
    };
    assert_eq!(command, "mcp-sqlite");
}

#[test]
fn test_resolve_mcp_disabled_removes() {
    let mut resolver = ConfigResolver::new();

    let mut layer1 = ConfigLayer {
        source: LayerSource::Global,
        ..Default::default()
    };
    layer1
        .mcp_servers
        .insert("noisy".into(), mcp_config("noisy-server"));

    let disabled = McpServerConfig::Stdio {
        command: "noisy-server".to_string(),
        args: Vec::new(),
        env: HashMap::new(),
        enabled: false,
        timeout_ms: 30_000,
    };
    let mut layer2 = ConfigLayer {
        source: LayerSource::Project,
        ..Default::default()
    };
    layer2.mcp_servers.insert("noisy".into(), disabled);

    resolver.add_layer(layer1);
    resolver.add_layer(layer2);

    let config = resolver.resolve().unwrap();
    assert!(config.mcp_servers.is_empty());
}

#[test]
fn test_resolve_skills_override_by_name() {
    let mut resolver = ConfigResolver::new();

    let skill1 = parse_skill("/commit", "Global commit skill.");
    let skill2 = parse_skill("/commit", "Project commit skill.");

    let mut layer1 = ConfigLayer {
        source: LayerSource::Global,
        ..Default::default()
    };
    layer1.skills = vec![skill1];
    let mut layer2 = ConfigLayer {
        source: LayerSource::Project,
        ..Default::default()
    };
    layer2.skills = vec![skill2];

    resolver.add_layer(layer1);
    resolver.add_layer(layer2);

    let config = resolver.resolve().unwrap();
    assert_eq!(config.skills.len(), 1);
    assert_eq!(
        config.skills["/commit"].skill.description,
        "Project commit skill."
    );
    assert_eq!(config.skills["/commit"].source, LayerSource::Project);
}

#[test]
fn test_resolve_hooks_append_all() {
    let mut resolver = ConfigResolver::new();

    let hook1 = HookConfig {
        event: HookEvent::PreToolUse,
        command: "echo global".into(),
        tool_filter: None,
        timeout_ms: 10_000,
    };
    let hook2 = HookConfig {
        event: HookEvent::PostToolUse,
        command: "echo project".into(),
        tool_filter: None,
        timeout_ms: 5_000,
    };

    let mut layer1 = ConfigLayer {
        source: LayerSource::Global,
        ..Default::default()
    };
    layer1.hooks = vec![hook1];
    let mut layer2 = ConfigLayer {
        source: LayerSource::Project,
        ..Default::default()
    };
    layer2.hooks = vec![hook2];

    resolver.add_layer(layer1);
    resolver.add_layer(layer2);

    let config = resolver.resolve().unwrap();
    assert_eq!(config.hooks.len(), 2);
    assert_eq!(config.hooks[0].config.command, "echo global");
    assert_eq!(config.hooks[1].config.command, "echo project");
}

#[test]
fn test_resolve_instructions_concatenated() {
    let mut resolver = ConfigResolver::new();

    let mut layer1 = ConfigLayer {
        source: LayerSource::Global,
        ..Default::default()
    };
    layer1.instructions = Some("Global instructions".into());
    let mut layer2 = ConfigLayer {
        source: LayerSource::Project,
        ..Default::default()
    };
    layer2.instructions = Some("Project instructions".into());

    resolver.add_layer(layer1);
    resolver.add_layer(layer2);

    let config = resolver.resolve().unwrap();
    assert!(config.instructions.contains("Global instructions"));
    assert!(config.instructions.contains("Project instructions"));
    assert!(config.instructions.contains("\n\n"));
}
