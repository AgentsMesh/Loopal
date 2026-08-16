use loopal_ipc::Connection;

use super::support::harness;
use crate::types::AgentExecutionRef;

#[tokio::test]
async fn completion_monitor_distinguishes_stale_lease_from_disconnected_transport() {
    let fixture = harness().await;
    let control = fixture.spawner.attempts.lock().await.by_attempt[&fixture.causation.attempt_id]
        .control
        .clone();
    let stale = AgentExecutionRef::local(
        fixture.execution.address.agent.clone(),
        fixture.execution.connection_generation + 1,
    );
    let completion = super::super::monitor::wait_completion(
        fixture.spawner.hub.clone(),
        &stale,
        &control.connection,
    )
    .await;
    assert_eq!(completion.reason, "transport_error");
    assert!(completion.result.unwrap().contains("lease is stale"));

    let (_peer, transport) = loopal_ipc::duplex_pair();
    let disconnected = Connection::new(transport).into_listening().0;
    disconnected.close().await;
    let completion = super::super::monitor::wait_completion(
        fixture.spawner.hub.clone(),
        &fixture.execution,
        &disconnected,
    )
    .await;
    assert_eq!(completion.reason, "transport_error");
    assert!(
        completion
            .result
            .unwrap()
            .contains("before exact completion")
    );
}
