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

/// Active sub-page overlay that replaces the main chat area.
pub enum SubPage {
    /// Model picker — user selects from known models.
    ModelPicker(PickerState),
    /// Rewind picker — user selects a turn to rewind to.
    RewindPicker(RewindPickerState),
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
