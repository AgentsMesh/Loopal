//! `/help` command — displays command list or skill details.

use async_trait::async_trait;

use super::{CommandEffect, CommandEntry, CommandHandler};
use crate::app::App;

pub struct HelpCmd;

#[async_trait]
impl CommandHandler for HelpCmd {
    fn name(&self) -> &str {
        "/help"
    }
    fn description(&self) -> &str {
        "Show commands and shortcuts"
    }
    fn has_arg(&self) -> bool {
        true
    }
    async fn execute(&self, app: &mut App, arg: Option<&str>) -> CommandEffect {
        let entries = app.command_registry.entries();
        let content = if let Some(name) = arg {
            show_single(name, app)
        } else {
            build_full_help(&entries)
        };
        app.session.push_system_message(content);
        CommandEffect::Done
    }
}

/// Show help for a single command/skill.
fn show_single(name: &str, app: &App) -> String {
    let lookup = if name.starts_with('/') {
        name.to_string()
    } else {
        format!("/{name}")
    };
    match app.command_registry.find(&lookup) {
        Some(handler) if handler.is_skill() => {
            let body = handler.skill_body().unwrap_or("");
            format!(
                "Skill: {}\n{}\n\n{body}",
                handler.name(),
                handler.description()
            )
        }
        Some(handler) => {
            format!("{}: {}", handler.name(), handler.description())
        }
        None => format!("Unknown command: {lookup}"),
    }
}

fn build_full_help(commands: &[CommandEntry]) -> String {
    let lines: Vec<String> = commands
        .iter()
        .map(|entry| {
            let arg_hint = if entry.has_arg { " <arg>" } else { "" };
            let tag = if entry.is_skill { " (skill)" } else { "" };
            format!(
                "  {:<16} {}{}",
                format!("{}{arg_hint}", entry.name),
                entry.description,
                tag,
            )
        })
        .collect();
    format!(
        "Available commands:\n{}\n\nShortcuts:\n  Shift+Tab       Toggle Plan/Act mode\n  Ctrl+C          Clear input / interrupt agent\n  Ctrl+D          Quit\n  PageUp/Down     Scroll chat\n  Up/Down         Input history",
        lines.join("\n"),
    )
}
