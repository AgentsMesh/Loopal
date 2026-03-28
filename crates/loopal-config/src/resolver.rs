use indexmap::IndexMap;

use loopal_error::{ConfigError, LoopalError};

use crate::layer::{ConfigLayer, LayerSource};
use crate::loader::deep_merge;
use crate::resolved::{HookEntry, McpServerEntry, ResolvedConfig, SkillEntry};
use crate::settings::Settings;

/// Merges multiple `ConfigLayer`s into a single `ResolvedConfig`.
///
/// Layers are added in priority order (lowest first). Later layers
/// override earlier ones according to per-field merge semantics.
pub struct ConfigResolver {
    layers: Vec<ConfigLayer>,
}

impl ConfigResolver {
    pub fn new() -> Self {
        Self { layers: Vec::new() }
    }

    /// Append a layer (higher priority than all previously added layers).
    pub fn add_layer(&mut self, layer: ConfigLayer) {
        self.layers.push(layer);
    }

    /// Consume all layers and produce a merged `ResolvedConfig`.
    pub fn resolve(self) -> Result<ResolvedConfig, LoopalError> {
        let mut merged_settings = serde_json::to_value(Settings::default())
            .map_err(|e| ConfigError::Parse(e.to_string()))?;

        let mut mcp_servers: IndexMap<String, McpServerEntry> = IndexMap::new();
        let mut skills: IndexMap<String, SkillEntry> = IndexMap::new();
        let mut hooks: Vec<HookEntry> = Vec::new();
        let mut instruction_parts: Vec<String> = Vec::new();
        let mut memory_parts: Vec<String> = Vec::new();
        let mut sources: Vec<LayerSource> = Vec::new();

        for layer in self.layers {
            sources.push(layer.source.clone());

            // Settings: deep merge (objects recursive, scalars replace)
            if !layer.settings.is_null() {
                deep_merge(&mut merged_settings, layer.settings);
            }

            // MCP servers: override by name; enabled=false removes
            for (name, config) in layer.mcp_servers {
                if config.enabled() {
                    mcp_servers.insert(
                        name,
                        McpServerEntry {
                            config,
                            source: layer.source.clone(),
                        },
                    );
                } else {
                    mcp_servers.shift_remove(&name);
                }
            }

            // Skills: override by name
            for skill in layer.skills {
                let name = skill.name.clone();
                skills.insert(
                    name,
                    SkillEntry {
                        skill,
                        source: layer.source.clone(),
                    },
                );
            }

            // Hooks: append all, preserving order
            for config in layer.hooks {
                hooks.push(HookEntry {
                    config,
                    source: layer.source.clone(),
                });
            }

            // Instructions: concatenate
            if let Some(text) = layer.instructions {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    instruction_parts.push(trimmed.to_string());
                }
            }

            // Memory: concatenate (same semantics as instructions)
            if let Some(text) = layer.memory {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    memory_parts.push(trimmed.to_string());
                }
            }
        }

        // Warn about unrecognised keys before deserialising
        crate::validate::warn_unknown_keys(&merged_settings);

        let mut settings: Settings = serde_json::from_value(merged_settings)
            .map_err(|e| ConfigError::Parse(e.to_string()))?;

        // Sync resolved typed fields into Settings so that downstream consumers
        // (Kernel, HookRegistry) that only read Settings get the merged view.
        settings.mcp_servers = mcp_servers
            .iter()
            .map(|(name, entry)| (name.clone(), entry.config.clone()))
            .collect();
        settings.hooks = hooks.iter().map(|h| h.config.clone()).collect();

        Ok(ResolvedConfig {
            settings,
            mcp_servers,
            skills,
            hooks,
            instructions: instruction_parts.join("\n\n"),
            memory: memory_parts.join("\n\n"),
            layers: sources,
        })
    }
}

impl Default for ConfigResolver {
    fn default() -> Self {
        Self::new()
    }
}
