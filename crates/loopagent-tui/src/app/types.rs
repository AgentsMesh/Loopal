use crate::command::SlashCommand;

/// Application state machine
#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Running,
    ToolConfirm {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    Exiting,
}

/// A tool call to display in the chat view
#[derive(Debug, Clone)]
pub struct DisplayToolCall {
    pub name: String,
    /// "pending", "success", "error"
    pub status: String,
    pub summary: String,
}

/// A message to display in the chat view
#[derive(Debug, Clone)]
pub struct DisplayMessage {
    pub role: String,
    pub content: String,
    pub tool_calls: Vec<DisplayToolCall>,
}

/// Autocomplete menu state for slash commands.
pub struct AutocompleteState {
    pub matches: Vec<&'static SlashCommand>,
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
