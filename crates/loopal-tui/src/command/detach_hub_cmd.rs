use async_trait::async_trait;

use super::{CommandEffect, CommandHandler};
use crate::app::App;

pub struct DetachHubCmd;

#[async_trait]
impl CommandHandler for DetachHubCmd {
    fn name(&self) -> &str {
        "/detach-hub"
    }
    fn description(&self) -> &str {
        "Detach TUI from Hub (Hub & agents keep running)"
    }
    async fn execute(&self, _app: &mut App, _arg: Option<&str>) -> CommandEffect {
        CommandEffect::Detach
    }
}
