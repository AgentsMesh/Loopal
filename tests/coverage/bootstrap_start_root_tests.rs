use std::sync::Arc;
use std::sync::atomic::Ordering;

use tokio::sync::Mutex;

use super::*;
use crate::states::RootPending;
use crate::workflow::WorkflowRuntime;

fn pending(with_runtime: bool) -> RootPending {
    RootPending {
        hub: Arc::new(Mutex::new(Hub)),
        hub_token: "hub-token".into(),
        agent_proc: AgentProcess,
        client_conn: Arc::new(ClientConnection),
        workflow_runtime: with_runtime.then_some(WorkflowRuntime),
    }
}

fn select_failure(mask: u8) {
    FAILURE_MASK.store(mask, Ordering::SeqCst);
    RECOVERIES.store(0, Ordering::SeqCst);
    ACTIVATIONS.store(0, Ordering::SeqCst);
    RUNTIME_SHUTDOWNS.store(0, Ordering::SeqCst);
    PROCESS_SHUTDOWNS.store(0, Ordering::SeqCst);
    WARNINGS.store(0, Ordering::SeqCst);
}

async fn failure(mask: u8, expected: &str) {
    select_failure(mask);
    let error = match pending(true)
        .start_root_agent(StartAgentParams::default())
        .await
    {
        Ok(_) => panic!("selected startup boundary must fail"),
        Err(error) => error,
    };
    assert!(error.to_string().contains(expected), "{error}");
    assert_eq!(RUNTIME_SHUTDOWNS.load(Ordering::SeqCst), 1);
    assert_eq!(PROCESS_SHUTDOWNS.load(Ordering::SeqCst), 1);
}

async fn success_paths_bind_generated_and_resumed_session_ids() {
    select_failure(0);
    let generated = pending(false)
        .start_root_agent(StartAgentParams::default())
        .await
        .unwrap();
    assert_eq!(generated.root_session_id, "generated-session");
    assert!(generated.workflow_runtime.is_none());

    let resumed = pending(true)
        .start_root_agent(StartAgentParams {
            resume: Some("resumed-session".into()),
            session_id: None,
        })
        .await
        .unwrap();
    assert_eq!(resumed.root_session_id, "resumed-session");
    assert_eq!(RECOVERIES.load(Ordering::SeqCst), 1);
    assert_eq!(ACTIVATIONS.load(Ordering::SeqCst), 1);
}

async fn every_startup_boundary_failure_rolls_back_owned_resources() {
    failure(FAIL_BIND, "bind failure").await;
    failure(FAIL_RECOVERY, "workflow recovery failed").await;
    failure(FAIL_START, "Agent start failure").await;
    failure(MISMATCH_SESSION, "different Hub-bound session id").await;
    failure(FAIL_ACTIVATION, "terminal delivery activation failed").await;

    failure(
        FAIL_BIND | FAIL_RUNTIME_SHUTDOWN | FAIL_PROCESS_SHUTDOWN,
        "bind failure",
    )
    .await;
    assert_eq!(WARNINGS.load(Ordering::SeqCst), 2);
    select_failure(0);
}

#[tokio::test]
async fn root_start_success_and_failure_boundaries_are_deterministic() {
    success_paths_bind_generated_and_resumed_session_ids().await;
    every_startup_boundary_failure_rolls_back_owned_resources().await;
}
