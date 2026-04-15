//! `/mcp` command — opens the MCP server status sub-page.

use async_trait::async_trait;
use loopal_protocol::ControlCommand;

use super::{CommandEffect, CommandHandler};
use crate::app::{App, McpPageState, SubPage};

pub struct McpCmd;

#[async_trait]
impl CommandHandler for McpCmd {
    fn name(&self) -> &str {
        "/mcp"
    }
    fn description(&self) -> &str {
        "Show MCP server status"
    }
    async fn execute(&self, app: &mut App, _arg: Option<&str>) -> CommandEffect {
        open_mcp_page(app).await;
        CommandEffect::Done
    }
}

async fn open_mcp_page(app: &mut App) {
    let servers = app.session.lock().mcp_status.clone();
    app.sub_page = Some(SubPage::McpPage(McpPageState::new(servers)));
    let target = app.session.lock().active_view.clone();
    app.session
        .send_control(target, ControlCommand::QueryMcpStatus)
        .await;
}
