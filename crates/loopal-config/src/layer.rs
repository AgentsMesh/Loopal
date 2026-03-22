use indexmap::IndexMap;

use crate::hook::HookConfig;
use crate::settings::McpServerConfig;
use crate::skills::Skill;

/// Identifies where a configuration layer originates from.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LayerSource {
    /// Global config (~/.loopal/)
    #[default]
    Global,
    /// Plugin directory (~/.loopal/plugins/<name>/)
    Plugin(String),
    /// Project config (<cwd>/.loopal/)
    Project,
    /// Local overrides (settings.local.json + LOOPAL.local.md)
    Local,
    /// Environment variable overrides
    Env,
    /// CLI argument overrides
    Cli,
}

impl std::fmt::Display for LayerSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plugin(name) => write!(f, "plugin:{name}"),
            Self::Global => write!(f, "global"),
            Self::Project => write!(f, "project"),
            Self::Local => write!(f, "local"),
            Self::Env => write!(f, "env"),
            Self::Cli => write!(f, "cli"),
        }
    }
}

/// Raw configuration from a single layer before merging.
///
/// Each field is optional/empty by default — only populated values
/// participate in the merge.
#[derive(Debug, Clone, Default)]
pub struct ConfigLayer {
    /// Where this layer was loaded from
    pub source: LayerSource,
    /// Raw settings JSON (deep-merged with other layers)
    pub settings: serde_json::Value,
    /// MCP server configs keyed by name
    pub mcp_servers: IndexMap<String, McpServerConfig>,
    /// Parsed skills
    pub skills: Vec<Skill>,
    /// Hook configurations
    pub hooks: Vec<HookConfig>,
    /// Instruction text (from LOOPAL.md)
    pub instructions: Option<String>,
    /// Memory content (from memory/MEMORY.md)
    pub memory: Option<String>,
}
