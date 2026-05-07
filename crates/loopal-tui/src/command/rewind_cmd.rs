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
    fn has_arg(&self) -> bool {
        false
    }
    async fn execute(&self, app: &mut App, _arg: Option<&str>) -> CommandEffect {
        match open_rewind_picker(app) {
            Ok(()) => CommandEffect::Done,
            Err(msg) => CommandEffect::Reply(msg),
        }
    }
}

fn open_rewind_picker(app: &mut App) -> Result<(), String> {
    if !app.is_active_agent_idle() {
        return Err("Cannot rewind while the agent is busy.".into());
    }
    let turns: Vec<RewindTurnItem> = app.with_active_conversation(|conv| {
        conv.messages
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
            .collect()
    });

    if turns.is_empty() {
        return Err("No turns to rewind to.".into());
    }

    app.sub_page = Some(SubPage::RewindPicker(RewindPickerState {
        turns,
        selected: 0,
    }));
    Ok(())
}
