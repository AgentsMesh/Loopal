use std::sync::Arc;

use loopal_protocol::{Envelope, InterruptSignal};
use loopal_provider_api::Message;
use loopal_runtime::{
    AgentConfig, AgentDeps, AgentLoopParams, AgentLoopParamsBuilder, GoalRuntimeSession,
    InterruptHandle, SessionResumeHook,
};
use loopal_storage::Session;
use loopal_tool_api::{
    FetchRefinerPolicy, MemoryChannel, OneShotChatService, OutstandingTasksDigest,
};
use loopal_turn::Turn;

/// Aggregate inputs for [`assemble_agent_loop_params`] — collapses what
/// would otherwise be a 14-argument helper into a single value so the
/// `agent_setup` call site stays readable.
pub(crate) struct AgentLoopAssembly {
    pub config: AgentConfig,
    pub deps: AgentDeps,
    pub session: Session,
    pub initial_turns: Vec<Turn>,
    pub hydrate_initial_history: bool,
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
    pub outstanding_tasks: Option<Arc<dyn OutstandingTasksDigest>>,
    pub goal_session: Option<Arc<GoalRuntimeSession>>,
    pub scheduler: Arc<loopal_scheduler::CronScheduler>,
    pub workflow_permission_causation: Option<loopal_protocol::WorkflowPermissionCausation>,
    pub decision_cell: loopal_runtime::frontend::DecisionCell,
    pub workflow_input_handler:
        Option<Arc<dyn loopal_runtime::workflow_input::WorkflowInputHandler>>,
    pub workflow_lease_tracker: Arc<loopal_runtime::WorkflowLeaseTracker>,
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
    .hydrate_initial_history(a.hydrate_initial_history)
    .shared(a.shared)
    .scheduled_rx(a.scheduled_rx)
    .harness(a.harness)
    .message_snapshot(a.message_snapshot)
    .resume_hooks(a.resume_hooks)
    .memory_channel_opt(a.memory_channel)
    .scheduler(a.scheduler)
    .workflow_permission_causation_opt(a.workflow_permission_causation)
    .decision_cell(a.decision_cell)
    .workflow_lease_tracker(a.workflow_lease_tracker);
    let builder = match a.workflow_input_handler {
        Some(handler) => builder.workflow_input_handler(handler),
        None => builder,
    };
    let builder = match a.one_shot_chat {
        Some(s) => builder.one_shot_chat(s),
        None => builder,
    };
    let builder = match a.fetch_refiner_policy {
        Some(p) => builder.fetch_refiner_policy(p),
        None => builder,
    };
    let builder = builder.outstanding_tasks_opt(a.outstanding_tasks);
    let builder = builder.goal_session_opt(a.goal_session);
    builder.build()
}
