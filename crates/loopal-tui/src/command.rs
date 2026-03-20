use std::collections::HashSet;

use loopal_config::Skill;

/// A static slash command definition (internal).
#[derive(Debug, Clone)]
struct SlashCommand {
    name: &'static str,
    description: &'static str,
    has_arg: bool,
}

/// Unified command entry for both built-in commands and skills.
#[derive(Debug, Clone)]
pub struct CommandEntry {
    pub name: String,
    pub description: String,
    pub has_arg: bool,
    /// Prompt template body. `None` means built-in command.
    pub skill_body: Option<String>,
}

/// All built-in slash commands.
fn all_commands() -> &'static [SlashCommand] {
    static COMMANDS: &[SlashCommand] = &[
        SlashCommand { name: "/plan", description: "Switch to Plan mode", has_arg: false },
        SlashCommand { name: "/act", description: "Switch to Act mode", has_arg: false },
        SlashCommand { name: "/clear", description: "Clear conversation history", has_arg: false },
        SlashCommand { name: "/compact", description: "Compact old messages", has_arg: false },
        SlashCommand { name: "/model", description: "Switch model", has_arg: false },
        SlashCommand { name: "/rewind", description: "Rewind to a previous turn", has_arg: false },
        SlashCommand { name: "/status", description: "Show current status", has_arg: false },
        SlashCommand {
            name: "/sessions",
            description: "List session history",
            has_arg: false,
        },
        SlashCommand { name: "/help", description: "Show commands and shortcuts", has_arg: false },
        SlashCommand { name: "/exit", description: "Exit the application", has_arg: false },
    ];
    COMMANDS
}

/// Convert built-in commands to owned entries.
pub fn builtin_entries() -> Vec<CommandEntry> {
    all_commands()
        .iter()
        .map(|cmd| CommandEntry {
            name: cmd.name.to_string(),
            description: cmd.description.to_string(),
            has_arg: cmd.has_arg,
            skill_body: None,
        })
        .collect()
}

/// Merge built-in commands with skills. Built-in commands take priority over same-named skills.
pub fn merge_commands(skills: &[Skill]) -> Vec<CommandEntry> {
    let mut entries = builtin_entries();
    let builtin_names: HashSet<String> =
        entries.iter().map(|e| e.name.clone()).collect();
    for skill in skills {
        if !builtin_names.contains(&skill.name) {
            entries.push(CommandEntry {
                name: skill.name.clone(),
                description: skill.description.clone(),
                has_arg: skill.has_arg,
                skill_body: Some(skill.body.clone()),
            });
        }
    }
    entries
}

/// Filter entries by prefix, returning indices of matching entries.
pub fn filter_entries(entries: &[CommandEntry], input: &str) -> Vec<usize> {
    let lower = input.to_ascii_lowercase();
    entries
        .iter()
        .enumerate()
        .filter(|(_, e)| input == "/" || e.name.starts_with(&lower))
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_entries_count() {
        let entries = builtin_entries();
        assert_eq!(entries.len(), all_commands().len());
    }

    #[test]
    fn test_filter_entries_all() {
        let entries = builtin_entries();
        let result = filter_entries(&entries, "/");
        assert_eq!(result.len(), entries.len());
    }

    #[test]
    fn test_filter_entries_prefix() {
        let entries = builtin_entries();
        let result = filter_entries(&entries, "/c");
        let names: Vec<&str> = result.iter().map(|&i| entries[i].name.as_str()).collect();
        assert!(names.contains(&"/clear"));
        assert!(names.contains(&"/compact"));
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn test_merge_commands_builtin_priority() {
        let skills = vec![Skill {
            name: "/help".to_string(),
            description: "Custom help".to_string(),
            has_arg: false,
            body: "custom".to_string(),
        }];
        let entries = merge_commands(&skills);
        let help = entries.iter().find(|e| e.name == "/help").unwrap();
        // Built-in wins: skill_body should be None
        assert!(help.skill_body.is_none());
    }

    #[test]
    fn test_merge_commands_adds_skill() {
        let skills = vec![Skill {
            name: "/commit".to_string(),
            description: "Generate commit".to_string(),
            has_arg: true,
            body: "Review changes. $ARGUMENTS".to_string(),
        }];
        let entries = merge_commands(&skills);
        let commit = entries.iter().find(|e| e.name == "/commit").unwrap();
        assert!(commit.skill_body.is_some());
        assert!(commit.has_arg);
    }
}
