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
            open_model_picker(app);
        }
        SlashCommandAction::ModelSelected(name) => {
            app.model = name.clone();
            app.messages.push(DisplayMessage {
                role: "system".to_string(),
                content: format!("Switched model to: {name}"),
                tool_calls: Vec::new(),
            });
            let _ = user_input_tx.send(UserCommand::ModelSwitch(name)).await;
        }
        SlashCommandAction::Status => {
            show_status(app);
        }
        SlashCommandAction::Sessions => {
            app.messages.push(DisplayMessage {
                role: "system".to_string(),
                content: "Session listing is not yet available in TUI.".to_string(),
                tool_calls: Vec::new(),
            });
        }
        SlashCommandAction::Help(name) => {
            show_help(app, name.as_deref());
        }
    }
}

fn open_model_picker(app: &mut App) {
    let models = loopagent_provider::list_all_models();
    let current = app.model.clone();
    let items: Vec<PickerItem> = models
        .into_iter()
        .map(|m| {
            let marker = if m.id == current { " (current)" } else { "" };
            PickerItem {
                label: m.display_name.clone(),
                description: format!(
                    "{}  ctx:{}k  out:{}k{marker}",
                    m.id,
                    m.context_window / 1000,
                    m.max_output_tokens / 1000,
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

fn show_status(app: &mut App) {
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

fn show_help(app: &mut App, skill_name: Option<&str>) {
    let content = if let Some(name) = skill_name {
        // Show detail for a specific skill
        let lookup = if name.starts_with('/') {
            name.to_string()
        } else {
            format!("/{name}")
        };
        match app.commands.iter().find(|e| e.name == lookup) {
            Some(entry) if entry.skill_body.is_some() => {
                let body = entry.skill_body.as_deref().unwrap_or("");
                format!("Skill: {}\n{}\n\n{body}", entry.name, entry.description)
            }
            Some(entry) => {
                format!("{}: {}", entry.name, entry.description)
            }
            None => format!("Unknown command: {lookup}"),
        }
    } else {
        // Show all commands + skills
        build_full_help(&app.commands)
    };
    app.messages.push(DisplayMessage {
        role: "system".to_string(),
        content,
        tool_calls: Vec::new(),
    });
}

fn build_full_help(commands: &[crate::command::CommandEntry]) -> String {
    let lines: Vec<String> = commands
        .iter()
        .map(|entry| {
            let arg_hint = if entry.has_arg { " <arg>" } else { "" };
            let tag = if entry.skill_body.is_some() { " (skill)" } else { "" };
            format!(
                "  {:<16} {}{}",
                format!("{}{arg_hint}", entry.name),
                entry.description,
                tag,
            )
        })
        .collect();
    format!(
        "Available commands:\n{}\n\nShortcuts:\n  Shift+Tab       Toggle Plan/Act mode\n  Ctrl+C/D        Quit\n  PageUp/Down     Scroll chat\n  Up/Down         Input history",
        lines.join("\n"),
    )
}
