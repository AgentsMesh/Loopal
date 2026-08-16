use loopal_protocol::{
    AgentEvent, AgentEventPayload, McpServerSnapshot, WorkflowNodeId, WorkflowRunId,
    WorkflowRunState, WorkflowRunSummary, WorkflowStateCounts,
};
use loopal_session::{ROOT_AGENT, SessionController};
use tokio::sync::mpsc;

fn controller() -> SessionController {
    let (control_tx, _control_rx) = mpsc::channel(1);
    let (permission_tx, _permission_rx) = mpsc::channel(1);
    let (question_tx, _question_rx) = mpsc::channel(1);
    SessionController::new(
        control_tx,
        permission_tx,
        question_tx,
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0).0),
    )
}

fn server(name: &str) -> McpServerSnapshot {
    McpServerSnapshot {
        name: name.into(),
        transport: "stdio".into(),
        source: "project".into(),
        status: "connected".into(),
        tool_count: 2,
        resource_count: 1,
        prompt_count: 0,
        errors: vec![],
    }
}

fn workflow() -> WorkflowRunSummary {
    WorkflowRunSummary {
        id: WorkflowRunId::new("run-1"),
        run_goal: "goal".into(),
        state: WorkflowRunState::Running,
        revision: 1,
        output_node: WorkflowNodeId::new("output"),
        counts: WorkflowStateCounts {
            pending: 1,
            ready: 0,
            active: 0,
            succeeded: 0,
            failed: 0,
            cancelled: 0,
            skipped: 0,
        },
        created_at_unix_ms: 1,
        updated_at_unix_ms: 2,
    }
}

#[test]
fn resumed_and_mcp_events_update_session_state() {
    let controller = controller();
    controller.handle_event(AgentEvent::root(AgentEventPayload::SessionResumed {
        session_id: "session-1".into(),
        message_count: 7,
    }));
    controller.handle_event(AgentEvent::root(AgentEventPayload::McpStatusReport {
        servers: vec![server("filesystem")],
    }));

    let state = controller.lock();
    assert_eq!(state.root_session_id.as_deref(), Some("session-1"));
    assert_eq!(state.mcp_status.as_ref().unwrap()[0].name, "filesystem");
}

#[test]
fn child_session_resume_does_not_rebind_root_session() {
    let controller = controller();
    controller.set_root_session_id("root-session");

    controller.handle_event(AgentEvent::named(
        "child",
        AgentEventPayload::SessionResumed {
            session_id: "child-session".into(),
            message_count: 2,
        },
    ));

    assert_eq!(
        controller.root_session_id().as_deref(),
        Some("root-session")
    );
}

#[test]
fn terminal_active_child_returns_to_root() {
    for payload in [
        AgentEventPayload::Finished,
        AgentEventPayload::Error {
            message: "failed".into(),
        },
    ] {
        let controller = controller();
        controller.enter_agent_view("child");
        controller.handle_event(AgentEvent::named("child", payload));
        assert_eq!(controller.lock().active_view, ROOT_AGENT);
    }
}

#[test]
fn terminal_other_root_or_unnamed_agent_preserves_active_child() {
    let controller = controller();
    controller.enter_agent_view("child");
    controller.handle_event(AgentEvent::named("other", AgentEventPayload::Finished));
    controller.handle_event(AgentEvent::named(ROOT_AGENT, AgentEventPayload::Finished));
    controller.handle_event(AgentEvent::root(AgentEventPayload::Finished));
    assert_eq!(controller.lock().active_view, "child");
}

#[test]
fn per_agent_payloads_do_not_change_session_state() {
    let controller = controller();
    controller.enter_agent_view("child");
    controller.set_root_session_id("session-1");
    controller.handle_event(AgentEvent::root(AgentEventPayload::McpStatusReport {
        servers: vec![server("filesystem")],
    }));

    for payload in [
        AgentEventPayload::Started,
        AgentEventPayload::Running,
        AgentEventPayload::AwaitingInput,
        AgentEventPayload::Interrupted,
        AgentEventPayload::Stream {
            text: "chunk".into(),
        },
        AgentEventPayload::ToolPermissionResolved {
            id: "permission".into(),
        },
        AgentEventPayload::ModeChanged {
            mode: "plan".into(),
        },
        AgentEventPayload::WorkflowRunChanged(workflow()),
        AgentEventPayload::HubDegraded { since_unix_ms: 10 },
        AgentEventPayload::HubRecovered { duration_ms: 5 },
        AgentEventPayload::TurnCancelled {
            cause: "cancelled".into(),
        },
    ] {
        controller.handle_event(AgentEvent::root(payload));
    }

    let state = controller.lock();
    assert_eq!(state.active_view, "child");
    assert_eq!(state.root_session_id.as_deref(), Some("session-1"));
    assert_eq!(state.mcp_status.as_ref().unwrap()[0].name, "filesystem");
}
