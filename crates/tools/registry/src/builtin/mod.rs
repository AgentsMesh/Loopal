use std::sync::Arc;

use loopal_tool_background::BackgroundTaskStore;

use crate::registry::ToolRegistry;

/// Register all built-in tools that every agent (root or sub) needs.
///
/// Role-scoped tools (e.g. thread-goal tools that only make sense on
/// the root agent that owns the goal session) live in dedicated
/// `register_*_tools` helpers below; callers pick what to register.
pub fn register_all(registry: &ToolRegistry, bg_store: Arc<BackgroundTaskStore>) {
    registry.register(Box::new(loopal_tool_apply_patch::ApplyPatchTool));
    registry.register(Box::new(loopal_tool_read::ReadTool));
    registry.register(Box::new(loopal_tool_read_pdf::ReadPdfTool));
    registry.register(Box::new(loopal_tool_read_html::ReadHtmlTool));
    registry.register(Box::new(loopal_tool_write::WriteTool));
    registry.register(Box::new(loopal_tool_edit::EditTool));
    registry.register(Box::new(loopal_tool_multi_edit::MultiEditTool));
    registry.register(Box::new(loopal_tool_glob::GlobTool));
    registry.register(Box::new(loopal_tool_grep::GrepTool));
    registry.register(Box::new(loopal_tool_bash::BashTool::new(bg_store)));
    registry.register(Box::new(loopal_tool_ls::LsTool));
    registry.register(Box::new(loopal_tool_fetch::FetchTool));
    // reason: WebSearch is a client-side placeholder that the Anthropic/Google/
    // OpenAI providers detect by name and replace with their server-side
    // `web_search` declaration; without registering it here the providers
    // never inject the server tool and the model loses search capability.
    registry.register(Box::new(loopal_tool_web_search::WebSearchTool));
    registry.register(Box::new(loopal_tool_ask_user::AskUserTool));
    registry.register(Box::new(loopal_tool_plan_mode::EnterPlanModeTool));
    registry.register(Box::new(loopal_tool_plan_mode::ExitPlanModeTool));
    registry.register(Box::new(loopal_tool_file_ops::move_file::MoveFileTool));
    registry.register(Box::new(loopal_tool_file_ops::delete::DeleteTool));
    registry.register(Box::new(loopal_tool_file_ops::copy::CopyFileTool));
}

/// Register thread-goal tools. Only the root agent owns a `goal_session`,
/// so sub-agents must NOT call this — registering tools they cannot back
/// would expose dead options to the model.
pub fn register_goal_tools(registry: &ToolRegistry) {
    registry.register(Box::new(loopal_tool_goal::GetGoalTool));
    registry.register(Box::new(loopal_tool_goal::CreateGoalTool));
    registry.register(Box::new(loopal_tool_goal::UpdateGoalTool));
}
