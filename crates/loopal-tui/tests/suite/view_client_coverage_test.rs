use loopal_protocol::{
    AgentEvent, AgentEventPayload, AgentStatus, WorkflowNodeId, WorkflowRunId, WorkflowRunState,
    WorkflowRunSummary, WorkflowStateCounts,
};
use loopal_tui::view_client::ViewClient;
use loopal_view_state::{SessionViewState, ViewSnapshot, ViewStateApplyOutcome};

fn workflow() -> WorkflowRunSummary {
    WorkflowRunSummary {
        id: WorkflowRunId::new("wrun_snapshot"),
        run_goal: "snapshot workflow".into(),
        state: WorkflowRunState::Running,
        revision: 1,
        output_node: WorkflowNodeId::new("done"),
        counts: WorkflowStateCounts {
            pending: 0,
            ready: 0,
            active: 0,
            succeeded: 0,
            failed: 0,
            cancelled: 0,
            skipped: 0,
        },
        created_at_unix_ms: 1,
        updated_at_unix_ms: 1,
    }
}

#[test]
fn identity_state_and_revision_accessors_track_versioned_and_live_events() {
    let client = ViewClient::empty("main");
    assert_eq!(client.agent(), "main");
    assert_eq!(client.rev(), 0);
    assert_eq!(client.state().rev(), 0);

    let mut started = AgentEvent::root(AgentEventPayload::Started);
    started.rev = Some(4);
    assert_eq!(
        client.apply_event(&started),
        ViewStateApplyOutcome::Applied { revision: 4 }
    );
    assert_eq!(client.rev(), 4);
    assert_eq!(
        client.state().state().agent.observable.status,
        AgentStatus::Running
    );

    let mut stale = AgentEvent::root(AgentEventPayload::Finished);
    stale.rev = Some(4);
    assert_eq!(client.apply_event(&stale), ViewStateApplyOutcome::NoOp);
    assert_eq!(
        client.state().state().agent.observable.status,
        AgentStatus::Running
    );

    assert!(matches!(
        client.apply_event(&AgentEvent::root(AgentEventPayload::AwaitingInput)),
        ViewStateApplyOutcome::Applied { revision: 5 }
    ));
    assert_eq!(client.state().rev(), 5);
}

#[test]
fn events_are_applied_only_to_the_addressed_replica() {
    let root = ViewClient::empty("main");
    let worker = ViewClient::empty("worker");
    let worker_started = AgentEvent::named("worker", AgentEventPayload::Started);

    assert_eq!(
        root.apply_event(&worker_started),
        ViewStateApplyOutcome::NoOp
    );
    assert!(matches!(
        worker.apply_event(&worker_started),
        ViewStateApplyOutcome::Applied { revision: 1 }
    ));
    assert_eq!(
        worker.apply_event(&AgentEvent::root(AgentEventPayload::Finished)),
        ViewStateApplyOutcome::NoOp
    );
}

#[test]
fn snapshot_constructor_retains_root_workflows_but_strips_child_workflows() {
    let mut state = SessionViewState::empty("main");
    state.workflows.active.push(workflow());
    let snapshot = ViewSnapshot { rev: 9, state };

    let root = ViewClient::from_snapshot("main", snapshot.clone());
    assert_eq!(root.rev(), 9);
    assert_eq!(root.state().state().workflows.active.len(), 1);

    let child = ViewClient::from_snapshot("worker", snapshot);
    assert_eq!(child.rev(), 9);
    assert!(child.state().state().workflows.is_empty());
}
