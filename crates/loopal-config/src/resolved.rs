use indexmap::IndexMap;

use crate::hook::HookConfig;
use crate::layer::LayerSource;
use crate::settings::{McpServerConfig, Settings};
use crate::skills::Skill;

/// Fully resolved configuration after merging all layers.
#[derive(Debug, Clone)]
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
    /// Layer sources in merge order (for debugging)
    pub layers: Vec<LayerSource>,
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
