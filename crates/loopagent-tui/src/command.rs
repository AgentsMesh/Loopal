/// A slash command definition for the autocomplete menu.
#[derive(Debug, Clone)]
pub struct SlashCommand {
    /// Command name including the leading `/`, e.g. "/plan"
    pub name: &'static str,
    /// Short description shown in the menu
    pub description: &'static str,
    /// Whether the command expects an argument after the name
    pub has_arg: bool,
}

/// All available slash commands.
pub fn all_commands() -> &'static [SlashCommand] {
    static COMMANDS: &[SlashCommand] = &[
        SlashCommand {
            name: "/plan",
            description: "Switch to Plan mode",
            has_arg: false,
        },
        SlashCommand {
            name: "/act",
            description: "Switch to Act mode",
            has_arg: false,
        },
        SlashCommand {
            name: "/clear",
            description: "Clear conversation history",
            has_arg: false,
        },
        SlashCommand {
            name: "/compact",
            description: "Compact old messages",
            has_arg: false,
        },
        SlashCommand {
            name: "/model",
            description: "Switch model",
            has_arg: false,
        },
        SlashCommand {
            name: "/status",
            description: "Show current status",
            has_arg: false,
        },
        SlashCommand {
            name: "/sessions",
            description: "List session history",
            has_arg: false,
        },
        SlashCommand {
            name: "/help",
            description: "Show commands and shortcuts",
            has_arg: false,
        },
        SlashCommand {
            name: "/exit",
            description: "Exit the application",
            has_arg: false,
        },
    ];
    COMMANDS
}

/// Filter commands by prefix. If `input` is just "/", return all commands.
/// Otherwise match commands whose name starts with `input`.
pub fn filter_commands(input: &str) -> Vec<&'static SlashCommand> {
    let lower = input.to_ascii_lowercase();
    all_commands()
        .iter()
        .filter(|cmd| input == "/" || cmd.name.starts_with(&lower))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slash_only_returns_all() {
        let result = filter_commands("/");
        assert_eq!(result.len(), all_commands().len());
    }

    #[test]
    fn test_filter_prefix_a() {
        let result = filter_commands("/a");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "/act");
    }

    #[test]
    fn test_filter_prefix_c() {
        let result = filter_commands("/c");
        let names: Vec<&str> = result.iter().map(|c| c.name).collect();
        assert!(names.contains(&"/clear"));
        assert!(names.contains(&"/compact"));
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn test_filter_no_match() {
        let result = filter_commands("/xyz");
        assert!(result.is_empty());
    }

    #[test]
    fn test_filter_exact_match() {
        let result = filter_commands("/help");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "/help");
    }

    #[test]
    fn test_model_opens_picker() {
        let cmd = all_commands().iter().find(|c| c.name == "/model").unwrap();
        // /model now opens a picker sub-page, no inline argument
        assert!(!cmd.has_arg);
    }
}
