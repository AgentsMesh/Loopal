//! Display types shared between session controller and UI consumers.
//!
//! These types represent the presentation-layer view of agent messages,
//! tool calls, and pending permission requests.

use loopal_protocol::Question;

/// A message to display in the chat view.
#[derive(Debug, Clone)]
pub struct DisplayMessage {
    pub role: String,
    pub content: String,
    pub tool_calls: Vec<DisplayToolCall>,
}

/// A tool call to display in the chat view.
#[derive(Debug, Clone)]
pub struct DisplayToolCall {
    pub name: String,
    /// "pending", "success", "error"
    pub status: String,
    /// Call description, e.g. "Read(/tmp/foo.rs)". Not overwritten by ToolResult.
    pub summary: String,
    /// Full tool output (None while pending).
    /// Session layer applies loose storage-protection truncation (200 lines / 10 KB).
    pub result: Option<String>,
}

/// A pending tool permission request awaiting user approval.
#[derive(Debug, Clone)]
pub struct PendingPermission {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

/// A pending user question dialog awaiting selection.
#[derive(Debug, Clone)]
pub struct PendingQuestion {
    pub id: String,
    pub questions: Vec<Question>,
    pub selected: Vec<Vec<bool>>,
    pub current_question: usize,
    pub cursor: usize,
}

impl PendingQuestion {
    pub fn new(id: String, questions: Vec<Question>) -> Self {
        let selected: Vec<Vec<bool>> = questions
            .iter()
            .map(|q| vec![false; q.options.len()])
            .collect();
        Self { id, questions, selected, current_question: 0, cursor: 0 }
    }

    pub fn cursor_up(&mut self) {
        if self.cursor > 0 { self.cursor -= 1; }
    }

    pub fn cursor_down(&mut self) {
        let q = &self.questions[self.current_question];
        if self.cursor + 1 < q.options.len() { self.cursor += 1; }
    }

    pub fn toggle(&mut self) {
        let sel = &mut self.selected[self.current_question];
        sel[self.cursor] = !sel[self.cursor];
    }

    /// Collect selected labels for current question.
    pub fn get_answers(&self) -> Vec<String> {
        let q = &self.questions[self.current_question];
        let sel = &self.selected[self.current_question];
        q.options.iter().zip(sel.iter())
            .filter(|(_, s)| **s)
            .map(|(opt, _)| opt.label.clone())
            .collect()
    }
}
