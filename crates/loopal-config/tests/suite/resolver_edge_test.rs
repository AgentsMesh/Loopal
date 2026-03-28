use std::collections::HashMap;

use loopal_config::hook::{HookConfig, HookEvent};
use loopal_config::layer::{ConfigLayer, LayerSource};
use loopal_config::resolver::ConfigResolver;
use loopal_config::settings::McpServerConfig;

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
fn test_resolve_empty_instructions_skipped() {
    let mut resolver = ConfigResolver::new();
    let mut layer1 = ConfigLayer {
        source: LayerSource::Global,
        ..Default::default()
    };
    layer1.instructions = Some("   ".into());
    let mut layer2 = ConfigLayer {
        source: LayerSource::Project,
        ..Default::default()
    };
    layer2.instructions = Some("Real content".into());
    resolver.add_layer(layer1);
    resolver.add_layer(layer2);
    let config = resolver.resolve().unwrap();
    assert_eq!(config.instructions, "Real content");
}

#[test]
fn test_resolve_mcp_written_back_to_settings() {
    let mut resolver = ConfigResolver::new();
    let mut layer = ConfigLayer {
        source: LayerSource::Global,
        ..Default::default()
    };
    layer
        .mcp_servers
        .insert("test".into(), mcp_config("test-cmd"));
    resolver.add_layer(layer);
    let config = resolver.resolve().unwrap();
    assert_eq!(config.mcp_servers.len(), 1);
    assert_eq!(config.settings.mcp_servers.len(), 1);
    let mcp = config.settings.mcp_servers.get("test").unwrap();
    let McpServerConfig::Stdio { command, .. } = mcp else {
        panic!("expected Stdio config");
    };
    assert_eq!(command, "test-cmd");
}

#[test]
fn test_resolve_hooks_written_back_to_settings() {
    let mut resolver = ConfigResolver::new();
    let hook = HookConfig {
        event: HookEvent::PreToolUse,
        command: "echo test".into(),
        tool_filter: None,
        timeout_ms: 10_000,
    };
    let mut layer = ConfigLayer {
        source: LayerSource::Global,
        ..Default::default()
    };
    layer.hooks = vec![hook];
    resolver.add_layer(layer);
    let config = resolver.resolve().unwrap();
    assert_eq!(config.hooks.len(), 1);
    assert_eq!(config.settings.hooks.len(), 1);
    assert_eq!(config.settings.hooks[0].command, "echo test");
}

#[test]
fn test_resolve_layers_tracked() {
    let mut resolver = ConfigResolver::new();
    resolver.add_layer(ConfigLayer {
        source: LayerSource::Plugin("foo".into()),
        ..Default::default()
    });
    resolver.add_layer(ConfigLayer {
        source: LayerSource::Global,
        ..Default::default()
    });
    let config = resolver.resolve().unwrap();
    assert_eq!(config.layers.len(), 2);
    assert_eq!(config.layers[0], LayerSource::Plugin("foo".into()));
    assert_eq!(config.layers[1], LayerSource::Global);
}

#[test]
fn test_resolve_null_settings_layer_skipped() {
    let mut resolver = ConfigResolver::new();
    // Default ConfigLayer has settings = Value::Null
    resolver.add_layer(ConfigLayer::default());
    let config = resolver.resolve().unwrap();
    assert_eq!(config.settings.model, "claude-sonnet-4-20250514");
}

#[test]
fn test_layer_source_display() {
    assert_eq!(LayerSource::Global.to_string(), "global");
    assert_eq!(LayerSource::Project.to_string(), "project");
    assert_eq!(LayerSource::Local.to_string(), "local");
    assert_eq!(LayerSource::Env.to_string(), "env");
    assert_eq!(LayerSource::Cli.to_string(), "cli");
    assert_eq!(LayerSource::Plugin("foo".into()).to_string(), "plugin:foo");
}

#[test]
fn test_layer_source_default() {
    assert_eq!(LayerSource::default(), LayerSource::Global);
}

#[test]
fn test_load_layer_invalid_mcp_logs_warning() {
    // mcp_servers with wrong types should not crash, just be skipped
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("settings.json"),
        r#"{"mcp_servers": {"bad": {"command": 123}}}"#,
    )
    .unwrap();
    let layer =
        loopal_config::loader::load_layer_from_dir(dir.path(), LayerSource::Global, None).unwrap();
    // Invalid MCP config should be skipped
    assert!(layer.mcp_servers.is_empty());
    // The raw JSON still has the key removed
    assert!(layer.settings.get("mcp_servers").is_none());
}

#[test]
fn test_load_layer_invalid_hooks_logs_warning() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("settings.json"),
        r#"{"hooks": [{"event": "invalid_event", "command": "echo"}]}"#,
    )
    .unwrap();
    let layer =
        loopal_config::loader::load_layer_from_dir(dir.path(), LayerSource::Global, None).unwrap();
    assert!(layer.hooks.is_empty());
    assert!(layer.settings.get("hooks").is_none());
}
