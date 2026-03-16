/// Action triggered by a slash command from the autocomplete menu.
pub enum SlashCommandAction {
    Clear,
    Compact,
    Status,
    Sessions,
    /// Show help. `None` = all commands; `Some(name)` = specific skill detail.
    Help(Option<String>),
    /// Open the model picker sub-page.
    ModelPicker,
    /// A model was selected from the picker.
    ModelSelected(String),
}

/// Action resulting from input handling.
pub enum InputAction {
    /// No action needed
    None,
    /// User submitted a message (legacy — slash commands still use Submit path)
    Submit(String),
    /// User message queued into Inbox (not sent directly to agent)
    InboxPush(String),
    /// User wants to quit
    Quit,
    /// User approved tool use
    ToolApprove,
    /// User denied tool use
    ToolDeny,
    /// User wants to switch mode
    ModeSwitch(String),
    /// User executed a slash command
    SlashCommand(SlashCommandAction),
}
