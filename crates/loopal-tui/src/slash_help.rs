use crate::app::App;

pub(crate) fn show_help(app: &mut App, skill_name: Option<&str>) {
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
        "Available commands:\n{}\n\nShortcuts:\n  Shift+Tab       Toggle Plan/Act mode\n  Ctrl+C          Clear input / interrupt agent\n  Ctrl+D          Quit\n  PageUp/Down     Scroll chat\n  Up/Down         Input history",
        lines.join("\n"),
    )
}
