mod types;

pub use types::*;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use loopal_protocol::{ImageAttachment, UserContent};
use loopal_session::SessionController;

use crate::command::CommandRegistry;
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
    /// Images attached to the current input (pending submit).
    pub pending_images: Vec<ImageAttachment>,
    /// Active autocomplete menu, if any.
    pub autocomplete: Option<AutocompleteState>,
    /// Active sub-page (full-screen picker), if any.
    pub sub_page: Option<SubPage>,
    /// Unified command registry (built-in + skills). Skills refreshed on demand.
    pub command_registry: CommandRegistry,
    /// Working directory, used to reload skills on demand.
    pub cwd: PathBuf,
    /// Timestamp of the last ESC press (for double-ESC rewind trigger).
    pub last_esc_time: Option<Instant>,
    /// Vertical scroll offset when input exceeds max visible height.
    pub input_scroll: usize,
    /// Paste placeholder → original content map for large paste folding.
    pub paste_map: HashMap<String, String>,
    /// Whether the content area overflows the viewport (set by render pass).
    /// Used by input handler to decide Up/Down = scroll vs history navigation.
    pub content_overflows: bool,
    /// Whether the topology overlay is visible (toggled by /topology).
    pub show_topology: bool,

    // === Session Controller (observable + interactive) ===
    pub session: SessionController,

    // === Render optimization ===
    pub line_cache: LineCache,
}

impl App {
    pub fn new(session: SessionController, cwd: PathBuf) -> Self {
        let mut registry = CommandRegistry::new();
        // Load initial skills from config
        let config = loopal_config::load_config(&cwd);
        let skills: Vec<_> = match config {
            Ok(c) => c.skills.into_values().map(|e| e.skill).collect(),
            Err(_) => Vec::new(),
        };
        registry.reload_skills(&skills);

        Self {
            exiting: false,
            input: String::new(),
            input_cursor: 0,
            scroll_offset: 0,
            input_history: Vec::new(),
            history_index: None,
            pending_images: Vec::new(),
            autocomplete: None,
            sub_page: None,
            command_registry: registry,
            cwd,
            last_esc_time: None,
            input_scroll: 0,
            paste_map: HashMap::new(),
            content_overflows: false,
            show_topology: false,
            session,
            line_cache: LineCache::new(),
        }
    }

    /// Submit the current input with any pending images, returning `UserContent`.
    /// Does NOT add to messages or history — the session controller handles that.
    /// Paste placeholders are expanded to full content before submission.
    pub fn submit_input(&mut self) -> Option<UserContent> {
        let has_images = !self.pending_images.is_empty();
        if self.input.trim().is_empty() && !has_images {
            return None;
        }
        let mut text = std::mem::take(&mut self.input);
        let images = std::mem::take(&mut self.pending_images);
        // Expand paste placeholders to full content
        if !self.paste_map.is_empty() {
            text = crate::input::paste::expand_paste_placeholders(&text, &self.paste_map);
            self.paste_map.clear();
        }
        self.input_cursor = 0;
        self.input_scroll = 0;
        self.scroll_offset = 0;
        Some(UserContent { text, images })
    }

    /// Pop the last Inbox message back into the input field for editing.
    /// Returns true if a message was popped.
    pub fn pop_inbox_to_input(&mut self) -> bool {
        if let Some(content) = self.session.pop_inbox_to_edit() {
            self.input = content.text;
            self.pending_images = content.images;
            self.input_cursor = self.input.len();
            true
        } else {
            false
        }
    }

    /// Attach an image to the current pending input.
    pub fn attach_image(&mut self, attachment: ImageAttachment) {
        self.pending_images.push(attachment);
    }

    /// Number of images attached to the current input.
    pub fn pending_image_count(&self) -> usize {
        self.pending_images.len()
    }

    /// Reload skills from disk and rebuild the command registry.
    pub fn refresh_commands(&mut self) {
        let config = loopal_config::load_config(&self.cwd);
        let skills: Vec<_> = match config {
            Ok(c) => c.skills.into_values().map(|e| e.skill).collect(),
            Err(_) => Vec::new(),
        };
        self.command_registry.reload_skills(&skills);
    }
}
