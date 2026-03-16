pub mod bash;
pub mod edit;
pub mod glob;
pub mod grep;
pub mod ls;
pub mod read;
pub mod web_fetch;
pub mod write;

use crate::registry::ToolRegistry;

/// Register all built-in tools with the given registry.
pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(read::ReadTool));
    registry.register(Box::new(write::WriteTool));
    registry.register(Box::new(edit::EditTool));
    registry.register(Box::new(glob::GlobTool));
    registry.register(Box::new(grep::GrepTool));
    registry.register(Box::new(bash::BashTool));
    registry.register(Box::new(ls::LsTool));
    registry.register(Box::new(web_fetch::WebFetchTool));
}
