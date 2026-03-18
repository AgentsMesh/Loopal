mod types;

pub use types::*;

use std::path::PathBuf;

use loopagent_session::SessionController;

use crate::command::{CommandEntry, merge_commands};
use crate::views::progress::LineCache;

/// Main application state — UI-only fields + session controller handle.
pub struct App {
    // === UI-only state ===
    pub exiting: bool,
    pub input: String,
    pub input_cursor: usize,
    pub scroll_offset: u16,
    pub input_history: Vec<String>,
    pub history_index: Option<usize>,
    /// Active autocomplete menu, if any.
    pub autocomplete: Option<AutocompleteState>,
    /// Active sub-page (full-screen picker), if any.
    pub sub_page: Option<SubPage>,
    /// Merged command entries (built-in + skills). Refreshed on demand.
    pub commands: Vec<CommandEntry>,
    /// Working directory, used to reload skills on demand.
    pub cwd: PathBuf,

    // === Session Controller (observable + interactive) ===
    pub session: SessionController,

    // === Render optimization ===
    pub line_cache: LineCache,
}

impl App {
    pub fn new(
        session: SessionController,
        commands: Vec<CommandEntry>,
        cwd: PathBuf,
    ) -> Self {
        Self {
            exiting: false,
            input: String::new(),
            input_cursor: 0,
            scroll_offset: 0,
            input_history: Vec::new(),
            history_index: None,
            autocomplete: None,
            sub_page: None,
            commands,
            cwd,
            session,
            line_cache: LineCache::new(),
        }
    }

    /// Submit the current input, returning the text.
    /// Does NOT add to messages or history — the session controller handles that.
    pub fn submit_input(&mut self) -> Option<String> {
        if self.input.trim().is_empty() {
            return None;
        }
        let text = std::mem::take(&mut self.input);
        self.input_cursor = 0;
        self.scroll_offset = 0;
        Some(text)
    }

    /// Pop the last Inbox message back into the input field for editing.
    /// Returns true if a message was popped.
    pub fn pop_inbox_to_input(&mut self) -> bool {
        if let Some(text) = self.session.pop_inbox_to_edit() {
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
