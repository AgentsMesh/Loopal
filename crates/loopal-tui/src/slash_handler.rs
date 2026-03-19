use crate::app::{App, PickerItem, PickerState, SubPage};
use crate::input::SlashCommandAction;

/// Handle a slash command action. All interaction goes through `app.session`.
pub(crate) async fn handle_slash_command(
    app: &mut App,
    cmd: SlashCommandAction,
) {
    match cmd {
        SlashCommandAction::Clear => {
            app.session.clear().await;
        }
        SlashCommandAction::Compact => {
            app.session.compact().await;
        }
        SlashCommandAction::ModelPicker => {
            open_model_picker(app);
        }
        SlashCommandAction::ModelSelected(name) => {
            app.session.switch_model(name).await;
        }
        SlashCommandAction::Status => {
            show_status(app);
        }
        SlashCommandAction::Sessions => {
            app.session.push_system_message(
                "Session listing is not yet available in TUI.".to_string(),
            );
        }
        SlashCommandAction::Help(name) => {
            show_help(app, name.as_deref());
        }
    }
}

fn open_model_picker(app: &mut App) {
    let current_model = app.session.lock().model.clone();
    let models = loopal_provider::list_all_models();
    let items: Vec<PickerItem> = models
        .into_iter()
        .map(|m| {
            let marker = if m.id == current_model {
                " (current)"
            } else {
                ""
            };
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
    let state = app.session.lock();
    let token_count = state.token_count();
    let context_info = if state.context_window > 0 {
        format!(
            "{}k/{}k",
            token_count / 1000,
            state.context_window / 1000
        )
    } else {
        format!("{} tokens", token_count)
    };
    let status = format!(
        "Mode: {} | Model: {} | Context: {} | Turns: {} | CWD: {}",
        state.mode.to_uppercase(),
        state.model,
        context_info,
        state.turn_count,
        std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "unknown".to_string()),
    );
    drop(state);
    app.session.push_system_message(status);
}

fn show_help(app: &mut App, skill_name: Option<&str>) {
    let content = if let Some(name) = skill_name {
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
        build_full_help(&app.commands)
    };
    app.session.push_system_message(content);
}

fn build_full_help(commands: &[crate::command::CommandEntry]) -> String {
    let lines: Vec<String> = commands
        .iter()
        .map(|entry| {
            let arg_hint = if entry.has_arg { " <arg>" } else { "" };
            let tag = if entry.skill_body.is_some() {
                " (skill)"
            } else {
                ""
            };
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
