use super::{InputAction, SlashCommandAction};

/// Attempt to parse and execute a manually typed slash command (without autocomplete).
pub(super) fn try_execute_slash_command(input: &str) -> Option<InputAction> {
    if !input.starts_with('/') {
        return None;
    }

    let parts: Vec<&str> = input.splitn(2, ' ').collect();
    let cmd_name = parts[0];
    let arg = parts.get(1).map(|s| s.trim()).filter(|s| !s.is_empty());

    // Only dispatch if it's a known command
    let all = crate::command::all_commands();
    let known = all.iter().any(|c| c.name == cmd_name);
    if !known {
        return None;
    }

    Some(dispatch_command(cmd_name, arg))
}

/// Convert a resolved command name + optional arg into the correct InputAction.
pub(super) fn dispatch_command(name: &str, _arg: Option<&str>) -> InputAction {
    match name {
        "/plan" => InputAction::ModeSwitch("plan".to_string()),
        "/act" => InputAction::ModeSwitch("act".to_string()),
        "/clear" => InputAction::SlashCommand(SlashCommandAction::Clear),
        "/compact" => InputAction::SlashCommand(SlashCommandAction::Compact),
        "/status" => InputAction::SlashCommand(SlashCommandAction::Status),
        "/sessions" => InputAction::SlashCommand(SlashCommandAction::Sessions),
        "/help" => InputAction::SlashCommand(SlashCommandAction::Help),
        "/model" => InputAction::SlashCommand(SlashCommandAction::ModelPicker),
        "/exit" => InputAction::Quit,
        _ => InputAction::None,
    }
}
