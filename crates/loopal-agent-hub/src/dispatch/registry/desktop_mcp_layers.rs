use std::{collections::BTreeMap, path::Path};

use loopal_config::{ConfigLayer, LayerSource, McpServerConfig};

pub(super) type EffectiveMcpConfig = (LayerSource, McpServerConfig);

pub(super) fn effective(root: &Path, expected: &str) -> Result<Option<EffectiveMcpConfig>, String> {
    Ok(
        replay(loopal_config::load_config_layers(root).map_err(|_| config_error())?)
            .remove(expected),
    )
}

pub(super) fn all(root: &Path) -> Result<BTreeMap<String, EffectiveMcpConfig>, String> {
    Ok(replay(
        loopal_config::load_config_layers(root).map_err(|_| config_error())?,
    ))
}

fn config_error() -> String {
    "Loopal configuration is invalid; repair the project settings files".into()
}

fn replay(layers: Vec<ConfigLayer>) -> BTreeMap<String, EffectiveMcpConfig> {
    let mut effective = BTreeMap::new();
    for layer in layers {
        for (name, config) in layer.mcp_servers {
            if layer.source == LayerSource::Local && is_tombstone(&config) {
                effective.remove(&name);
            } else {
                effective.insert(name, (layer.source.clone(), config));
            }
        }
    }
    effective
}

fn is_tombstone(config: &McpServerConfig) -> bool {
    matches!(
        config,
        McpServerConfig::Stdio { command, enabled: false, .. }
            if command == "__loopal_disabled__"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;
    use loopal_config::McpSharing;

    fn layer(source: LayerSource, rows: Vec<(&str, &str, bool)>) -> ConfigLayer {
        let mcp_servers = rows
            .into_iter()
            .map(|(name, command, enabled)| {
                (
                    name.into(),
                    McpServerConfig::Stdio {
                        command: command.into(),
                        args: Vec::new(),
                        env: Default::default(),
                        enabled,
                        timeout_ms: 30_000,
                        sharing: McpSharing::HubSingleton,
                        cwd_isolation: None,
                    },
                )
            })
            .collect::<IndexMap<_, _>>();
        ConfigLayer {
            source,
            mcp_servers,
            ..Default::default()
        }
    }

    #[test]
    fn replay_includes_plugin_disabled_and_higher_layer_override() {
        let layers = vec![
            layer(
                LayerSource::Plugin("tools".into()),
                vec![("plugin", "plugin", false)],
            ),
            layer(LayerSource::Project, vec![("same", "settings", true)]),
            layer(LayerSource::Project, vec![("same", "dot-mcp", false)]),
        ];
        let effective = replay(layers);
        assert!(!effective["plugin"].1.enabled());
        let McpServerConfig::Stdio {
            command, enabled, ..
        } = &effective["same"].1
        else {
            panic!("expected stdio")
        };
        assert_eq!(command, "dot-mcp");
        assert!(!enabled);
    }
}
