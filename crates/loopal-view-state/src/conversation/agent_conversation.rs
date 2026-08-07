use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::pending_question::PendingQuestion;
use super::thinking_display::format_thinking_content;
use super::types::{PendingPermission, PendingPlanApproval, SessionMessage};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentConversation {
    pub messages: Vec<SessionMessage>,
    #[serde(default)]
    pub history_truncated: bool,
    pub streaming_text: String,
    pub streaming_thinking: String,
    pub thinking_active: bool,
    pub pending_permission: Option<PendingPermission>,
    pub pending_question: Option<PendingQuestion>,
    pub pending_plan_approval: Option<PendingPlanApproval>,
    pub retry_banner: Option<String>,
    pub compact_banner: Option<String>,
    pub turn_count: u32,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub context_window: u32,
    pub cache_creation_tokens: u32,
    pub cache_read_tokens: u32,
    pub thinking_tokens: u32,
    #[serde(skip)]
    turn_start: Option<Instant>,
    #[serde(skip)]
    last_turn_duration: Duration,
    /// Bridge gap between `AwaitingInput` and next `Running` so spinner doesn't flicker.
    #[serde(skip)]
    last_active_at: Option<Instant>,
    #[serde(skip)]
    pub(super) flushed_thinking_index: Option<usize>,
}

impl AgentConversation {
    /// Total token count for context usage display.
    pub fn token_count(&self) -> u32 {
        self.input_tokens + self.output_tokens + self.cache_creation_tokens + self.cache_read_tokens
    }

    /// Current turn working duration.
    ///
    /// This is business-lifecycle time, not an animation clock. Its
    /// `Instant` anchor is process-local and intentionally does not cross a
    /// serialized view snapshot; TUI animations must use their own monotonic
    /// clock instead.
    pub fn turn_elapsed(&self) -> Duration {
        match self.turn_start {
            Some(start) => start.elapsed(),
            None => self.last_turn_duration,
        }
    }

    /// Mark the start of a new turn (agent begins working).
    pub fn begin_turn(&mut self) {
        if self.turn_start.is_none() {
            self.flushed_thinking_index = None;
            self.turn_start = Some(Instant::now());
        }
    }

    /// Record that the agent just emitted an activity signal.
    ///
    /// The TUI uses this timestamp to keep the status spinner/timer live
    /// during the brief gap between `AwaitingInput` (end of turn N) and
    /// `Running` (start of turn N+1), which can be several milliseconds
    /// because those events hop across agent-proc → hub → TUI IPC.
    pub fn mark_active(&mut self) {
        self.last_active_at = Some(Instant::now());
    }

    /// Whether the agent emitted any activity within the last `grace` window.
    pub fn is_recently_active(&self, grace: Duration) -> bool {
        self.last_active_at.is_some_and(|t| t.elapsed() < grace)
    }

    /// Mark the end of a turn (agent became idle).
    pub fn end_turn(&mut self) {
        if let Some(start) = self.turn_start.take() {
            self.last_turn_duration = start.elapsed();
        }
    }

    /// Reset the turn timer (e.g., after /clear).
    pub fn reset_timer(&mut self) {
        self.turn_start = None;
        self.last_turn_duration = Duration::ZERO;
        self.last_active_at = None;
    }

    /// Wipe the conversation back to its post-construction state. Used by
    /// the `Cleared` event mutator. Pending permission/question dialogs
    /// are dropped along with the rows that referenced them — a multi-
    /// client race that produced a pending dialog elsewhere must not leave
    /// a zombie popup pointing at a tool_call_id whose message has just
    /// been wiped.
    pub fn clear_all(&mut self, context_window: u32) {
        self.clear_history();
        self.context_window = context_window;
    }

    /// Variant of `clear_all` for callers that must NOT touch the budget
    /// indicator — the follow-up `TokenUsage` event will reset
    /// `context_window`, so resetting it here would briefly show 0 then
    /// flicker back. Used by the `SessionResumed` mutator.
    pub fn clear_history(&mut self) {
        self.messages.clear();
        self.history_truncated = false;
        self.streaming_text.clear();
        self.streaming_thinking.clear();
        self.thinking_active = false;
        self.retry_banner = None;
        self.compact_banner = None;
        self.turn_count = 0;
        self.input_tokens = 0;
        self.output_tokens = 0;
        self.cache_creation_tokens = 0;
        self.cache_read_tokens = 0;
        self.thinking_tokens = 0;
        self.pending_permission = None;
        self.pending_question = None;
        self.pending_plan_approval = None;
        self.flushed_thinking_index = None;
        self.reset_timer();
    }

    pub fn replace_history(&mut self, messages: Vec<SessionMessage>, truncated: bool) {
        self.messages = messages;
        self.history_truncated = truncated;
        self.streaming_text.clear();
        self.streaming_thinking.clear();
        self.thinking_active = false;
        self.retry_banner = None;
        self.compact_banner = None;
        self.pending_permission = None;
        self.pending_question = None;
        self.pending_plan_approval = None;
        self.flushed_thinking_index = None;
    }

    /// Flush buffered streaming text and thinking into SessionMessages.
    pub fn flush_streaming(&mut self) {
        if !self.streaming_thinking.is_empty() {
            let thinking = std::mem::take(&mut self.streaming_thinking);
            let token_est = thinking.len() as u32 / 4;
            let content = format_thinking_content(&thinking, token_est);
            self.flushed_thinking_index = Some(self.messages.len());
            self.messages.push(SessionMessage {
                role: "thinking".to_string(),
                content,
                ..Default::default()
            });
            self.thinking_active = false;
        }
        if !self.streaming_text.is_empty() {
            let text = std::mem::take(&mut self.streaming_text);
            if let Some(last) = self.messages.last_mut()
                && last.role == "assistant"
                && last.tool_calls.is_empty()
            {
                last.content.push_str(&text);
                return;
            }
            self.messages.push(SessionMessage {
                role: "assistant".to_string(),
                content: text,
                ..Default::default()
            });
        }
    }
}
