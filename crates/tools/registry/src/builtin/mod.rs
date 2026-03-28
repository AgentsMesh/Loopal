use crate::registry::ToolRegistry;

/// Register all built-in tools with the given registry.
pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Box::new(loopal_tool_apply_patch::ApplyPatchTool));
    registry.register(Box::new(loopal_tool_read::ReadTool));
    registry.register(Box::new(loopal_tool_write::WriteTool));
    registry.register(Box::new(loopal_tool_edit::EditTool));
    registry.register(Box::new(loopal_tool_multi_edit::MultiEditTool));
    registry.register(Box::new(loopal_tool_glob::GlobTool));
    registry.register(Box::new(loopal_tool_grep::GrepTool));
    registry.register(Box::new(loopal_tool_bash::BashTool));
    registry.register(Box::new(loopal_tool_ls::LsTool));
    registry.register(Box::new(loopal_tool_fetch::FetchTool));
    registry.register(Box::new(loopal_tool_web_search::WebSearchTool));
    registry.register(Box::new(loopal_tool_ask_user::AskUserTool));
    registry.register(Box::new(loopal_tool_plan_mode::EnterPlanModeTool));
    registry.register(Box::new(loopal_tool_plan_mode::ExitPlanModeTool));
    registry.register(Box::new(loopal_tool_file_ops::move_file::MoveFileTool));
    registry.register(Box::new(loopal_tool_file_ops::delete::DeleteTool));
    registry.register(Box::new(loopal_tool_file_ops::copy::CopyFileTool));
}
