//! Builder for [`AgentLoopParams`] — keeps test/setup call sites
//! resilient to new fields.
//!
//! Without a builder, every new optional field on `AgentLoopParams`
//! forces every callsite (12+ test fixtures + production setup) to add
//! `field: default_value`. The builder centralizes the defaults, so
//! adding a field is a one-line change here, not a sweep across the
//! codebase.
//!
//! Required arguments (`config`, `deps`, `session`, `store`,
//! `interrupt`) are passed to `new()`; everything else has a sensible
//! default and is overridden via fluent setters.

use std::sync::Arc;

use loopal_config::HarnessConfig;
use loopal_context::ContextStore;
use loopal_message::Message;
use loopal_storage::Session;
use loopal_tool_api::{FetchRefinerPolicy, MemoryChannel, OneShotChatService};

use super::params::{AgentConfig, AgentDeps, AgentLoopParams, InterruptHandle};
use crate::session_resume_hook::SessionResumeHook;

pub struct AgentLoopParamsBuilder {
    config: AgentConfig,
    deps: AgentDeps,
    session: Session,
    store: ContextStore,
    interrupt: InterruptHandle,
    shared: Option<Arc<dyn std::any::Any + Send + Sync>>,
    memory_channel: Option<Arc<dyn MemoryChannel>>,
    one_shot_chat: Option<Arc<dyn OneShotChatService>>,
    fetch_refiner_policy: Option<Arc<dyn FetchRefinerPolicy>>,
    scheduled_rx: Option<tokio::sync::mpsc::Receiver<loopal_protocol::Envelope>>,
    auto_classifier: Option<Arc<loopal_auto_mode::AutoClassifier>>,
    harness: HarnessConfig,
    rewake_rx: Option<tokio::sync::mpsc::Receiver<loopal_protocol::Envelope>>,
    message_snapshot: Option<Arc<std::sync::RwLock<Vec<Message>>>>,
    resume_hooks: Vec<Arc<dyn SessionResumeHook>>,
}

impl AgentLoopParamsBuilder {
    pub fn new(
        config: AgentConfig,
        deps: AgentDeps,
        session: Session,
        store: ContextStore,
        interrupt: InterruptHandle,
    ) -> Self {
        Self {
            config,
            deps,
            session,
            store,
            interrupt,
            shared: None,
            memory_channel: None,
            one_shot_chat: None,
            fetch_refiner_policy: None,
            scheduled_rx: None,
            auto_classifier: None,
            harness: HarnessConfig::default(),
            rewake_rx: None,
            message_snapshot: None,
            resume_hooks: Vec::new(),
        }
    }

    pub fn shared(mut self, s: Arc<dyn std::any::Any + Send + Sync>) -> Self {
        self.shared = Some(s);
        self
    }
    pub fn memory_channel(mut self, m: Arc<dyn MemoryChannel>) -> Self {
        self.memory_channel = Some(m);
        self
    }
    pub fn memory_channel_opt(mut self, m: Option<Arc<dyn MemoryChannel>>) -> Self {
        self.memory_channel = m;
        self
    }
    pub fn one_shot_chat(mut self, s: Arc<dyn OneShotChatService>) -> Self {
        self.one_shot_chat = Some(s);
        self
    }
    pub fn fetch_refiner_policy(mut self, p: Arc<dyn FetchRefinerPolicy>) -> Self {
        self.fetch_refiner_policy = Some(p);
        self
    }
    pub fn scheduled_rx(
        mut self,
        rx: tokio::sync::mpsc::Receiver<loopal_protocol::Envelope>,
    ) -> Self {
        self.scheduled_rx = Some(rx);
        self
    }
    pub fn auto_classifier(mut self, c: Arc<loopal_auto_mode::AutoClassifier>) -> Self {
        self.auto_classifier = Some(c);
        self
    }
    pub fn auto_classifier_opt(mut self, c: Option<Arc<loopal_auto_mode::AutoClassifier>>) -> Self {
        self.auto_classifier = c;
        self
    }
    pub fn harness(mut self, h: HarnessConfig) -> Self {
        self.harness = h;
        self
    }
    pub fn rewake_rx(mut self, rx: tokio::sync::mpsc::Receiver<loopal_protocol::Envelope>) -> Self {
        self.rewake_rx = Some(rx);
        self
    }
    pub fn message_snapshot(mut self, m: Arc<std::sync::RwLock<Vec<Message>>>) -> Self {
        self.message_snapshot = Some(m);
        self
    }
    pub fn resume_hooks(mut self, h: Vec<Arc<dyn SessionResumeHook>>) -> Self {
        self.resume_hooks = h;
        self
    }

    pub fn build(self) -> AgentLoopParams {
        AgentLoopParams {
            config: self.config,
            deps: self.deps,
            session: self.session,
            store: self.store,
            interrupt: self.interrupt,
            shared: self.shared,
            memory_channel: self.memory_channel,
            one_shot_chat: self.one_shot_chat,
            fetch_refiner_policy: self.fetch_refiner_policy,
            scheduled_rx: self.scheduled_rx,
            auto_classifier: self.auto_classifier,
            harness: self.harness,
            rewake_rx: self.rewake_rx,
            message_snapshot: self.message_snapshot,
            resume_hooks: self.resume_hooks,
        }
    }
}
