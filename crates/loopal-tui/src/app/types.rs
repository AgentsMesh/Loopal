// Re-export display types from session crate
pub use loopal_session::{PendingPermission, SessionMessage, SessionToolCall};

use crate::command::CommandEntry;

/// Autocomplete menu state for slash commands.
///
/// Stores a snapshot of matched entries (not indices) so that the state
/// remains consistent even if the registry is reloaded between keystrokes.
pub struct AutocompleteState {
    /// Matched command entries (snapshot taken when autocomplete was built).
    pub matches: Vec<CommandEntry>,
    pub selected: usize,
}

/// A single item in a picker list.
#[derive(Debug, Clone)]
pub struct PickerItem {
    /// Primary label (e.g., model id)
    pub label: String,
    /// Secondary description shown to the right
    pub description: String,
    /// The value to use when this item is selected
    pub value: String,
}

/// A single thinking effort option for ←→ cycling in the model picker.
#[derive(Debug, Clone)]
pub struct ThinkingOption {
    /// Display label: "Auto", "Low", "Medium", "High", "Disabled"
    pub label: &'static str,
    /// Serialized ThinkingConfig JSON value.
    pub value: String,
}

/// Generic picker (sub-page) state.
pub struct PickerState {
    /// Title shown at the top of the picker
    pub title: String,
    /// All available items (unfiltered)
    pub items: Vec<PickerItem>,
    /// Current filter text
    pub filter: String,
    /// Cursor position within the filter text
    pub filter_cursor: usize,
    /// Index of the selected item in the *filtered* list
    pub selected: usize,
    /// Thinking effort options for ←→ cycling. Empty if not applicable.
    pub thinking_options: Vec<ThinkingOption>,
    /// Currently selected thinking option index.
    pub thinking_selected: usize,
}

impl PickerState {
    /// Return items matching the current filter.
    pub fn filtered_items(&self) -> Vec<&PickerItem> {
        if self.filter.is_empty() {
            self.items.iter().collect()
        } else {
            let lower = self.filter.to_ascii_lowercase();
            self.items
                .iter()
                .filter(|item| {
                    item.label.to_ascii_lowercase().contains(&lower)
                        || item.description.to_ascii_lowercase().contains(&lower)
                })
                .collect()
        }
    }

    /// Clamp selected index to filtered results length.
    pub fn clamp_selected(&mut self) {
        let count = self.filtered_items().len();
        if count == 0 {
            self.selected = 0;
        } else if self.selected >= count {
            self.selected = count - 1;
        }
    }
}

use super::StatusPageState;

use crate::app::McpPageState;
use crate::app::SkillsPageState;

/// Active sub-page overlay that replaces the main chat area.
pub enum SubPage {
    /// Model picker — user selects from known models.
    ModelPicker(PickerState),
    /// Rewind picker — user selects a turn to rewind to.
    RewindPicker(RewindPickerState),
    /// Session picker — user selects a session to resume.
    SessionPicker(PickerState),
    /// Status dashboard — tabbed view of session info, config, and usage.
    StatusPage(StatusPageState),
    /// MCP server status page — list of MCP servers with connection state.
    McpPage(McpPageState),
    /// Skills page — list of loaded skills with source info.
    SkillsPage(SkillsPageState),
    /// Background task log viewer — full output of a single bg task.
    BgTaskLog(BgTaskLogState),
}

/// State for the background task log viewer sub-page.
pub struct BgTaskLogState {
    /// ID of the task being viewed.
    pub task_id: String,
    /// Scroll offset (lines from top; 0 = top).
    pub scroll_offset: usize,
    /// Auto-scroll to bottom when new output arrives.
    pub auto_follow: bool,
    /// Previous line count — detects output growth for auto-follow.
    pub prev_line_count: usize,
}

/// Which sub-panel within the panel zone is focused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelKind {
    /// Agent status list.
    Agents,
    /// Structured tasks progress.
    Tasks,
    /// Background shell tasks.
    BgTasks,
}

/// Which UI region currently owns keyboard input.
///
/// Orthogonal to panel section focus — mode says "are we navigating a panel"
/// while the `PanelSectionState.focused` fields say "which item is highlighted".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusMode {
    /// Default: typing goes to input field; Up/Down = multiline → scroll → history.
    #[default]
    Input,
    /// Panel navigation: Up/Down = navigate items; Enter = drill in (agents only).
    Panel(PanelKind),
}

/// State for the rewind turn picker.
pub struct RewindPickerState {
    /// Available turns (most recent first for display).
    pub turns: Vec<RewindTurnItem>,
    /// Currently selected index within `turns`.
    pub selected: usize,
}

/// A single turn entry in the rewind picker.
pub struct RewindTurnItem {
    /// Turn index in the runtime (0 = oldest).
    pub turn_index: usize,
    /// User message preview (truncated).
    pub preview: String,
}
