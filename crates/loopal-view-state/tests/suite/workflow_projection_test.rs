use loopal_protocol::{
    AgentEventPayload, AgentStateSnapshot, WorkflowNodeId, WorkflowRunId, WorkflowRunState,
    WorkflowRunSummary, WorkflowStateCounts,
};
use loopal_view_state::{ViewStateApplyOutcome, ViewStateReducer, WorkflowRevisionGap};

fn summary(id: &str, state: WorkflowRunState, revision: u64, updated: u64) -> WorkflowRunSummary {
    WorkflowRunSummary {
        id: WorkflowRunId::new(id),
        run_goal: format!("goal-{id}"),
        state,
        revision,
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
        created_at_unix_ms: updated.saturating_sub(1),
        updated_at_unix_ms: updated,
    }
}

fn apply(reducer: &mut ViewStateReducer, run: WorkflowRunSummary) -> Option<u64> {
    reducer.apply(AgentEventPayload::WorkflowRunChanged(run))
}

#[test]
fn active_run_upserts_and_duplicate_or_stale_revision_is_noop() {
    let mut reducer = ViewStateReducer::new("main");
    let first = summary("run-1", WorkflowRunState::Running, 2, 20);
    assert_eq!(apply(&mut reducer, first.clone()), Some(1));
    assert_eq!(reducer.state().workflows.active, vec![first.clone()]);

    assert!(apply(&mut reducer, first).is_none());
    assert!(
        apply(
            &mut reducer,
            summary("run-1", WorkflowRunState::Running, 1, 30)
        )
        .is_none()
    );
    assert_eq!(reducer.rev(), 1);

    let newer = summary("run-1", WorkflowRunState::Running, 3, 40);
    assert_eq!(apply(&mut reducer, newer.clone()), Some(2));
    assert_eq!(reducer.state().workflows.active, vec![newer]);
}

#[test]
fn revision_gap_requests_resync_without_mutating_projection() {
    let mut reducer = ViewStateReducer::new("main");
    let current = summary("run-1", WorkflowRunState::Running, 2, 20);
    apply(&mut reducer, current.clone());

    let outcome = reducer.apply_checked(AgentEventPayload::WorkflowRunChanged(summary(
        "run-1",
        WorkflowRunState::Succeeded,
        4,
        40,
    )));
    assert_eq!(
        outcome,
        ViewStateApplyOutcome::ResyncRequired(WorkflowRevisionGap {
            run_id: WorkflowRunId::new("run-1"),
            expected_revision: 3,
            actual_revision: 4,
        })
    );
    assert_eq!(reducer.rev(), 1);
    assert_eq!(reducer.state().workflows.active, vec![current]);
    assert!(reducer.state().workflows.recent.is_empty());

    assert!(
        apply(
            &mut reducer,
            summary("run-1", WorkflowRunState::Running, 3, 30)
        )
        .is_some()
    );
    let terminal = summary("run-1", WorkflowRunState::Succeeded, 4, 40);
    assert!(apply(&mut reducer, terminal.clone()).is_some());
    assert_eq!(reducer.state().workflows.recent, vec![terminal]);
}

#[test]
fn terminal_run_moves_to_recent_and_cannot_be_rewritten() {
    let mut reducer = ViewStateReducer::new("main");
    apply(
        &mut reducer,
        summary("run-1", WorkflowRunState::Running, 1, 10),
    );
    let terminal = summary("run-1", WorkflowRunState::Succeeded, 2, 20);
    apply(&mut reducer, terminal.clone());

    assert!(reducer.state().workflows.active.is_empty());
    assert_eq!(reducer.state().workflows.recent, vec![terminal]);
    assert!(
        apply(
            &mut reducer,
            summary("run-1", WorkflowRunState::Running, 3, 30)
        )
        .is_none()
    );
    assert!(
        apply(
            &mut reducer,
            summary("run-1", WorkflowRunState::Failed, 4, 40)
        )
        .is_none()
    );
}

#[test]
fn recent_runs_are_bounded_newest_first_and_resume_clears_them() {
    let mut reducer = ViewStateReducer::new("main");
    apply(
        &mut reducer,
        summary("active", WorkflowRunState::Running, 1, 100),
    );
    for index in 0..40 {
        apply(
            &mut reducer,
            summary(
                &format!("run-{index:02}"),
                WorkflowRunState::Cancelled,
                1,
                index,
            ),
        );
    }
    let recent = &reducer.state().workflows.recent;
    assert_eq!(recent.len(), 32);
    assert_eq!(recent.first().unwrap().id.as_str(), "run-39");
    assert_eq!(recent.last().unwrap().id.as_str(), "run-08");
    assert_eq!(reducer.state().workflows.active.len(), 1);

    reducer.apply(AgentEventPayload::SessionResumed {
        session_id: "next-session".into(),
        message_count: 0,
    });
    assert!(reducer.state().workflows.active.is_empty());
    assert!(reducer.state().workflows.recent.is_empty());
}

#[test]
fn active_runs_sort_by_creation_time_then_id() {
    let mut reducer = ViewStateReducer::new("main");
    apply(
        &mut reducer,
        summary("run-b", WorkflowRunState::Running, 1, 30),
    );
    apply(
        &mut reducer,
        summary("run-c", WorkflowRunState::Validated, 1, 20),
    );
    apply(
        &mut reducer,
        summary("run-a", WorkflowRunState::Planned, 1, 20),
    );

    let ids = reducer
        .state()
        .workflows
        .active
        .iter()
        .map(|run| run.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["run-a", "run-c", "run-b"]);
}

#[test]
fn snapshot_seeds_active_and_recent_then_accepts_next_revision() {
    let active = summary("active", WorkflowRunState::Running, 8, 80);
    let terminal = summary("done", WorkflowRunState::Succeeded, 3, 30);
    let mut snapshot = AgentStateSnapshot::empty();
    snapshot.workflows.active.push(active);
    snapshot.workflows.recent.push(terminal.clone());

    let mut reducer = ViewStateReducer::from_snapshot("main", snapshot);
    assert_eq!(reducer.state().workflows.recent, vec![terminal]);
    let next = summary("active", WorkflowRunState::Running, 9, 90);
    assert_eq!(apply(&mut reducer, next.clone()), Some(2));
    assert_eq!(reducer.state().workflows.active, vec![next]);
}
