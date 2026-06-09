use std::sync::Arc;

use loopal_protocol::{Envelope, InterruptSignal};
use loopal_provider_api::Message;
use loopal_runtime::{
    AgentConfig, AgentDeps, AgentLoopParams, AgentLoopParamsBuilder, GoalRuntimeSession,
    InterruptHandle, SessionResumeHook,
};
use loopal_storage::Session;
use loopal_tool_api::{FetchRefinerPolicy, MemoryChannel, OneShotChatService};
use loopal_turn::Turn;

/// Aggregate inputs for [`assemble_agent_loop_params`] — collapses what
/// would otherwise be a 14-argument helper into a single value so the
/// `agent_setup` call site stays readable.
pub(crate) struct AgentLoopAssembly {
    pub config: AgentConfig,
    pub deps: AgentDeps,
    pub session: Session,
    pub initial_turns: Vec<Turn>,
    pub budget: loopal_context::ContextBudget,
    pub interrupt: InterruptSignal,
    pub interrupt_tx: Arc<tokio::sync::watch::Sender<u64>>,
    pub shared: Arc<dyn std::any::Any + Send + Sync>,
    pub scheduled_rx: tokio::sync::mpsc::Receiver<Envelope>,
    pub harness: loopal_config::HarnessConfig,
    pub message_snapshot: Arc<std::sync::RwLock<Vec<Message>>>,
    pub resume_hooks: Vec<Arc<dyn SessionResumeHook>>,
    pub memory_channel: Option<Arc<dyn MemoryChannel>>,
    pub one_shot_chat: Option<Arc<dyn OneShotChatService>>,
    pub fetch_refiner_policy: Option<Arc<dyn FetchRefinerPolicy>>,
    pub goal_session: Option<Arc<GoalRuntimeSession>>,
    pub scheduler: Arc<loopal_scheduler::CronScheduler>,
    pub decision_cell: loopal_runtime::frontend::DecisionCell,
}

pub(crate) fn assemble_agent_loop_params(a: AgentLoopAssembly) -> AgentLoopParams {
    let builder = AgentLoopParamsBuilder::new(
        a.config,
        a.deps,
        a.session,
        a.budget,
        InterruptHandle {
            signal: a.interrupt,
            tx: a.interrupt_tx,
        },
    )
    .initial_turns(a.initial_turns)
    .shared(a.shared)
    .scheduled_rx(a.scheduled_rx)
    .harness(a.harness)
    .message_snapshot(a.message_snapshot)
    .resume_hooks(a.resume_hooks)
    .memory_channel_opt(a.memory_channel)
    .scheduler(a.scheduler)
    .decision_cell(a.decision_cell);
    let builder = match a.one_shot_chat {
        Some(s) => builder.one_shot_chat(s),
        None => builder,
    };
    let builder = match a.fetch_refiner_policy {
        Some(p) => builder.fetch_refiner_policy(p),
        None => builder,
    };
    let builder = builder.goal_session_opt(a.goal_session);
    builder.build()
}
