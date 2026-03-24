use std::collections::HashMap;

use serde::Serialize;

/// Runtime context passed to Minijinja for rendering prompt fragments.
///
/// All fields are available as template variables.
/// Example: `{% if "Bash" in tool_names %}...{% endif %}`
#[derive(Debug, Clone, Serialize)]
pub struct PromptContext {
    // -- Environment --
    pub cwd: String,
    pub platform: String,
    pub date: String,
    pub is_git_repo: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,

    // -- Mode --
    /// Current agent mode: "act", "plan", "auto".
    pub mode: String,

    // -- Tools --
    /// Names of all available tools (for `{% if "X" in tool_names %}`).
    pub tool_names: Vec<String>,
    /// Tool name → description mapping.
    pub tool_descriptions: HashMap<String, String>,

    // -- User content (injected raw, not rendered as templates) --
    pub instructions: String,
    pub memory: String,
    pub skills_summary: String,

    // -- Feature flags --
    pub features: Vec<String>,

    // -- Sub-agent context --
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
}

impl Default for PromptContext {
    fn default() -> Self {
        Self {
            cwd: String::new(),
            platform: std::env::consts::OS.to_string(),
            date: String::new(),
            is_git_repo: false,
            git_branch: None,
            mode: "act".to_string(),
            tool_names: Vec::new(),
            tool_descriptions: HashMap::new(),
            instructions: String::new(),
            memory: String::new(),
            skills_summary: String::new(),
            features: Vec::new(),
            agent_name: None,
            agent_type: None,
        }
    }
}
