use std::collections::VecDeque;
use std::sync::Arc;

use loopal_context::middleware::file_snapshot::FileSnapshot;
use loopal_context::{TurnStore, TurnTracker};
use loopal_protocol::{AgentStatus, InterruptSignal};
use loopal_tool_api::{GoalSession, ToolContext};
use tokio::sync::watch;

use super::AgentLoopParams;
use super::continuation_gate::ContinuationGate;
use super::governance::aggregator::{FirstDenyWins, VerdictAggregator};
use super::governance::traits::{Governance, TurnHook};
use super::model_config::ModelConfig;
use super::token_accumulator::TokenAccumulator;
use super::turn_history::TurnHistory;
use crate::goal::GoalSessionToolAdapter;
use crate::plan_file::PlanFile;

/// Encapsulates the agent loop state and behavior.
pub struct AgentLoopRunner {
    pub params: AgentLoopParams,
    pub tool_ctx: ToolContext,
    pub turn_count: u32,
    pub tokens: TokenAccumulator,
    pub model_config: ModelConfig,
    pub interrupt: InterruptSignal,
    pub interrupt_tx: Arc<watch::Sender<u64>>,
    pub governance: Vec<Box<dyn Governance>>,
    /// Policy for combining per-governance verdicts into a final outcome.
    /// Default is `FirstDenyWins`; tests or future configs can replace it.
    pub aggregator: Box<dyn VerdictAggregator>,
    pub hooks: Vec<Box<dyn TurnHook>>,
    pub config_snapshots: Vec<FileSnapshot>,
    pub trigger_rx: Option<tokio::sync::mpsc::Receiver<loopal_protocol::Envelope>>,
    /// Async hook rewake channel — background hooks send Envelopes here.
    pub rewake_rx: Option<tokio::sync::mpsc::Receiver<loopal_protocol::Envelope>>,
    /// Frontend data received while suspended remains ordered and is consumed
    /// after Unsuspend reopens the session.
    pub deferred_frontend_inputs: VecDeque<crate::agent_input::AgentInput>,
    /// Inputs drained at an ephemeral idle boundary. They are processed one
    /// envelope per turn so each envelope retains its own workflow decision.
    pub ephemeral_pending_inputs: VecDeque<loopal_protocol::Envelope>,
    /// Local status for idempotent `transition()` checks.
    ///
    /// This is NOT the authoritative status for external observers. The session
    /// layer derives its `observable.status` solely from agent events
    /// (`AwaitingInput`, `Finished`, `Error`, etc.). If `emit()` fails during
    /// `transition()`, this field is rolled back so the event can be retried.
    pub status: AgentStatus,
    pub plan_file: PlanFile,
    pub pending_consumed_ids: Vec<String>,
    pub continuation_gate: ContinuationGate,
    pub turn_history: TurnHistory,
    pub last_continuation_goal_id: Option<String>,
    /// Domain-layer turn tracking — current turn id, step index, in-flight
    /// ToolBatch step index, and in-memory `Vec<Turn>` mirror. Mutated only
    /// by `turn_record` helpers under fail-closed atomicity with turns.jsonl.
    pub turns: TurnTracker,
}

impl AgentLoopRunner {
    pub fn new(mut params: AgentLoopParams) -> Self {
        let goal_adapter: Option<Arc<dyn GoalSession>> = params
            .goal_session
            .as_ref()
            .map(|s| Arc::new(GoalSessionToolAdapter::new(Arc::clone(s))) as Arc<dyn GoalSession>);
        let tool_ctx = ToolContext::new(
            params.deps.kernel.create_backend(
                std::path::Path::new(&params.session.cwd),
                &params.session.id,
            ),
            params.session.id.clone(),
        )
        .with_shared_opt(params.shared.clone())
        .with_memory_channel_opt(params.memory_channel.clone())
        .with_one_shot_chat_opt(params.one_shot_chat.clone())
        .with_fetch_refiner_policy_opt(params.fetch_refiner_policy.clone())
        .with_goal_session_opt(goal_adapter)
        .with_protected_effect_audit(params.deps.protected_effect_audit.clone())
        .with_secret_client_opt(params.deps.kernel.secret_client().cloned())
        .with_read_tracker(Arc::new(loopal_tool_api::FileReadTracker::new()));
        let model_config = ModelConfig::from_agent_config(&params.config);
        let interrupt = params.interrupt.signal.clone();
        let interrupt_tx = params.interrupt.tx.clone();
        let trigger_rx = params.scheduled_rx.take();
        let rewake_rx = params.rewake_rx.take();
        let plan_file = PlanFile::new(std::path::Path::new(&params.session.cwd));
        let initial_turns = std::mem::take(&mut params.initial_turns);
        let turn_store = if initial_turns.is_empty() {
            TurnStore::new()
        } else {
            TurnStore::from_turns(initial_turns)
        };
        let turns = TurnTracker::new(turn_store, params.budget.clone());
        Self {
            params,
            tool_ctx,
            turn_count: 0,
            tokens: TokenAccumulator::new(),
            model_config,
            interrupt,
            interrupt_tx,
            governance: Vec::new(),
            aggregator: Box::new(FirstDenyWins),
            hooks: Vec::new(),
            config_snapshots: Vec::new(),
            trigger_rx,
            rewake_rx,
            deferred_frontend_inputs: VecDeque::new(),
            ephemeral_pending_inputs: VecDeque::new(),
            status: AgentStatus::Starting,
            plan_file,
            pending_consumed_ids: Vec::new(),
            continuation_gate: ContinuationGate::new(),
            turn_history: TurnHistory::new(),
            last_continuation_goal_id: None,
            turns,
        }
    }
}
