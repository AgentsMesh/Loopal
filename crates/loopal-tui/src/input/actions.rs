use loopal_protocol::UserContent;

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
    /// Model + thinking selected from the enhanced model picker.
    ModelAndThinkingSelected { model: String, thinking_json: String },
    /// Open the rewind turn picker.
    RewindPicker,
    /// A turn was selected for rewind (turn_index from oldest = 0).
    RewindConfirmed(usize),
}

/// Action resulting from input handling.
pub enum InputAction {
    /// No action needed
    None,
    /// User message queued into Inbox for forwarding to agent
    InboxPush(UserContent),
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
    /// Cycle focus to the next agent in the agents map
    FocusNextAgent,
    /// Clear agent focus (return to root view)
    UnfocusAgent,
    // --- Question dialog actions ---
    /// Navigate up in question options
    QuestionUp,
    /// Navigate down in question options
    QuestionDown,
    /// Confirm current selection (submit answer)
    QuestionConfirm,
    /// Toggle option selection (multi-select)
    QuestionToggle,
    /// Cancel question dialog
    QuestionCancel,
    /// User pressed Ctrl+V — caller should spawn async clipboard read
    PasteRequested,
}
