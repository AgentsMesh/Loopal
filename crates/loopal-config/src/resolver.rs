use std::path::PathBuf;
use std::sync::Arc;

use indexmap::IndexMap;

use loopal_error::{ConfigError, LoopalError};
use loopal_secret_runtime::{JsonlAuditSink, MergedVault, default_telemetry_dir};
use loopal_vault_age::AgeVault;
use loopal_vault_api::Vault;

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
        let mut classifier_prompt: Option<String> = None;
        let mut vaults_dir: Option<PathBuf> = None;
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

            // Hooks: dedup by id (higher layer wins), append others.
            for config in layer.hooks {
                if let Some(ref id) = config.id {
                    // Same id across layers: higher-priority layer replaces.
                    if let Some(pos) = hooks.iter().position(|h| h.config.id.as_ref() == Some(id)) {
                        hooks[pos] = HookEntry {
                            config,
                            source: layer.source.clone(),
                        };
                        continue;
                    }
                }
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

            // Memory: concatenate with source labels for precedence clarity
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

            // Classifier prompt: replace (not concat); later layer overrides.
            if let Some(text) = layer.classifier_prompt {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    classifier_prompt = Some(trimmed.to_string());
                }
            }

            // Vaults dir: highest-priority non-None wins (replace semantics).
            if let Some(path) = layer.vaults_dir {
                vaults_dir = Some(path);
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

fn build_secret_store(
    vaults_dir: Option<PathBuf>,
    default_vault_name: Option<&str>,
) -> Option<Arc<dyn Vault>> {
    let dir = vaults_dir?;
    let default_name = default_vault_name.unwrap_or("default");

    let all_names = loopal_vault_age::list_initialized_vaults(&dir);
    if all_names.is_empty() {
        return None;
    }

    let identity = match loopal_vault_age::discover() {
        Ok(i) => Arc::new(i),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "vaults present but SSH identity discovery failed; vaults disabled"
            );
            return None;
        }
    };

    let audit: Arc<dyn loopal_vault_api::AuditSink> = match default_telemetry_dir() {
        Some(td) => Arc::new(JsonlAuditSink::new(td)),
        None => Arc::new(loopal_vault_api::NoopAuditSink),
    };

    let mk = |name: &str| -> Arc<dyn Vault> {
        let store = dir.join(format!("{name}.vault")).join("store.age");
        let recipients = dir.join(format!("{name}.vault")).join("recipients");
        Arc::new(AgeVault::with_audit(
            store,
            recipients,
            identity.clone(),
            audit.clone(),
        ))
    };

    let default = if all_names.iter().any(|n| n == default_name) {
        (default_name.to_string(), mk(default_name))
    } else if default_vault_name.is_some() {
        // User explicitly configured a default_vault name that does not exist.
        // Fail-fast rather than silently falling back: an explicit config
        // pointing at a missing vault is a bug the user must see.
        tracing::error!(
            requested = default_name,
            available = ?all_names,
            "configured default_vault not found; vault subsystem disabled"
        );
        return None;
    } else {
        // No explicit configuration; fall back to alphabetical first.
        let first = all_names[0].clone();
        if first != "default" {
            tracing::info!(
                using = first.as_str(),
                "no 'default' vault present; using first alphabetical as default"
            );
        }
        (first.clone(), mk(&first))
    };
    let others: Vec<(String, Arc<dyn Vault>)> = all_names
        .iter()
        .filter(|n| n.as_str() != default.0)
        .map(|n| (n.clone(), mk(n)))
        .collect();

    if others.is_empty() {
        Some(default.1)
    } else {
        Some(Arc::new(MergedVault::new(default, others)))
    }
}

impl Default for ConfigResolver {
    fn default() -> Self {
        Self::new()
    }
}
