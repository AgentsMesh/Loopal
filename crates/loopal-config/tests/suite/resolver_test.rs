use std::collections::HashMap;

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
    assert_eq!(config.settings.model, "claude-opus-4-7");
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
    layer1.settings = serde_json::json!({"model": "gpt-4", "max_context_tokens": 100});

    let mut layer2 = ConfigLayer {
        source: LayerSource::Project,
        ..Default::default()
    };
    layer2.settings = serde_json::json!({"max_context_tokens": 200});

    resolver.add_layer(layer1);
    resolver.add_layer(layer2);

    let config = resolver.resolve().unwrap();
    assert_eq!(config.settings.model, "gpt-4");
    assert_eq!(config.settings.max_context_tokens, 200);
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

// ---------------------------------------------------------------------------
// Memory layer labeling tests
// ---------------------------------------------------------------------------

#[test]
fn test_resolve_memory_global_labeled() {
    let mut resolver = ConfigResolver::new();
    let mut layer = ConfigLayer {
        source: LayerSource::Global,
        ..Default::default()
    };
    layer.memory = Some("User prefers Rust".into());
    resolver.add_layer(layer);

    let config = resolver.resolve().unwrap();
    assert!(
        config.memory.contains("## Global Memory"),
        "global memory should be labeled: {}",
        config.memory
    );
    assert!(config.memory.contains("User prefers Rust"));
}

#[test]
fn test_resolve_memory_project_labeled() {
    let mut resolver = ConfigResolver::new();
    let mut layer = ConfigLayer {
        source: LayerSource::Project,
        ..Default::default()
    };
    layer.memory = Some("Use snake_case".into());
    resolver.add_layer(layer);

    let config = resolver.resolve().unwrap();
    assert!(
        config.memory.contains("## Project Memory"),
        "project memory should be labeled: {}",
        config.memory
    );
    assert!(config.memory.contains("Use snake_case"));
}

#[test]
fn test_resolve_memory_plugin_labeled() {
    let mut resolver = ConfigResolver::new();
    let mut layer = ConfigLayer {
        source: LayerSource::Plugin("my-plugin".into()),
        ..Default::default()
    };
    layer.memory = Some("Plugin-specific fact".into());
    resolver.add_layer(layer);

    let config = resolver.resolve().unwrap();
    assert!(
        config.memory.contains("## Plugin Memory: my-plugin"),
        "plugin memory should be labeled: {}",
        config.memory
    );
}

#[test]
fn test_resolve_memory_multiple_layers_concatenated() {
    let mut resolver = ConfigResolver::new();

    let mut global = ConfigLayer {
        source: LayerSource::Global,
        ..Default::default()
    };
    global.memory = Some("Global fact".into());

    let mut project = ConfigLayer {
        source: LayerSource::Project,
        ..Default::default()
    };
    project.memory = Some("Project fact".into());

    resolver.add_layer(global);
    resolver.add_layer(project);

    let config = resolver.resolve().unwrap();
    assert!(config.memory.contains("## Global Memory"));
    assert!(config.memory.contains("## Project Memory"));
    assert!(config.memory.contains("Global fact"));
    assert!(config.memory.contains("Project fact"));
    // Verify they are separated
    assert!(config.memory.contains("\n\n"));
}

#[test]
fn test_resolve_memory_empty_layers_skipped() {
    let mut resolver = ConfigResolver::new();

    let mut layer1 = ConfigLayer {
        source: LayerSource::Global,
        ..Default::default()
    };
    layer1.memory = Some("   ".into()); // whitespace-only

    let mut layer2 = ConfigLayer {
        source: LayerSource::Project,
        ..Default::default()
    };
    layer2.memory = Some("Actual content".into());

    resolver.add_layer(layer1);
    resolver.add_layer(layer2);

    let config = resolver.resolve().unwrap();
    assert!(
        !config.memory.contains("## Global Memory"),
        "whitespace-only memory should be skipped"
    );
    assert!(config.memory.contains("## Project Memory"));
}

// ---------------------------------------------------------------------------
// MemoryConfig defaults test
// ---------------------------------------------------------------------------

#[test]
fn test_memory_config_defaults() {
    let config = loopal_config::settings::MemoryConfig::default();
    assert!(config.enabled);
    assert_eq!(config.batch_window_ms, 2000);
    assert_eq!(config.channel_buffer, 256);
    assert_eq!(config.consolidation_interval_days, 7);
}

#[test]
fn test_memory_config_deserialize_partial() {
    let json = serde_json::json!({"enabled": false});
    let config: loopal_config::settings::MemoryConfig = serde_json::from_value(json).unwrap();
    assert!(!config.enabled);
    // Other fields should have defaults
    assert_eq!(config.batch_window_ms, 2000);
    assert_eq!(config.channel_buffer, 256);
    assert_eq!(config.consolidation_interval_days, 7);
}

#[test]
fn test_memory_config_deserialize_full() {
    let json = serde_json::json!({
        "enabled": true,
        "batch_window_ms": 5000,
        "channel_buffer": 512,
        "consolidation_interval_days": 14
    });
    let config: loopal_config::settings::MemoryConfig = serde_json::from_value(json).unwrap();
    assert!(config.enabled);
    assert_eq!(config.batch_window_ms, 5000);
    assert_eq!(config.channel_buffer, 512);
    assert_eq!(config.consolidation_interval_days, 14);
}

#[test]
fn test_memory_config_in_settings() {
    let json = serde_json::json!({"memory": {"batch_window_ms": 1000}});
    let settings: loopal_config::settings::Settings = serde_json::from_value(json).unwrap();
    assert_eq!(settings.memory.batch_window_ms, 1000);
    assert!(settings.memory.enabled); // default
}

#[test]
fn test_resolve_memory_env_labeled() {
    let mut resolver = ConfigResolver::new();
    let mut layer = ConfigLayer {
        source: LayerSource::Env,
        ..Default::default()
    };
    layer.memory = Some("Env memory fact".into());
    resolver.add_layer(layer);

    let config = resolver.resolve().unwrap();
    assert!(
        config.memory.contains("## Environment Memory"),
        "env memory should be labeled: {}",
        config.memory
    );
}

#[test]
fn test_resolve_memory_cli_labeled() {
    let mut resolver = ConfigResolver::new();
    let mut layer = ConfigLayer {
        source: LayerSource::Cli,
        ..Default::default()
    };
    layer.memory = Some("CLI memory fact".into());
    resolver.add_layer(layer);

    let config = resolver.resolve().unwrap();
    assert!(
        config.memory.contains("## CLI Memory"),
        "cli memory should be labeled: {}",
        config.memory
    );
}
