use crate::command::CommandEntry;

use super::InputAction;

/// Attempt to parse and execute a slash command (from manual input, not autocomplete).
/// Returns `Some(InputAction)` if the input matched a known command.
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
    commands.iter().find(|e| e.name == cmd_name)?;

    // Unified dispatch — registry handles both built-in and skill commands
    Some(InputAction::RunCommand(
        cmd_name.to_string(),
        arg.map(|s| s.to_string()),
    ))
}
