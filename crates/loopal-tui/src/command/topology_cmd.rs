//! /topology command — toggles the graphical agent tree overlay.

use async_trait::async_trait;

use crate::app::App;
use crate::command::{CommandEffect, CommandHandler};

pub struct TopologyCmd;

#[async_trait]
impl CommandHandler for TopologyCmd {
    fn name(&self) -> &str {
        "/topology"
    }
    fn description(&self) -> &str {
        "Toggle agent topology graph overlay"
    }
    fn has_arg(&self) -> bool {
        false
    }
    async fn execute(&self, app: &mut App, _arg: Option<&str>) -> CommandEffect {
        app.show_topology = !app.show_topology;
        CommandEffect::Done
    }
}
