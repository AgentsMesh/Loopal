//! Resolver tests specific to hook merge/dedup semantics.

use loopal_config::hook::{HookConfig, HookEvent};
use loopal_config::layer::{ConfigLayer, LayerSource};
use loopal_config::resolver::ConfigResolver;

fn make_hook(event: HookEvent, command: &str, id: Option<&str>) -> HookConfig {
    HookConfig {
        event,
        command: command.into(),
        tool_filter: None,
        timeout_ms: 10_000,
        hook_type: Default::default(),
        url: None,
        headers: Default::default(),
        prompt: None,
        model: None,
        condition: None,
        id: id.map(String::from),
    }
}

#[test]
fn test_resolve_hooks_append_all() {
    let mut resolver = ConfigResolver::new();

    let hook1 = make_hook(HookEvent::PreToolUse, "echo global", None);
    let hook2 = make_hook(HookEvent::PostToolUse, "echo project", None);

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
fn test_resolve_hooks_dedup_by_id_higher_priority_wins() {
    let mut resolver = ConfigResolver::new();

    let mut global = ConfigLayer {
        source: LayerSource::Global,
        ..Default::default()
    };
    global.hooks = vec![
        make_hook(HookEvent::PreToolUse, "echo global-lint", Some("lint")),
        make_hook(HookEvent::PostToolUse, "echo global-log", None),
    ];

    let mut project = ConfigLayer {
        source: LayerSource::Project,
        ..Default::default()
    };
    project.hooks = vec![make_hook(
        HookEvent::PreToolUse,
        "echo project-lint",
        Some("lint"),
    )];

    resolver.add_layer(global);
    resolver.add_layer(project);

    let config = resolver.resolve().unwrap();
    // id="lint" deduped: project wins over global.
    // id=None hook appended.
    assert_eq!(config.hooks.len(), 2);
    let lint = config
        .hooks
        .iter()
        .find(|h| h.config.id.as_deref() == Some("lint"))
        .unwrap();
    assert_eq!(lint.config.command, "echo project-lint");
    assert_eq!(lint.source, LayerSource::Project);
    // No-id hook preserved.
    let log = config
        .hooks
        .iter()
        .find(|h| h.config.id.is_none())
        .unwrap();
    assert_eq!(log.config.command, "echo global-log");
}

#[test]
fn test_resolve_hooks_written_back_to_settings() {
    let mut resolver = ConfigResolver::new();
    let hook = make_hook(HookEvent::PreToolUse, "echo test", None);
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
