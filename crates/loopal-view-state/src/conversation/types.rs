use std::time::Instant;

use loopal_protocol::{MessageSource, Question, SkillInvocation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
    pub tool_calls: Vec<SessionToolCall>,
    pub image_count: usize,
    pub skill_info: Option<SkillInvocation>,
    pub inbox: Option<InboxOrigin>,
    /// Stable id assigned by the routing layer. Carried so consumers
    /// can dedup or correlate to subsequent `InboxConsumed` events.
    /// `None` for system-originated rows that don't have an envelope id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    /// True for messages that exist only on the UI side (welcome banner,
    /// system notices, resumed history). Hub state never sets this; UI
    /// preserves these across `view/snapshot` resync.
    #[serde(default, skip_serializing_if = "is_false")]
    pub ui_local: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxOrigin {
    pub message_id: String,
    pub source: MessageSource,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ToolCallStatus {
    Pending = 0,
    Running = 1,
    Success = 2,
    Error = 3,
}

impl ToolCallStatus {
    pub fn is_active(self) -> bool {
        matches!(self, Self::Pending | Self::Running)
    }
    pub fn is_done(self) -> bool {
        matches!(self, Self::Success | Self::Error)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionToolCall {
    pub id: String,
    pub name: String,
    pub status: ToolCallStatus,
    pub summary: String,
    pub result: Option<String>,
    pub tool_input: Option<serde_json::Value>,
    pub batch_id: Option<String>,
    #[serde(skip)]
    pub started_at: Option<Instant>,
    pub duration_ms: Option<u64>,
    pub progress_tail: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingPermission {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        Self {
            id,
            questions,
            selected,
            current_question: 0,
            cursor: 0,
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
