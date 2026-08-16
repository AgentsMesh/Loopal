use std::sync::Arc;

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_protocol::{
    InterruptSignal, WorkflowRunId, WorkflowRunState, WorkflowTerminalDeliveryId,
    WorkflowTerminalNotification, WorkflowTerminalOutcome,
};
use loopal_runtime::agent_input::AgentInput;

use crate::session_hub::SharedSession;
use crate::session_start::SessionHandle;

pub(super) type Peer = (
    Arc<Connection<Listening>>,
    tokio::sync::mpsc::Receiver<Incoming>,
);

pub(super) fn peers() -> (Peer, Peer) {
    let (left, right) = loopal_ipc::duplex_pair();
    (
        Connection::new(left).into_listening(),
        Connection::new(right).into_listening(),
    )
}

pub(super) fn connection() -> Arc<Connection<Listening>> {
    let (transport, _peer) = loopal_ipc::duplex_pair();
    Connection::new(transport).into_listening().0
}

pub(super) fn session(
    id: &str,
    capacity: usize,
) -> (Arc<SharedSession>, tokio::sync::mpsc::Receiver<AgentInput>) {
    let (input_tx, input_rx) = tokio::sync::mpsc::channel(capacity);
    let (interrupt_tx, _) = tokio::sync::watch::channel(0);
    (
        Arc::new(SharedSession::new(
            id.into(),
            input_tx,
            InterruptSignal::new(),
            Arc::new(interrupt_tx),
        )),
        input_rx,
    )
}

pub(super) fn pending_handle(session: Arc<SharedSession>) -> SessionHandle {
    SessionHandle {
        session_id: session.session_id.clone(),
        session,
        agent_task: tokio::spawn(std::future::pending::<Option<loopal_error::AgentOutput>>()),
        lifecycle: loopal_runtime::LifecycleMode::Persistent,
        shutdown: tokio_util::sync::CancellationToken::new(),
        redaction_seed: loopal_output_guard::FinalSinkRedactionSeed::new(),
        completion_result_limit: loopal_output_guard::MAX_AGENT_COMPLETION_RESULT_BYTES,
    }
}

pub(super) fn terminal_notification(session_id: &str) -> WorkflowTerminalNotification {
    WorkflowTerminalNotification {
        delivery_id: WorkflowTerminalDeliveryId::new(
            session_id,
            WorkflowRunId::new("wrun_forward_loop_test"),
            1,
        ),
        state: WorkflowRunState::Succeeded,
        run_goal: "finish".into(),
        outcome: WorkflowTerminalOutcome::Succeeded {
            result: "done".into(),
        },
        content: "workflow done".into(),
    }
}
