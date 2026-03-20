use crate::command::CommandEntry;

use super::{InputAction, SlashCommandAction};

/// Attempt to parse and execute a manually typed slash command (without autocomplete).
pub(super) fn try_execute_slash_command(
    input: &str,
    commands: &[CommandEntry],
) -> Option<InputAction> {
    if !input.starts_with('/') {
        return None;
    }

    let parts: Vec<&str> = input.splitn(2, ' ').collect();
    let cmd_name = parts[0];
    let arg = parts.get(1).map(|s| s.trim()).filter(|s| !s.is_empty());

    // Find matching command entry
    let entry = commands.iter().find(|e| e.name == cmd_name)?;

    if let Some(ref body) = entry.skill_body {
        // Skill: expand and push to inbox
        let expanded = expand_skill(body, arg.unwrap_or(""));
        Some(InputAction::InboxPush(expanded))
    } else {
        // Built-in: dispatch
        Some(dispatch_command(cmd_name, arg))
    }
}

/// Expand a skill template by replacing `$ARGUMENTS` with the given args.
pub(super) fn expand_skill(body: &str, args: &str) -> String {
    let trimmed = args.trim();
    if body.contains("$ARGUMENTS") {
        body.replace("$ARGUMENTS", trimmed)
    } else if !trimmed.is_empty() {
        format!("{body}\n{trimmed}")
    } else {
        body.to_string()
    }
}

/// Convert a resolved command name + optional arg into the correct InputAction.
pub(super) fn dispatch_command(name: &str, arg: Option<&str>) -> InputAction {
    match name {
        "/plan" => InputAction::ModeSwitch("plan".to_string()),
        "/act" => InputAction::ModeSwitch("act".to_string()),
        "/clear" => InputAction::SlashCommand(SlashCommandAction::Clear),
        "/compact" => InputAction::SlashCommand(SlashCommandAction::Compact),
        "/status" => InputAction::SlashCommand(SlashCommandAction::Status),
        "/sessions" => InputAction::SlashCommand(SlashCommandAction::Sessions),
        "/help" => InputAction::SlashCommand(SlashCommandAction::Help(
            arg.map(|s| s.to_string()),
        )),
        "/model" => InputAction::SlashCommand(SlashCommandAction::ModelPicker),
        "/rewind" => InputAction::SlashCommand(SlashCommandAction::RewindPicker),
        "/exit" => InputAction::Quit,
        _ => InputAction::None,
    }
}
