use std::fmt;
use std::sync::Arc;

use crate::backend::Backend;
use crate::goal_session::GoalSession;
use crate::memory_channel::MemoryChannel;
use crate::output_tail::OutputTail;
use crate::provider_resolver::{FetchRefinerPolicy, OneShotChatService};

#[non_exhaustive]
pub struct ToolContext {
    pub backend: Arc<dyn Backend>,
    pub session_id: String,
    pub shared: Option<Arc<dyn std::any::Any + Send + Sync>>,
    pub memory_channel: Option<Arc<dyn MemoryChannel>>,
    pub output_tail: Option<Arc<OutputTail>>,
    pub one_shot_chat: Option<Arc<dyn OneShotChatService>>,
    pub fetch_refiner_policy: Option<Arc<dyn FetchRefinerPolicy>>,
    pub goal_session: Option<Arc<dyn GoalSession>>,
}

impl ToolContext {
    pub fn new(backend: Arc<dyn Backend>, session_id: impl Into<String>) -> Self {
        Self {
            backend,
            session_id: session_id.into(),
            shared: None,
            memory_channel: None,
            output_tail: None,
            one_shot_chat: None,
            fetch_refiner_policy: None,
            goal_session: None,
        }
    }

    pub fn with_shared(mut self, s: Arc<dyn std::any::Any + Send + Sync>) -> Self {
        self.shared = Some(s);
        self
    }

    pub fn with_shared_opt(mut self, s: Option<Arc<dyn std::any::Any + Send + Sync>>) -> Self {
        self.shared = s;
        self
    }

    pub fn with_memory_channel(mut self, m: Arc<dyn MemoryChannel>) -> Self {
        self.memory_channel = Some(m);
        self
    }

    pub fn with_memory_channel_opt(mut self, m: Option<Arc<dyn MemoryChannel>>) -> Self {
        self.memory_channel = m;
        self
    }

    pub fn with_output_tail(mut self, t: Arc<OutputTail>) -> Self {
        self.output_tail = Some(t);
        self
    }

    pub fn with_one_shot_chat(mut self, s: Arc<dyn OneShotChatService>) -> Self {
        self.one_shot_chat = Some(s);
        self
    }

    pub fn with_one_shot_chat_opt(mut self, s: Option<Arc<dyn OneShotChatService>>) -> Self {
        self.one_shot_chat = s;
        self
    }

    pub fn with_fetch_refiner_policy(mut self, p: Arc<dyn FetchRefinerPolicy>) -> Self {
        self.fetch_refiner_policy = Some(p);
        self
    }

    pub fn with_fetch_refiner_policy_opt(mut self, p: Option<Arc<dyn FetchRefinerPolicy>>) -> Self {
        self.fetch_refiner_policy = p;
        self
    }

    pub fn with_goal_session(mut self, g: Arc<dyn GoalSession>) -> Self {
        self.goal_session = Some(g);
        self
    }

    pub fn with_goal_session_opt(mut self, g: Option<Arc<dyn GoalSession>>) -> Self {
        self.goal_session = g;
        self
    }
}

impl Clone for ToolContext {
    fn clone(&self) -> Self {
        Self {
            backend: self.backend.clone(),
            session_id: self.session_id.clone(),
            shared: self.shared.clone(),
            memory_channel: self.memory_channel.clone(),
            output_tail: self.output_tail.clone(),
            one_shot_chat: self.one_shot_chat.clone(),
            fetch_refiner_policy: self.fetch_refiner_policy.clone(),
            goal_session: self.goal_session.clone(),
        }
    }
}

impl fmt::Debug for ToolContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ToolContext")
            .field("cwd", &self.backend.cwd())
            .field("session_id", &self.session_id)
            .field("shared", &self.shared.is_some())
            .finish()
    }
}
