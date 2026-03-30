//! `/rewind` command — opens the rewind picker sub-page.

use async_trait::async_trait;

use super::{CommandEffect, CommandHandler};
use crate::app::{App, RewindPickerState, RewindTurnItem, SubPage};

pub struct RewindCmd;

#[async_trait]
impl CommandHandler for RewindCmd {
    fn name(&self) -> &str {
        "/rewind"
    }
    fn description(&self) -> &str {
        "Rewind to a previous turn"
    }
    async fn execute(&self, app: &mut App, _arg: Option<&str>) -> CommandEffect {
        open_rewind_picker(app);
        CommandEffect::Done
    }
}

fn open_rewind_picker(app: &mut App) {
    let state = app.session.lock();
    let conv = state.active_conversation();
    if !conv.agent_idle {
        drop(state);
        app.session
            .push_system_message("Cannot rewind while the agent is busy.".into());
        return;
    }
    let turns: Vec<RewindTurnItem> = conv
        .messages
        .iter()
        .enumerate()
        .filter(|(_, m)| m.role == "user")
        .enumerate()
        .map(|(turn_idx, (_, msg))| {
            let preview = if msg.content.chars().count() > 60 {
                let truncated: String = msg.content.chars().take(60).collect();
                format!("{truncated}...")
            } else {
                msg.content.clone()
            };
            RewindTurnItem {
                turn_index: turn_idx,
                preview,
            }
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    drop(state);

    if turns.is_empty() {
        app.session
            .push_system_message("No turns to rewind to.".into());
        return;
    }

    app.sub_page = Some(SubPage::RewindPicker(RewindPickerState {
        turns,
        selected: 0,
    }));
}
