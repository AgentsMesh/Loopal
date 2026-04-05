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
    /// A session was selected to resume (full session ID).
    SessionSelected(String),
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
    /// Enter Panel focus mode (Tab from Input when panels have content)
    EnterPanel,
    /// Exit Panel focus mode back to Input
    ExitPanel,
    /// Tab within panel zone — switch panel or cycle item
    PanelTab,
    /// Navigate up within the active panel (with scroll)
    PanelUp,
    /// Navigate down within the active panel (with scroll)
    PanelDown,
    /// Enter the focused agent's conversation view (drill in, Agents panel only)
    EnterAgentView,
    /// Return to root/parent view (drill out)
    ExitAgentView,
    /// Terminate the focused agent (Agents panel only)
    TerminateFocusedAgent,
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
