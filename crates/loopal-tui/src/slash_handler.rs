use crate::app::{
    App, PickerItem, PickerState, SubPage, ThinkingOption,
    RewindPickerState, RewindTurnItem,
};
use crate::input::SlashCommandAction;
use crate::slash_help::show_help;

/// Handle a slash command action. All interaction goes through `app.session`.
pub(crate) async fn handle_slash_command(
    app: &mut App,
    cmd: SlashCommandAction,
) {
    match cmd {
        SlashCommandAction::Clear => {
            app.pending_images.clear();
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
        SlashCommandAction::ModelAndThinkingSelected { model, thinking_json } => {
            app.session.switch_model(model).await;
            app.session.switch_thinking(thinking_json).await;
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
        SlashCommandAction::RewindPicker => {
            open_rewind_picker(app);
        }
        SlashCommandAction::RewindConfirmed(turn_index) => {
            app.session.rewind(turn_index).await;
        }
    }
}

fn open_rewind_picker(app: &mut App) {
    let state = app.session.lock();
    if !state.agent_idle {
        drop(state);
        app.session.push_system_message(
            "Cannot rewind while the agent is busy.".into(),
        );
        return;
    }
    let turns: Vec<RewindTurnItem> = state
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
            RewindTurnItem { turn_index: turn_idx, preview }
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    drop(state);

    if turns.is_empty() {
        app.session.push_system_message("No turns to rewind to.".into());
        return;
    }

    app.sub_page = Some(SubPage::RewindPicker(RewindPickerState {
        turns,
        selected: 0,
    }));
}

fn open_model_picker(app: &mut App) {
    let state = app.session.lock();
    let current_model = state.model.clone();
    let current_thinking = state.thinking_config.clone();
    drop(state);

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

    let (thinking_options, thinking_selected) = build_thinking_options(&current_thinking);
    app.sub_page = Some(SubPage::ModelPicker(PickerState {
        title: "Switch Model".to_string(),
        items,
        filter: String::new(),
        filter_cursor: 0,
        selected: 0,
        thinking_options,
        thinking_selected,
    }));
}

/// Build the 5 thinking options and determine which one is currently selected.
fn build_thinking_options(current: &str) -> (Vec<ThinkingOption>, usize) {
    let options = vec![
        ThinkingOption { label: "Auto", value: r#"{"type":"auto"}"#.to_string() },
        ThinkingOption { label: "Low", value: r#"{"type":"effort","level":"low"}"#.to_string() },
        ThinkingOption { label: "Medium", value: r#"{"type":"effort","level":"medium"}"#.to_string() },
        ThinkingOption { label: "High", value: r#"{"type":"effort","level":"high"}"#.to_string() },
        ThinkingOption { label: "Disabled", value: r#"{"type":"disabled"}"#.to_string() },
    ];
    let idx = match current {
        "low" => 1,
        "medium" => 2,
        "high" => 3,
        "disabled" => 4,
        _ => 0, // "auto" or unknown
    };
    (options, idx)
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
        app.cwd.display(),
    );
    drop(state);
    app.session.push_system_message(status);
}
