mod mcp_page;
mod status_page;
mod types;

pub use mcp_page::*;
pub use status_page::*;
pub use types::*;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use loopal_protocol::{BgTaskDetail, BgTaskSnapshot, ImageAttachment, UserContent};
use loopal_session::SessionController;

use crate::command::CommandRegistry;
use crate::views::progress::ContentScroll;

/// Main application state — UI-only fields + session controller handle.
pub struct App {
    // === UI-only state ===
    pub exiting: bool,
    pub input: String,
    pub input_cursor: usize,
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
    /// Whether the topology overlay is visible (toggled by /topology).
    pub show_topology: bool,
    /// Agent panel cursor — Tab cycles through agents. Purely TUI concept.
    pub focused_agent: Option<String>,
    /// Background tasks panel cursor.
    pub focused_bg_task: Option<String>,
    /// Which UI region owns keyboard focus.
    pub focus_mode: FocusMode,
    /// Scroll offset for the agent panel (index of first visible agent).
    pub agent_panel_offset: usize,

    /// Cached background task snapshots (synced from session state each frame).
    pub bg_snapshots: Vec<BgTaskSnapshot>,
    /// Full bg task details including output (for log viewer).
    pub bg_task_details: Vec<BgTaskDetail>,

    // === Session Controller (observable + interactive) ===
    pub session: SessionController,

    // === Content area scroll + render state ===
    pub content_scroll: ContentScroll,
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
            show_topology: true,
            focused_agent: None,
            focused_bg_task: None,
            focus_mode: FocusMode::default(),
            agent_panel_offset: 0,
            bg_snapshots: Vec::new(),
            bg_task_details: Vec::new(),
            session,
            content_scroll: ContentScroll::new(),
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
        self.content_scroll.to_bottom();
        Some(UserContent {
            text,
            images,
            skill_info: None,
        })
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
