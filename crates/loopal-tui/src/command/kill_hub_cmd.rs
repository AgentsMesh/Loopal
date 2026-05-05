use async_trait::async_trait;

use super::{CommandEffect, CommandHandler};
use crate::app::App;

pub struct KillHubCmd;

#[async_trait]
impl CommandHandler for KillHubCmd {
    fn name(&self) -> &str {
        "/kill-hub"
    }
    fn description(&self) -> &str {
        "Shut down Hub and all agents, then exit TUI"
    }
    async fn execute(&self, app: &mut App, _arg: Option<&str>) -> CommandEffect {
        app.shutdown_initiated = true;
        app.session.shutdown_hub().await;
        CommandEffect::Quit
    }
}
