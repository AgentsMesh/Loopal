//! Per-agent conversation state for TUI rendering.
//!
//! `AgentConversation` captures the full renderable state of a single agent's
//! conversation — messages, streaming buffers, tool calls, pending interactions,
//! tokens, and timing. Both root ("main") and sub-agents use the same type.

use std::time::{Duration, Instant};

use crate::thinking_display::format_thinking_content;
use crate::types::{PendingPermission, PendingQuestion, SessionMessage};

/// Per-agent conversation state — everything needed to render one agent's chat view.
#[derive(Debug, Default)]
pub struct AgentConversation {
    pub messages: Vec<SessionMessage>,
    pub streaming_text: String,
    pub streaming_thinking: String,
    pub thinking_active: bool,
    pub pending_permission: Option<PendingPermission>,
    pub pending_question: Option<PendingQuestion>,
    /// Transient retry error banner.
    pub retry_banner: Option<String>,
    pub turn_count: u32,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub context_window: u32,
    pub cache_creation_tokens: u32,
    pub cache_read_tokens: u32,
    pub thinking_tokens: u32,
    // Turn timer
    turn_start: Option<Instant>,
    last_turn_duration: Duration,
    /// Last time the agent emitted a "still working" signal (Stream, tool
    /// event, Running, …). Used by the TUI to bridge short idle windows
    /// between `AwaitingInput` and the next `Running` event, so the status
    /// spinner does not flicker off between turns.
    last_active_at: Option<Instant>,
}

impl AgentConversation {
    /// Total token count for context usage display.
    pub fn token_count(&self) -> u32 {
        self.input_tokens + self.output_tokens + self.cache_creation_tokens + self.cache_read_tokens
    }

    /// Current turn working duration.
    pub fn turn_elapsed(&self) -> Duration {
        match self.turn_start {
            Some(start) => start.elapsed(),
            None => self.last_turn_duration,
        }
    }

    /// Mark the start of a new turn (agent begins working).
    pub fn begin_turn(&mut self) {
        if self.turn_start.is_none() {
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

    /// Flush buffered streaming text and thinking into SessionMessages.
    pub fn flush_streaming(&mut self) {
        if !self.streaming_thinking.is_empty() {
            let thinking = std::mem::take(&mut self.streaming_thinking);
            let token_est = thinking.len() as u32 / 4;
            let content = format_thinking_content(&thinking, token_est);
            self.messages.push(SessionMessage {
                role: "thinking".to_string(),
                content,
                tool_calls: Vec::new(),
                image_count: 0,
                skill_info: None,
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
                tool_calls: Vec::new(),
                image_count: 0,
                skill_info: None,
            });
        }
    }
}
