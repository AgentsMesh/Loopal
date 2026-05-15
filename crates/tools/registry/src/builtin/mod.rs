use std::sync::Arc;

use loopal_tool_api::TypedBridge;
use loopal_tool_background::BackgroundTaskStore;

use crate::registry::ToolRegistry;

pub fn register_all(registry: &ToolRegistry, bg_store: Arc<BackgroundTaskStore>) {
    registry.register(Box::new(TypedBridge::<
        _,
        loopal_tool_apply_patch::ApplyPatchParams,
    >::new(loopal_tool_apply_patch::ApplyPatchTool)));
    registry.register(Box::new(
        TypedBridge::<_, loopal_tool_read::ReadParams>::new(loopal_tool_read::ReadTool),
    ));
    registry.register(Box::new(TypedBridge::<
        _,
        loopal_tool_read_pdf::ReadPdfParams,
    >::new(loopal_tool_read_pdf::ReadPdfTool)));
    registry.register(Box::new(TypedBridge::<
        _,
        loopal_tool_read_html::ReadHtmlParams,
    >::new(loopal_tool_read_html::ReadHtmlTool)));
    registry.register(Box::new(
        TypedBridge::<_, loopal_tool_write::WriteParams>::new(loopal_tool_write::WriteTool),
    ));
    registry.register(Box::new(
        TypedBridge::<_, loopal_tool_edit::EditParams>::new(loopal_tool_edit::EditTool),
    ));
    registry.register(Box::new(TypedBridge::<
        _,
        loopal_tool_multi_edit::MultiEditParams,
    >::new(loopal_tool_multi_edit::MultiEditTool)));
    registry.register(Box::new(
        TypedBridge::<_, loopal_tool_glob::GlobParams>::new(loopal_tool_glob::GlobTool),
    ));
    registry.register(Box::new(
        TypedBridge::<_, loopal_tool_grep::GrepParams>::new(loopal_tool_grep::GrepTool),
    ));
    registry.register(Box::new(
        TypedBridge::<_, loopal_tool_bash::BashParams>::new(loopal_tool_bash::BashTool::new(
            bg_store.clone(),
        )),
    ));
    registry.register(Box::new(TypedBridge::<
        _,
        loopal_tool_bash_process::BashProcessParams,
    >::new(
        loopal_tool_bash_process::BashProcessTool::new(bg_store),
    )));
    registry.register(Box::new(TypedBridge::<_, loopal_tool_ls::LsParams>::new(
        loopal_tool_ls::LsTool,
    )));
    registry.register(Box::new(
        TypedBridge::<_, loopal_tool_fetch::FetchParams>::new(loopal_tool_fetch::FetchTool),
    ));
    // reason: WebSearch is a client-side placeholder that the Anthropic/Google/
    // OpenAI providers detect by name and replace with their server-side
    // `web_search` declaration; without registering it here the providers
    // never inject the server tool and the model loses search capability.
    registry.register(Box::new(TypedBridge::<
        _,
        loopal_tool_web_search::WebSearchParams,
    >::new(loopal_tool_web_search::WebSearchTool)));
    registry.register(Box::new(TypedBridge::<
        _,
        loopal_tool_ask_user::AskUserParams,
    >::new(loopal_tool_ask_user::AskUserTool)));
    registry.register(Box::new(TypedBridge::<
        _,
        loopal_tool_plan_mode::EnterPlanModeParams,
    >::new(
        loopal_tool_plan_mode::EnterPlanModeTool
    )));
    registry.register(Box::new(TypedBridge::<
        _,
        loopal_tool_plan_mode::ExitPlanModeParams,
    >::new(loopal_tool_plan_mode::ExitPlanModeTool)));
    registry.register(Box::new(TypedBridge::<
        _,
        loopal_tool_file_ops::MoveFileParams,
    >::new(loopal_tool_file_ops::MoveFileTool)));
    registry.register(Box::new(
        TypedBridge::<_, loopal_tool_file_ops::DeleteParams>::new(loopal_tool_file_ops::DeleteTool),
    ));
    registry.register(Box::new(TypedBridge::<
        _,
        loopal_tool_file_ops::CopyFileParams,
    >::new(loopal_tool_file_ops::CopyFileTool)));
}

pub fn register_goal_tools(registry: &ToolRegistry) {
    registry.register(Box::new(
        TypedBridge::<_, loopal_tool_goal::GetGoalParams>::new(loopal_tool_goal::GetGoalTool),
    ));
    registry.register(Box::new(
        TypedBridge::<_, loopal_tool_goal::CreateGoalParams>::new(loopal_tool_goal::CreateGoalTool),
    ));
    registry.register(Box::new(
        TypedBridge::<_, loopal_tool_goal::UpdateGoalParams>::new(loopal_tool_goal::UpdateGoalTool),
    ));
}
