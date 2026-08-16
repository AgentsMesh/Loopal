use std::sync::Arc;

use loopal_protocol::{
    InterruptSignal, WorkflowRunId, WorkflowRunState, WorkflowTerminalDeliveryId,
    WorkflowTerminalNotification, WorkflowTerminalOutcome,
};
use loopal_runtime::agent_input::{AgentInput, WorkflowTerminalRequest};

use super::*;

fn session() -> SharedSession {
    let (input_tx, _input_rx) = tokio::sync::mpsc::channel::<AgentInput>(1);
    let (interrupt_tx, _) = tokio::sync::watch::channel(0);
    SharedSession::new(
        "session-capacity".into(),
        input_tx,
        InterruptSignal::new(),
        Arc::new(interrupt_tx),
    )
}

fn notification(terminal_revision: u64) -> WorkflowTerminalNotification {
    WorkflowTerminalNotification {
        delivery_id: WorkflowTerminalDeliveryId::new(
            "session-capacity",
            WorkflowRunId::new("wrun_capacity_test"),
            terminal_revision,
        ),
        state: WorkflowRunState::Succeeded,
        run_goal: "finish".into(),
        outcome: WorkflowTerminalOutcome::Succeeded {
            result: "done".into(),
        },
        content: "workflow done".into(),
    }
}

include!("workflow_terminal_pending_tests/capacity.rs");
include!("workflow_terminal_pending_tests/leases.rs");
