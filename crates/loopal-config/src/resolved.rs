use std::sync::Arc;

use indexmap::IndexMap;
use loopal_provider_api::ThinkingConfig;
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
    /// Preset recommendation kept outside serialized user settings.
    pub workflow_preset_thinking_recommendation: Option<ThinkingConfig>,
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
            .field("mcp_server_count", &self.mcp_servers.len())
            .field("skill_count", &self.skills.len())
            .field("hook_count", &self.hooks.len())
            .field("instruction_bytes", &self.instructions.len())
            .field("memory_bytes", &self.memory.len())
            .field("has_classifier_prompt", &self.classifier_prompt.is_some())
            .field(
                "has_workflow_preset_thinking_recommendation",
                &self.workflow_preset_thinking_recommendation.is_some(),
            )
            .field("layer_count", &self.layers.len())
            .field("secrets", &self.secrets.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::ProviderConfig;

    #[test]
    fn debug_reports_shape_without_configuration_contents() {
        let marker = "resolved-config-secret-marker";
        let mut settings = Settings::default();
        settings.providers.anthropic = Some(ProviderConfig {
            api_key: Some(marker.into()),
            api_key_env: None,
            base_url: None,
        });
        let resolved = ResolvedConfig {
            settings,
            workflow_preset_thinking_recommendation: None,
            mcp_servers: Default::default(),
            skills: Default::default(),
            hooks: Vec::new(),
            instructions: marker.into(),
            memory: marker.into(),
            classifier_prompt: Some(marker.into()),
            layers: vec![LayerSource::Local],
            secrets: None,
        };
        let debug = format!("{resolved:?}");
        assert!(!debug.contains(marker));
        assert!(debug.contains("mcp_server_count"));
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
