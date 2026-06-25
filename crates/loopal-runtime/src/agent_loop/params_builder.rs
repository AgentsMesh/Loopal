use std::sync::Arc;

use loopal_config::HarnessConfig;
use loopal_context::ContextBudget;
use loopal_provider_api::Message;
use loopal_storage::Session;
use loopal_tool_api::{
    FetchRefinerPolicy, MemoryChannel, OneShotChatService, OutstandingTasksDigest,
};
use loopal_turn::Turn;

use super::params::{AgentConfig, AgentDeps, AgentLoopParams, InterruptHandle};
use crate::goal::GoalRuntimeSession;
use crate::session_resume_hook::SessionResumeHook;

pub struct AgentLoopParamsBuilder {
    config: AgentConfig,
    deps: AgentDeps,
    session: Session,
    budget: ContextBudget,
    interrupt: InterruptHandle,
    initial_turns: Vec<Turn>,
    shared: Option<Arc<dyn std::any::Any + Send + Sync>>,
    memory_channel: Option<Arc<dyn MemoryChannel>>,
    one_shot_chat: Option<Arc<dyn OneShotChatService>>,
    fetch_refiner_policy: Option<Arc<dyn FetchRefinerPolicy>>,
    outstanding_tasks: Option<Arc<dyn OutstandingTasksDigest>>,
    goal_session: Option<Arc<GoalRuntimeSession>>,
    scheduled_rx: Option<tokio::sync::mpsc::Receiver<loopal_protocol::Envelope>>,
    harness: HarnessConfig,
    rewake_rx: Option<tokio::sync::mpsc::Receiver<loopal_protocol::Envelope>>,
    message_snapshot: Option<Arc<std::sync::RwLock<Vec<Message>>>>,
    resume_hooks: Vec<Arc<dyn SessionResumeHook>>,
    scheduler: Option<Arc<loopal_scheduler::CronScheduler>>,
    decision_cell: crate::frontend::DecisionCell,
}

impl AgentLoopParamsBuilder {
    pub fn new(
        config: AgentConfig,
        deps: AgentDeps,
        session: Session,
        budget: ContextBudget,
        interrupt: InterruptHandle,
    ) -> Self {
        Self {
            config,
            deps,
            session,
            budget,
            interrupt,
            initial_turns: Vec::new(),
            shared: None,
            memory_channel: None,
            one_shot_chat: None,
            fetch_refiner_policy: None,
            outstanding_tasks: None,
            goal_session: None,
            scheduled_rx: None,
            harness: HarnessConfig::default(),
            rewake_rx: None,
            message_snapshot: None,
            resume_hooks: Vec::new(),
            scheduler: None,
            decision_cell: crate::frontend::DecisionCell::new(
                loopal_decision_api::DecisionMode::default(),
            ),
        }
    }

    pub fn initial_turns(mut self, turns: Vec<Turn>) -> Self {
        self.initial_turns = turns;
        self
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
    pub fn outstanding_tasks_opt(mut self, t: Option<Arc<dyn OutstandingTasksDigest>>) -> Self {
        self.outstanding_tasks = t;
        self
    }
    pub fn goal_session(mut self, g: Arc<GoalRuntimeSession>) -> Self {
        self.goal_session = Some(g);
        self
    }
    pub fn goal_session_opt(mut self, g: Option<Arc<GoalRuntimeSession>>) -> Self {
        self.goal_session = g;
        self
    }
    pub fn scheduled_rx(
        mut self,
        rx: tokio::sync::mpsc::Receiver<loopal_protocol::Envelope>,
    ) -> Self {
        self.scheduled_rx = Some(rx);
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
    pub fn scheduler(mut self, s: Arc<loopal_scheduler::CronScheduler>) -> Self {
        self.scheduler = Some(s);
        self
    }
    pub fn decision_cell(mut self, c: crate::frontend::DecisionCell) -> Self {
        self.decision_cell = c;
        self
    }

    pub fn build(self) -> AgentLoopParams {
        AgentLoopParams {
            config: self.config,
            deps: self.deps,
            session: self.session,
            budget: self.budget,
            initial_turns: self.initial_turns,
            interrupt: self.interrupt,
            shared: self.shared,
            memory_channel: self.memory_channel,
            one_shot_chat: self.one_shot_chat,
            fetch_refiner_policy: self.fetch_refiner_policy,
            outstanding_tasks: self.outstanding_tasks,
            goal_session: self.goal_session,
            scheduled_rx: self.scheduled_rx,
            harness: self.harness,
            rewake_rx: self.rewake_rx,
            message_snapshot: self.message_snapshot,
            resume_hooks: self.resume_hooks,
            scheduler: self.scheduler,
            decision_cell: self.decision_cell,
        }
    }
}
