use std::sync::Arc;

use loopal_error::AgentOutput;
use loopal_output_guard::FinalSinkRedactionSeed;
use tokio_util::sync::CancellationToken;

use crate::session_hub::SharedSession;

pub(crate) struct SessionHandle {
    pub session_id: String,
    pub session: Arc<SharedSession>,
    pub agent_task: tokio::task::JoinHandle<Option<AgentOutput>>,
    pub lifecycle: loopal_runtime::LifecycleMode,
    /// Level-triggered session termination, distinct from per-turn interrupt.
    pub shutdown: CancellationToken,
    pub redaction_seed: FinalSinkRedactionSeed,
    pub completion_result_limit: usize,
}
