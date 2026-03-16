mod event_handler;
mod types;

pub use types::*;

use std::collections::VecDeque;
use std::path::PathBuf;

use crate::command::{CommandEntry, merge_commands};
use loopagent_types::event::AgentEvent;
use tokio::sync::mpsc;

/// Main application state
pub struct App {
    pub state: AppState,
    pub messages: Vec<DisplayMessage>,
    pub input: String,
    pub input_cursor: usize,
    pub scroll_offset: u16,
    pub model: String,
    pub mode: String,
    pub token_count: u32,
    pub context_window: u32,
    pub turn_count: u32,
    pub streaming_text: String,
    pub event_tx: mpsc::Sender<AgentEvent>,
    pub input_history: Vec<String>,
    pub history_index: Option<usize>,
    /// Active autocomplete menu, if any.
    pub autocomplete: Option<AutocompleteState>,
    /// Active sub-page (full-screen picker), if any.
    pub sub_page: Option<SubPage>,
    /// Inbox queue: user messages waiting to be forwarded to the agent.
    pub inbox: VecDeque<String>,
    /// Whether the agent is idle (awaiting input).
    pub agent_idle: bool,
    /// Merged command entries (built-in + skills). Refreshed on demand.
    pub commands: Vec<CommandEntry>,
    /// Working directory, used to reload skills on demand.
    pub cwd: PathBuf,
}

impl App {
    pub fn new(
        model: String,
        mode: String,
        event_tx: mpsc::Sender<AgentEvent>,
        commands: Vec<CommandEntry>,
        cwd: PathBuf,
    ) -> Self {
        Self {
            state: AppState::Running,
            messages: Vec::new(),
            input: String::new(),
            input_cursor: 0,
            scroll_offset: 0,
            model,
            mode,
            token_count: 0,
            context_window: 0,
            turn_count: 0,
            streaming_text: String::new(),
            event_tx,
            input_history: Vec::new(),
            history_index: None,
            autocomplete: None,
            sub_page: None,
            inbox: VecDeque::new(),
            agent_idle: false,
            commands,
            cwd,
        }
    }

    /// Submit the current input, returning the text.
    /// Does NOT add to messages or history — the Inbox mechanism handles that.
    pub fn submit_input(&mut self) -> Option<String> {
        if self.input.trim().is_empty() {
            return None;
        }
        let text = std::mem::take(&mut self.input);
        self.input_cursor = 0;
        self.scroll_offset = 0;
        Some(text)
    }

    /// Push a message into the Inbox queue and save to input history.
    pub fn push_to_inbox(&mut self, text: String) {
        self.input_history.push(text.clone());
        self.history_index = None;
        self.inbox.push_back(text);
        self.scroll_offset = 0;
    }

    /// If agent is idle and Inbox is non-empty, pop the front message,
    /// add it to displayed messages, and return it for channel forwarding.
    pub fn try_forward_inbox(&mut self) -> Option<String> {
        if self.agent_idle
            && let Some(text) = self.inbox.pop_front()
        {
            self.agent_idle = false;
            self.messages.push(DisplayMessage {
                role: "user".to_string(),
                content: text.clone(),
                tool_calls: Vec::new(),
            });
            return Some(text);
        }
        None
    }

    /// Pop the last Inbox message back into the input field for editing.
    /// Returns true if a message was popped.
    pub fn pop_inbox_to_input(&mut self) -> bool {
        if let Some(text) = self.inbox.pop_back() {
            self.input = text;
            self.input_cursor = self.input.len();
            true
        } else {
            false
        }
    }

    /// Reload skills from disk and rebuild the merged command list.
    pub fn refresh_commands(&mut self) {
        let skills = loopagent_config::load_skills(&self.cwd);
        self.commands = merge_commands(&skills);
    }
}
