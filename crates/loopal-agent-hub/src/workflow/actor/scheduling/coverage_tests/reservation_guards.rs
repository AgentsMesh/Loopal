use loopal_protocol::{WorkflowAttemptCapability, WorkflowAttemptId, WorkflowRunId};

use super::*;
use crate::workflow::WorkflowIdSource;

struct FixedAttemptId(WorkflowAttemptId);

impl WorkflowIdSource for FixedAttemptId {
    fn next_run_id(&self) -> WorkflowRunId {
        WorkflowRunId::new("wrun_unused")
    }

    fn next_attempt_id(&self) -> WorkflowAttemptId {
        self.0.clone()
    }

    fn next_attempt_capability(&self) -> WorkflowAttemptCapability {
        WorkflowAttemptCapability::parse("55".repeat(32)).unwrap()
    }
}

fn with_id(run: WorkflowRunSnapshot, id: &str) -> WorkflowCoordinator {
    let (mut coordinator, _commands) = super::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        TestSpawner::confirmed(),
        20,
    );
    coordinator.ids = Arc::new(FixedAttemptId(WorkflowAttemptId::new(id)));
    coordinator
}

#[tokio::test]
async fn reservation_rejects_invalid_and_colliding_attempt_ids() {
    let owner = owner();
    let run = running_ready_run("wrun_reserve_invalid");
    let run_id = run.id.clone();
    let mut coordinator = with_id(run, "bad/id");
    assert!(matches!(
        dispatch::admit(&mut coordinator, owner.clone(), run_id).await,
        Err(WorkflowCoordinatorError::InvalidGeneratedAttemptId(_))
    ));

    let run = running_ready_run("wrun_reserve_active");
    let run_id = run.id.clone();
    let id = WorkflowAttemptId::new("watt_reserve_active");
    let mut coordinator = with_id(run, id.as_str());
    let key = AttemptKey {
        run_id: run_id.clone(),
        node_id: WorkflowNodeId::new("node"),
        attempt_id: id.clone(),
    };
    coordinator.active.insert(
        id.clone(),
        active(
            &owner,
            &key,
            AgentExecutionRef::local("active", 1),
            ActiveAttemptPhase::Running,
        ),
    );
    assert!(matches!(
        dispatch::admit(&mut coordinator, owner.clone(), run_id).await,
        Err(WorkflowCoordinatorError::AttemptIdCollision(_))
    ));

    let run = running_ready_run("wrun_reserve_pending");
    let run_id = run.id.clone();
    let id = WorkflowAttemptId::new("watt_reserve_pending");
    let mut coordinator = with_id(run, id.as_str());
    let key = AttemptKey {
        run_id: run_id.clone(),
        node_id: WorkflowNodeId::new("node"),
        attempt_id: id.clone(),
    };
    coordinator.pending.insert(id, pending(&owner, &key));
    assert!(matches!(
        dispatch::admit(&mut coordinator, owner.clone(), run_id).await,
        Err(WorkflowCoordinatorError::AttemptIdCollision(_))
    ));

    let id = "watt_reserve_snapshot";
    let (mut run, _) = dispatching_run("wrun_reserve_snapshot", id);
    run.spec.limits.max_parallel = 2;
    run.nodes[0].state = WorkflowNodeState::Ready;
    run.nodes[0].current_attempt = None;
    let run_id = run.id.clone();
    let mut coordinator = with_id(run, id);
    assert!(matches!(
        dispatch::admit(&mut coordinator, owner, run_id).await,
        Err(WorkflowCoordinatorError::AttemptIdCollision(_))
    ));
}
