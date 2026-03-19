use std::collections::HashMap;

use loopal_types::tool::{Tool, ToolDefinition};

/// Registry that holds all available tools by name.
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool. Overwrites any existing tool with the same name.
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    /// Get a tool by name.
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    /// List all registered tools.
    pub fn list(&self) -> Vec<&dyn Tool> {
        self.tools.values().map(|t| t.as_ref()).collect()
    }

    /// Wrap every registered tool with a decorator function.
    /// Used by sandbox to wrap all tools in `SandboxedTool`.
    pub fn wrap_all(&mut self, wrapper: impl Fn(Box<dyn Tool>) -> Box<dyn Tool>) {
        let old = std::mem::take(&mut self.tools);
        for (name, tool) in old {
            let wrapped = wrapper(tool);
            self.tools.insert(name, wrapped);
        }
    }

    /// Convert all tools to definitions suitable for sending to an LLM.
    pub fn to_definitions(&self) -> Vec<ToolDefinition> {
        let mut defs: Vec<ToolDefinition> = self
            .tools
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
