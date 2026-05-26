mod vault;

use indexmap::IndexMap;

use loopal_error::{ConfigError, LoopalError};

use self::vault::build_secret_store;
use crate::layer::{ConfigLayer, LayerSource};
use crate::loader::deep_merge;
use crate::resolved::{HookEntry, McpServerEntry, ResolvedConfig, SkillEntry};
use crate::settings::Settings;

pub struct ConfigResolver {
    layers: Vec<ConfigLayer>,
}

impl ConfigResolver {
    pub fn new() -> Self {
        Self { layers: Vec::new() }
    }

    pub fn add_layer(&mut self, layer: ConfigLayer) {
        self.layers.push(layer);
    }

    pub fn resolve(self) -> Result<ResolvedConfig, LoopalError> {
        let mut merged_settings = serde_json::to_value(Settings::default())
            .map_err(|e| ConfigError::Parse(e.to_string()))?;

        let mut mcp_servers: IndexMap<String, McpServerEntry> = IndexMap::new();
        let mut skills: IndexMap<String, SkillEntry> = IndexMap::new();
        let mut hooks: Vec<HookEntry> = Vec::new();
        let mut instruction_parts: Vec<String> = Vec::new();
        let mut memory_parts: Vec<String> = Vec::new();
        let mut classifier_prompt: Option<String> = None;
        let mut vaults_dir: Option<std::path::PathBuf> = None;
        let mut sources: Vec<LayerSource> = Vec::new();

        for layer in self.layers {
            sources.push(layer.source.clone());

            if !layer.settings.is_null() {
                deep_merge(&mut merged_settings, layer.settings);
            }

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

            // Hooks dedup by id: same id across layers replaces in place;
            // unidentified hooks always append.
            for config in layer.hooks {
                if let Some(ref id) = config.id
                    && let Some(pos) = hooks.iter().position(|h| h.config.id.as_ref() == Some(id))
                {
                    hooks[pos] = HookEntry {
                        config,
                        source: layer.source.clone(),
                    };
                    continue;
                }
                hooks.push(HookEntry {
                    config,
                    source: layer.source.clone(),
                });
            }

            if let Some(text) = layer.instructions {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    instruction_parts.push(trimmed.to_string());
                }
            }

            if let Some(text) = layer.memory {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    let label = match &layer.source {
                        LayerSource::Plugin(name) => format!("## Plugin Memory: {name}"),
                        LayerSource::Global => "## Global Memory".to_string(),
                        LayerSource::Project => "## Project Memory".to_string(),
                        LayerSource::Local => "## Local Memory".to_string(),
                        LayerSource::Env => "## Environment Memory".to_string(),
                        LayerSource::Cli => "## CLI Memory".to_string(),
                    };
                    memory_parts.push(format!("{label}\n\n{trimmed}"));
                }
            }

            // Classifier prompt uses replace semantics (not concat): later
            // layer's non-empty value fully overrides earlier layers.
            if let Some(text) = layer.classifier_prompt {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    classifier_prompt = Some(trimmed.to_string());
                }
            }

            if let Some(path) = layer.vaults_dir {
                vaults_dir = Some(path);
            }
        }

        crate::validate::warn_unknown_keys(&merged_settings);

        let mut settings: Settings = serde_json::from_value(merged_settings)
            .map_err(|e| ConfigError::Parse(e.to_string()))?;

        let (sanitized_compaction, compact_warnings) = settings.compaction.clone().sanitize();
        for w in &compact_warnings {
            tracing::warn!("{w}");
        }
        settings.compaction = sanitized_compaction;

        // Mirror typed fields back into Settings so Kernel / HookRegistry
        // (which read Settings directly) see the merged view.
        settings.mcp_servers = mcp_servers
            .iter()
            .map(|(name, entry)| (name.clone(), entry.config.clone()))
            .collect();
        settings.hooks = hooks.iter().map(|h| h.config.clone()).collect();

        Ok(ResolvedConfig {
            secrets: build_secret_store(
                settings.secrets.vaults_dir.clone().or(vaults_dir),
                settings.secrets.default_vault.as_deref(),
            ),
            settings,
            mcp_servers,
            skills,
            hooks,
            instructions: instruction_parts.join("\n\n"),
            memory: memory_parts.join("\n\n"),
            classifier_prompt,
            layers: sources,
        })
    }
}

impl Default for ConfigResolver {
    fn default() -> Self {
        Self::new()
    }
}
