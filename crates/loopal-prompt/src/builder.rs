use crate::context::PromptContext;
use crate::registry::FragmentRegistry;

/// Assembles the final system prompt from fragments + user content.
pub struct PromptBuilder {
    registry: FragmentRegistry,
}

impl PromptBuilder {
    pub fn new(registry: FragmentRegistry) -> Self {
        Self { registry }
    }

    /// Build the full system prompt for the given context.
    ///
    /// Assembly order:
    /// 1. User instructions (raw, highest priority)
    /// 2. Matched & rendered fragments (sorted by priority)
    /// 3. Skills summary
    /// 4. Project memory (tail)
    pub fn build(&self, ctx: &PromptContext) -> String {
        let mut parts = Vec::new();

        // 1. User instructions (injected raw)
        if !ctx.instructions.is_empty() {
            parts.push(ctx.instructions.clone());
        }

        // 2. Fragments: select → sort → render → collect
        for frag in self.registry.select(ctx) {
            let rendered = self.registry.render(frag, ctx);
            let trimmed = rendered.trim();
            if !trimmed.is_empty() {
                parts.push(trimmed.to_string());
            }
        }

        // 3. Skills summary
        if !ctx.skills_summary.is_empty() {
            parts.push(ctx.skills_summary.clone());
        }

        // 4. Memory (tail position)
        if !ctx.memory.is_empty() {
            parts.push(format!("# Project Memory\n{}", ctx.memory));
        }

        parts.join("\n\n")
    }

    /// Build a prompt for a specific sub-agent type.
    ///
    /// Looks for a fragment with id "agents/{agent_type}" and renders it.
    /// Falls back to the default sub-agent prompt if not found.
    pub fn build_agent_prompt(&self, agent_type: &str, ctx: &PromptContext) -> String {
        let agent_id = format!("agents/{agent_type}");
        if let Some(frag) = self.registry.fragments().iter().find(|f| f.id == agent_id) {
            self.registry.render(frag, ctx)
        } else {
            default_agent_prompt(ctx)
        }
    }

    /// Access the underlying registry.
    pub fn registry(&self) -> &FragmentRegistry {
        &self.registry
    }
}

fn default_agent_prompt(ctx: &PromptContext) -> String {
    let name = ctx.agent_name.as_deref().unwrap_or("sub-agent");
    format!(
        "You are a sub-agent named '{name}'. Your working directory is: {}. \
         Complete the task given to you and report your findings.",
        ctx.cwd
    )
}
