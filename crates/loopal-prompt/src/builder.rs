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
    /// Assembly order (identity-first for stronger LLM attention):
    /// 1. Matched & rendered fragments (sorted by priority: core → tasks → tools → modes)
    /// 2. User instructions (project-specific, after core behavioral rules)
    /// 3. Skills summary
    /// 4. Project memory (tail)
    pub fn build(&self, ctx: &PromptContext) -> String {
        let mut parts = Vec::new();

        // 1. Fragments: select → sort → render → collect
        for frag in self.registry.select(ctx) {
            let rendered = self.registry.render(frag, ctx);
            let trimmed = rendered.trim();
            if !trimmed.is_empty() {
                parts.push(trimmed.to_string());
            }
        }

        // 2. User instructions (after fragments so identity/core rules come first)
        if !ctx.instructions.is_empty() {
            parts.push(ctx.instructions.clone());
        }

        // 3. Skills summary
        if !ctx.skills_summary.is_empty() {
            parts.push(ctx.skills_summary.clone());
        }

        // 4. Memory (tail position — curated by Knowledge Manager agent)
        if !ctx.memory.is_empty() {
            parts.push(format!(
                "# Memory\n\n\
                 When global and project memory conflict, project memory takes precedence.\n\n\
                 {}",
                ctx.memory
            ));
        }

        parts.join("\n\n")
    }

    /// Access the underlying registry.
    pub fn registry(&self) -> &FragmentRegistry {
        &self.registry
    }
}
