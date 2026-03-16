use tokio::sync::mpsc;

use crate::app::{App, DisplayMessage, PickerItem, PickerState, SubPage};
use crate::input::SlashCommandAction;
use loopagent_types::command::UserCommand;

/// Handle a slash command action, dispatching to either local TUI handling
/// or forwarding to the agent loop via `user_input_tx`.
pub(crate) async fn handle_slash_command(
    app: &mut App,
    cmd: SlashCommandAction,
    user_input_tx: &mpsc::Sender<UserCommand>,
) {
    match cmd {
        SlashCommandAction::Clear => {
            app.messages.clear();
            app.inbox.clear();
            app.turn_count = 0;
            app.token_count = 0;
            let _ = user_input_tx.send(UserCommand::Clear).await;
        }
        SlashCommandAction::Compact => {
            let _ = user_input_tx.send(UserCommand::Compact).await;
        }
        SlashCommandAction::ModelPicker => {
            let models = loopagent_provider::list_all_models();
            let current = app.model.clone();
            let items: Vec<PickerItem> = models
                .into_iter()
                .map(|m| {
                    let marker = if m.id == current { " (current)" } else { "" };
                    PickerItem {
                        label: m.display_name.clone(),
                        description: format!(
                            "{}  ctx:{}k  out:{}k{}",
                            m.id,
                            m.context_window / 1000,
                            m.max_output_tokens / 1000,
                            marker,
                        ),
                        value: m.id,
                    }
                })
                .collect();
            app.sub_page = Some(SubPage::ModelPicker(PickerState {
                title: "Switch Model".to_string(),
                items,
                filter: String::new(),
                filter_cursor: 0,
                selected: 0,
            }));
        }
        SlashCommandAction::ModelSelected(name) => {
            app.model = name.clone();
            app.messages.push(DisplayMessage {
                role: "system".to_string(),
                content: format!("Switched model to: {}", name),
                tool_calls: Vec::new(),
            });
            let _ = user_input_tx.send(UserCommand::ModelSwitch(name)).await;
        }
        SlashCommandAction::Status => {
            let context_info = if app.context_window > 0 {
                format!("{}k/{}k", app.token_count / 1000, app.context_window / 1000)
            } else {
                format!("{} tokens", app.token_count)
            };
            let status = format!(
                "Mode: {} | Model: {} | Context: {} | Turns: {} | CWD: {}",
                app.mode.to_uppercase(),
                app.model,
                context_info,
                app.turn_count,
                std::env::current_dir()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| "unknown".to_string()),
            );
            app.messages.push(DisplayMessage {
                role: "system".to_string(),
                content: status,
                tool_calls: Vec::new(),
            });
        }
        SlashCommandAction::Sessions => {
            // Session listing requires SessionManager access which is in the runtime.
            // Display a placeholder; a full implementation would query via a dedicated channel.
            app.messages.push(DisplayMessage {
                role: "system".to_string(),
                content: "Session listing is not yet available in TUI.".to_string(),
                tool_calls: Vec::new(),
            });
        }
        SlashCommandAction::Help => {
            let help_text = crate::command::all_commands()
                .iter()
                .map(|cmd| {
                    let arg_hint = if cmd.has_arg { " <arg>" } else { "" };
                    format!("  {:<16} {}", format!("{}{}", cmd.name, arg_hint), cmd.description)
                })
                .collect::<Vec<_>>()
                .join("\n");

            let full_help = format!(
                "Available commands:\n{}\n\nShortcuts:\n  Shift+Tab       Toggle Plan/Act mode\n  Ctrl+C/D        Quit\n  PageUp/Down     Scroll chat\n  Up/Down         Input history",
                help_text
            );
            app.messages.push(DisplayMessage {
                role: "system".to_string(),
                content: full_help,
                tool_calls: Vec::new(),
            });
        }
    }
}
