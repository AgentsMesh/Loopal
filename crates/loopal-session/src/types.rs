//! Display types shared between session controller and UI consumers.
//!
//! These types represent the presentation-layer view of agent messages,
//! tool calls, and pending permission requests.

use std::time::Instant;

use loopal_protocol::Question;

/// A message to display in the chat view.
#[derive(Debug, Clone)]
pub struct DisplayMessage {
    pub role: String,
    pub content: String,
    pub tool_calls: Vec<DisplayToolCall>,
    /// Number of images attached to this message (0 for text-only).
    pub image_count: usize,
}

/// Lifecycle status of a single tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ToolCallStatus {
    Pending = 0,
    Running = 1,
    Success = 2,
    Error = 3,
}

impl ToolCallStatus {
    /// Still executing (Pending or Running).
    pub fn is_active(self) -> bool {
        matches!(self, Self::Pending | Self::Running)
    }
    /// Finished (Success or Error).
    pub fn is_done(self) -> bool {
        matches!(self, Self::Success | Self::Error)
    }
}

/// A tool call to display in the chat view.
#[derive(Debug, Clone)]
pub struct DisplayToolCall {
    pub id: String,
    pub name: String,
    pub status: ToolCallStatus,
    /// Call description, e.g. "Read(/tmp/foo.rs)". Not overwritten by ToolResult.
    pub summary: String,
    /// Full tool output (None while pending).
    /// Session layer applies loose storage-protection truncation (200 lines / 10 KB).
    pub result: Option<String>,
    pub tool_input: Option<serde_json::Value>,
    pub batch_id: Option<String>,
    pub started_at: Option<Instant>,
    pub duration_ms: Option<u64>,
    pub progress_tail: Option<String>,
    /// Structured metadata from tool (e.g. `{"bytes_written": 1234}`).
    pub metadata: Option<serde_json::Value>,
}

/// A pending tool permission request awaiting user approval.
#[derive(Debug, Clone)]
pub struct PendingPermission {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
    /// IPC request ID from Hub relay (needed to respond via HubClient).
    /// None in local/test mode (uses channel instead).
    pub relay_request_id: Option<i64>,
}

/// A pending user question dialog awaiting selection.
#[derive(Debug, Clone)]
pub struct PendingQuestion {
    pub id: String,
    pub questions: Vec<Question>,
    pub selected: Vec<Vec<bool>>,
    pub current_question: usize,
    pub cursor: usize,
    /// IPC request ID from Hub relay (needed to respond via HubClient).
    pub relay_request_id: Option<i64>,
}

impl PendingQuestion {
    pub fn new(id: String, questions: Vec<Question>) -> Self {
        let selected: Vec<Vec<bool>> = questions
            .iter()
            .map(|q| vec![false; q.options.len()])
            .collect();
        Self {
            id,
            questions,
            selected,
            current_question: 0,
            cursor: 0,
            relay_request_id: None,
        }
    }

    pub fn cursor_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn cursor_down(&mut self) {
        let q = &self.questions[self.current_question];
        if self.cursor + 1 < q.options.len() {
            self.cursor += 1;
        }
    }

    pub fn toggle(&mut self) {
        let sel = &mut self.selected[self.current_question];
        sel[self.cursor] = !sel[self.cursor];
    }

    /// Collect selected labels for current question.
    pub fn get_answers(&self) -> Vec<String> {
        let q = &self.questions[self.current_question];
        let sel = &self.selected[self.current_question];
        q.options
            .iter()
            .zip(sel.iter())
            .filter(|(_, s)| **s)
            .map(|(opt, _)| opt.label.clone())
            .collect()
    }
}
