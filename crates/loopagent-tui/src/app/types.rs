// Re-export display types from session crate
pub use loopagent_session::{DisplayMessage, DisplayToolCall, PendingPermission};

/// Autocomplete menu state for slash commands.
pub struct AutocompleteState {
    /// Indices into the `App.commands` vec.
    pub matches: Vec<usize>,
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
}
