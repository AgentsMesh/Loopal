mod dispatch;
mod mcp_page;
mod skills_page;
mod status_page;
mod types;
mod view_access;
mod view_seed;

pub use mcp_page::*;
pub use skills_page::*;
pub use status_page::*;
pub use types::*;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use indexmap::IndexMap;
use loopal_protocol::{BgTaskDetail, ImageAttachment, UserContent};
use loopal_session::SessionController;

use crate::command::CommandRegistry;
use crate::panel_provider::PanelRegistry;
use crate::panel_state::PanelSectionState;
use crate::view_client::ViewClient;
use crate::views::progress::ContentScroll;

/// Main application state — UI-only fields + session controller handle.
pub struct App {
    pub exiting: bool,
    pub input: String,
    pub input_cursor: usize,
    pub input_history: Vec<String>,
    pub history_index: Option<usize>,
    pub pending_images: Vec<ImageAttachment>,
    pub autocomplete: Option<AutocompleteState>,
    pub sub_page: Option<SubPage>,
    pub command_registry: CommandRegistry,
    pub cwd: PathBuf,
    pub last_esc_time: Option<Instant>,
    pub input_scroll: usize,
    pub paste_map: HashMap<String, String>,
    pub show_topology: bool,
    pub panel_sections: Vec<PanelSectionState>,
    pub panel_registry: PanelRegistry,
    pub focus_mode: FocusMode,

    /// Per-agent ViewState replicas — SSOT for per-agent observable,
    /// conversation, tasks, crons, bg_tasks, and topology fields.
    /// Created on first sighting (or up-front for "main"); never
    /// removed mid-session so completed sub-agents stay visible.
    pub view_clients: IndexMap<String, ViewClient>,
    /// Bg-task transcripts for the log sub-page. Synced from
    /// `view_client.state().bg_tasks` each frame; the secondary cache
    /// lets the log viewer survive past a sweep of finished tasks.
    pub bg_task_details: Vec<BgTaskDetail>,

    pub session: SessionController,
    pub content_scroll: ContentScroll,
}

impl App {
    pub fn new(session: SessionController, cwd: PathBuf) -> Self {
        let mut registry = CommandRegistry::new();
        let config = loopal_config::load_config(&cwd);
        let skills: Vec<_> = match config {
            Ok(c) => c.skills.into_values().map(|e| e.skill).collect(),
            Err(_) => Vec::new(),
        };
        registry.reload_skills(&skills);

        let mut panel_registry = PanelRegistry::new();
        crate::providers::register_all(&mut panel_registry);
        let panel_sections = panel_registry
            .providers()
            .iter()
            .map(|p| PanelSectionState::new(p.kind()))
            .collect();

        let mut view_clients = IndexMap::new();
        view_clients.insert("main".to_string(), ViewClient::empty("main"));

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
            panel_sections,
            panel_registry,
            focus_mode: FocusMode::default(),
            view_clients,
            bg_task_details: Vec::new(),
            session,
            content_scroll: ContentScroll::new(),
        }
    }

    /// Submit the current input plus pending images. Returns `None`
    /// when both are empty. Paste placeholders are expanded to the
    /// stored content so the agent receives the full text.
    pub fn submit_input(&mut self) -> Option<UserContent> {
        let has_images = !self.pending_images.is_empty();
        if self.input.trim().is_empty() && !has_images {
            return None;
        }
        let mut text = std::mem::take(&mut self.input);
        let images = std::mem::take(&mut self.pending_images);
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

    pub fn attach_image(&mut self, attachment: ImageAttachment) {
        self.pending_images.push(attachment);
    }

    pub fn pending_image_count(&self) -> usize {
        self.pending_images.len()
    }

    pub fn refresh_commands(&mut self) {
        let config = loopal_config::load_config(&self.cwd);
        let skills: Vec<_> = match config {
            Ok(c) => c.skills.into_values().map(|e| e.skill).collect(),
            Err(_) => Vec::new(),
        };
        self.command_registry.reload_skills(&skills);
    }

    pub fn section(&self, kind: PanelKind) -> &PanelSectionState {
        self.panel_sections.iter().find(|s| s.kind == kind).unwrap()
    }

    pub fn section_mut(&mut self, kind: PanelKind) -> &mut PanelSectionState {
        self.panel_sections
            .iter_mut()
            .find(|s| s.kind == kind)
            .unwrap()
    }
}
