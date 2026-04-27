//! Helper that wires up [`AgentLoopParams`] from the various pieces
//! `agent_setup` already prepared. Split out so `agent_setup.rs` stays
//! within the project's 200-LOC file budget.

use std::sync::Arc;

use loopal_context::ContextStore;
use loopal_message::Message;
use loopal_protocol::{Envelope, InterruptSignal};
use loopal_runtime::{
    AgentConfig, AgentDeps, AgentLoopParams, AgentLoopParamsBuilder, InterruptHandle,
    SessionResumeHook,
};
use loopal_storage::Session;
use loopal_tool_api::MemoryChannel;

/// Aggregate inputs for [`assemble_agent_loop_params`] — collapses what
/// would otherwise be a 14-argument helper into a single value so the
/// `agent_setup` call site stays readable.
pub(crate) struct AgentLoopAssembly {
    pub config: AgentConfig,
    pub deps: AgentDeps,
    pub session: Session,
    pub messages: Vec<Message>,
    pub budget: loopal_context::ContextBudget,
    pub interrupt: InterruptSignal,
    pub interrupt_tx: Arc<tokio::sync::watch::Sender<u64>>,
    pub shared: Arc<dyn std::any::Any + Send + Sync>,
    pub scheduled_rx: tokio::sync::mpsc::Receiver<Envelope>,
    pub harness: loopal_config::HarnessConfig,
    pub message_snapshot: Arc<std::sync::RwLock<Vec<Message>>>,
    pub resume_hooks: Vec<Arc<dyn SessionResumeHook>>,
    pub memory_channel: Option<Arc<dyn MemoryChannel>>,
    pub auto_classifier: Option<Arc<loopal_auto_mode::AutoClassifier>>,
}

pub(crate) fn assemble_agent_loop_params(a: AgentLoopAssembly) -> AgentLoopParams {
    AgentLoopParamsBuilder::new(
        a.config,
        a.deps,
        a.session,
        ContextStore::from_messages(a.messages, a.budget),
        InterruptHandle {
            signal: a.interrupt,
            tx: a.interrupt_tx,
        },
    )
    .shared(a.shared)
    .scheduled_rx(a.scheduled_rx)
    .harness(a.harness)
    .message_snapshot(a.message_snapshot)
    .resume_hooks(a.resume_hooks)
    .memory_channel_opt(a.memory_channel)
    .auto_classifier_opt(a.auto_classifier)
    .build()
}
