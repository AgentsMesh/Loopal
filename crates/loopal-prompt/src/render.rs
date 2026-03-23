use minijinja::Environment;

use crate::context::PromptContext;

/// Thin wrapper around minijinja::Environment for rendering prompt templates.
pub struct PromptRenderer {
    env: Environment<'static>,
}

impl PromptRenderer {
    pub fn new() -> Self {
        let env = Environment::new();
        Self { env }
    }

    /// Render a template string with the given context.
    ///
    /// Returns the rendered text, or the raw template on error (best-effort).
    pub fn render(&self, template: &str, ctx: &PromptContext) -> String {
        let ctx_value = minijinja::Value::from_serialize(ctx);
        match self.env.render_str(template, ctx_value) {
            Ok(rendered) => rendered,
            Err(e) => {
                tracing::warn!(error = %e, "prompt template render failed, using raw content");
                template.to_string()
            }
        }
    }
}
