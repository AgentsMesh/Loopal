use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use loopal_tool_api::{Tool, ToolDefinition};

/// Thread-safe registry that holds all available tools by name.
///
/// Uses interior mutability (`RwLock`) so tools can be registered after the
/// owning `Kernel` is wrapped in `Arc` (e.g. MCP reconnect dynamic registration).
pub struct ToolRegistry {
    tools: RwLock<HashMap<String, Arc<dyn Tool>>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
        }
    }

    /// Register a tool. Overwrites any existing tool with the same name.
    pub fn register(&self, tool: Box<dyn Tool>) {
        let name = tool.name().to_string();
        self.tools.write().unwrap().insert(name, Arc::from(tool));
    }

    /// Get a tool by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.read().unwrap().get(name).cloned()
    }

    /// List all registered tools.
    pub fn list(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.read().unwrap().values().cloned().collect()
    }

    /// Convert all tools to definitions suitable for sending to an LLM.
    pub fn to_definitions(&self) -> Vec<ToolDefinition> {
        let guard = self.tools.read().unwrap();
        let mut defs: Vec<ToolDefinition> = guard
            .values()
            .map(|t| ToolDefinition {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: t.parameters_schema(),
            })
            .collect();
        defs.sort_by(|a, b| a.name.cmp(&b.name));
        defs
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
