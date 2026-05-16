use std::sync::Arc;

use indexmap::IndexMap;
use loopal_vault_api::Vault;

use crate::hook::HookConfig;
use crate::layer::LayerSource;
use crate::settings::{McpServerConfig, Settings};
use crate::skills::Skill;

/// Fully resolved configuration after merging all layers.
#[derive(Clone)]
pub struct ResolvedConfig {
    /// Deserialized settings (model, providers, sandbox, etc.)
    pub settings: Settings,
    /// MCP servers keyed by name, with provenance
    pub mcp_servers: IndexMap<String, McpServerEntry>,
    /// Skills keyed by name, with provenance
    pub skills: IndexMap<String, SkillEntry>,
    /// All hooks in layer order, with provenance
    pub hooks: Vec<HookEntry>,
    /// Concatenated instruction text from all layers
    pub instructions: String,
    /// Concatenated memory content from all layers
    pub memory: String,
    /// Optional Classifier-mode system prompt loaded from `.loopal/classifier.md`.
    /// Highest-priority non-empty layer wins. None means "use the built-in default".
    pub classifier_prompt: Option<String>,
    /// Layer sources in merge order (for debugging)
    pub layers: Vec<LayerSource>,
    /// Encrypted secrets vault. None when no vault is configured, or when SSH
    /// identity discovery failed. The store is lazy: instantiating it does not
    /// decrypt the vault.
    pub secrets: Option<Arc<dyn Vault>>,
}

impl std::fmt::Debug for ResolvedConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedConfig")
            .field("settings", &self.settings)
            .field("mcp_servers", &self.mcp_servers)
            .field("skills", &self.skills)
            .field("hooks", &self.hooks)
            .field("instructions", &self.instructions)
            .field("memory", &self.memory)
            .field("classifier_prompt", &self.classifier_prompt)
            .field("layers", &self.layers)
            .field("secrets", &self.secrets.is_some())
            .finish()
    }
}

/// An MCP server config with its originating layer.
#[derive(Debug, Clone)]
pub struct McpServerEntry {
    pub config: McpServerConfig,
    pub source: LayerSource,
}

/// A skill with its originating layer.
#[derive(Debug, Clone)]
pub struct SkillEntry {
    pub skill: Skill,
    pub source: LayerSource,
}

/// A hook config with its originating layer.
#[derive(Debug, Clone)]
pub struct HookEntry {
    pub config: HookConfig,
    pub source: LayerSource,
}
