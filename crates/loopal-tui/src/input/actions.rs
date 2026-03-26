use loopal_protocol::UserContent;

/// Result from a sub-page picker confirmation.
pub enum SubPageResult {
    /// A model was selected from the picker.
    ModelSelected(String),
    /// Model + thinking selected from the enhanced model picker.
    ModelAndThinkingSelected {
        model: String,
        thinking_json: String,
    },
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
    /// Interrupt the agent's current work (ESC while busy)
    Interrupt,
    /// User wants to switch mode (from Shift+Tab shortcut)
    ModeSwitch(String),
    /// Execute a registered slash command by name
    RunCommand(String, Option<String>),
    /// Sub-page picker confirmed a result
    SubPageConfirm(SubPageResult),
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
