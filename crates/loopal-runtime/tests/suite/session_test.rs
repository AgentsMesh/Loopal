use loopal_protocol::{
    QualifiedAddress, WORKFLOW_SPEC_V1, WorkflowAgentNode, WorkflowEvent, WorkflowEventPayload,
    WorkflowLimits, WorkflowOutputContract, WorkflowRunId, WorkflowRunSnapshot, WorkflowRunState,
    WorkflowSpec, WorkflowTerminalDeliveryId, WorkflowTerminalNotification,
    WorkflowTerminalOutcome, WorkflowWorkerProfileRef,
};
use loopal_runtime::SessionManager;
use loopal_storage::{SessionStore, WorkflowJournal};
use std::path::Path;
use tempfile::TempDir;

fn make_manager(tmp: &TempDir) -> SessionManager {
    SessionManager::with_base_dir(tmp.path().to_path_buf())
}

#[test]
fn test_session_manager_new() {
    // SessionManager::new() uses home dir — just verify it does not panic.
    // It may fail if home dir is not available, but that's unusual in CI.
    let result = SessionManager::new();
    assert!(result.is_ok(), "SessionManager::new() should succeed");
}

#[test]
fn test_create_session() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(&tmp);

    let cwd = Path::new("/tmp/test_project");
    let session = mgr.create_session(cwd, "test-model").unwrap();

    assert!(!session.id.is_empty(), "session ID should not be empty");
    assert_eq!(session.model, "test-model");
    assert_eq!(session.cwd, cwd.to_string_lossy());
}

#[test]
fn test_resume_session() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(&tmp);

    let cwd = Path::new("/tmp/test_project");
    let session = mgr.create_session(cwd, "test-model").unwrap();
    let session_id = session.id.clone();

    // Resume the session
    let (resumed, turns) = mgr.resume_session(&session_id).unwrap();

    assert_eq!(resumed.id, session_id);
    assert_eq!(resumed.model, "test-model");
    assert!(turns.is_empty(), "fresh session should have no turns");
}

#[test]
fn test_resume_nonexistent_session_fails() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(&tmp);

    let result = mgr.resume_session("nonexistent-session-id-12345");
    assert!(
        result.is_err(),
        "resuming a nonexistent session should fail"
    );
}

#[test]
fn test_list_sessions() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(&tmp);

    // Initially empty
    let sessions = mgr.list_sessions().unwrap();
    assert!(sessions.is_empty());

    // Create a session
    let cwd = Path::new("/tmp/test_project");
    let _s1 = mgr.create_session(cwd, "model-a").unwrap();
    let _s2 = mgr.create_session(cwd, "model-b").unwrap();

    let sessions = mgr.list_sessions().unwrap();
    assert_eq!(sessions.len(), 2);
}

#[test]
fn test_update_session() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(&tmp);

    let cwd = Path::new("/tmp/test_project");
    let mut session = mgr.create_session(cwd, "test-model").unwrap();
    let session_id = session.id.clone();

    // Update the title
    session.title = "My Updated Title".to_string();
    mgr.update_session(&session).unwrap();

    // Resume and verify
    let (resumed, _turns) = mgr.resume_session(&session_id).unwrap();
    assert_eq!(resumed.title, "My Updated Title");
}

#[test]
fn pending_workflow_delivery_ids_track_unacknowledged_terminal_intents() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(&tmp);
    let session = mgr
        .create_session(Path::new("/tmp/workflow_project"), "test-model")
        .unwrap();
    let sessions = SessionStore::with_base_dir(tmp.path().to_path_buf());
    let run_id = WorkflowRunId::new("wrun_pending");
    let journal =
        WorkflowJournal::from_session_store(&sessions, &session.id, run_id.clone()).unwrap();
    journal
        .append_init(planned_snapshot(run_id.clone()))
        .unwrap();
    journal
        .append_commit(
            vec![WorkflowEvent {
                run_id: run_id.clone(),
                revision: 1,
                occurred_at_unix_ms: 2,
                payload: WorkflowEventPayload::CancelRequested { reason: None },
            }],
            None,
        )
        .unwrap();
    let delivery_id = WorkflowTerminalDeliveryId::new(&session.id, run_id.clone(), 1);
    journal
        .append_delivery_intent(WorkflowTerminalNotification {
            delivery_id: delivery_id.clone(),
            state: WorkflowRunState::Cancelled,
            run_goal: "pending delivery".into(),
            outcome: WorkflowTerminalOutcome::Cancelled {
                reason: "cancelled".into(),
            },
            content: "cancelled".into(),
        })
        .unwrap();

    assert_eq!(
        mgr.pending_workflow_delivery_run_ids(&session.id).unwrap(),
        vec![run_id.clone()]
    );
    journal.append_delivery_ack(delivery_id).unwrap();
    assert!(
        mgr.pending_workflow_delivery_run_ids(&session.id)
            .unwrap()
            .is_empty()
    );
}

fn planned_snapshot(run_id: WorkflowRunId) -> WorkflowRunSnapshot {
    WorkflowRunSnapshot::planned(
        run_id,
        QualifiedAddress::local("root"),
        WorkflowSpec {
            version: WORKFLOW_SPEC_V1,
            run_goal: "pending delivery".into(),
            nodes: vec![WorkflowAgentNode {
                id: "worker".into(),
                dependencies: Vec::new(),
                task: "cancel worker".into(),
                worker_profile: WorkflowWorkerProfileRef::new("default"),
            }],
            limits: WorkflowLimits {
                max_nodes: 1,
                max_parallel: 1,
                max_attempts: 1,
                run_deadline_ms: 1_000,
                attempt_timeout_ms: 500,
                max_output_bytes: 1_024,
            },
            output_node: "worker".into(),
            output_contract: WorkflowOutputContract::Text { max_bytes: 1_024 },
        },
        1,
    )
}
