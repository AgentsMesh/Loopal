//! End-to-end completion contract through a real agent server and mock LLM.

use std::sync::Arc;
use std::time::Duration;

use loopal_agent_client::{AgentClient, StartAgentParams};
use loopal_agent_hub::Hub;
use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::{
    AgentEvent, AgentEventPayload, ControlCommand, Envelope, GateCloseReason, MessageSource,
    WAIT_AGENT_TYPED_RESPONSE_V1, WaitAgentResponse, WaitAgentStatus,
};
use loopal_test_support::TestFixture;
use loopal_test_support::mock_provider::{HangingProvider, MultiCallProvider};
use tokio::sync::{Mutex, mpsc};

const E2E_TIMEOUT: Duration = Duration::from_secs(10);

async fn run_mock_llm_through_hub(
    name: &str,
    calls: loopal_test_support::scenarios::Calls,
) -> WaitAgentResponse {
    let fixture = TestFixture::new();
    let provider =
        Arc::new(MultiCallProvider::new(calls)) as Arc<dyn loopal_provider_api::Provider>;
    let (server_transport, hub_transport) = loopal_ipc::duplex_pair();
    let cwd = fixture.path().to_path_buf();
    let session_dir = fixture.path().join("sessions");
    let server_task = tokio::spawn(async move {
        loopal_agent_server::run_server_for_test(server_transport, provider, cwd, session_dir).await
    });

    let client = AgentClient::new(hub_transport);
    client.initialize().await.expect("initialize agent server");
    let (connection, incoming_rx) = client.into_parts();

    let (event_tx, _event_rx) = mpsc::channel::<AgentEvent>(64);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    register_agent_connection(
        hub.clone(),
        name,
        connection.clone(),
        incoming_rx,
        None,
        None,
        None,
    )
    .await
    .expect("register real agent-server connection in Hub");

    let wait_hub = hub.clone();
    let wait_name = name.to_string();
    let wait_task = tokio::spawn(async move {
        loopal_agent_hub::dispatch::dispatch_hub_request(
            &wait_hub,
            methods::HUB_WAIT_AGENT.name,
            serde_json::json!({
                "name": wait_name,
                "response_format": WAIT_AGENT_TYPED_RESPONSE_V1,
            }),
            "parent".into(),
        )
        .await
    });

    AgentClient::start_agent_on(
        &connection,
        &StartAgentParams {
            cwd: fixture.path().to_path_buf(),
            prompt: Some("perform the assigned work".into()),
            lifecycle: Some("ephemeral".into()),
            ..Default::default()
        },
        E2E_TIMEOUT,
    )
    .await
    .expect("start ephemeral mock-LLM agent");

    let value = tokio::time::timeout(E2E_TIMEOUT, wait_task)
        .await
        .expect("hub/wait_agent must not hang")
        .expect("wait task must not panic")
        .expect("hub/wait_agent request must succeed");
    let response: WaitAgentResponse =
        serde_json::from_value(value).expect("Hub must return the typed wait_agent contract");

    tokio::time::timeout(E2E_TIMEOUT, server_task)
        .await
        .expect("real agent server must exit after ephemeral completion")
        .expect("real agent server task must not panic")
        .expect("real agent server must exit cleanly");
    response
}

#[tokio::test]
async fn mock_llm_goal_completion_reaches_hub_wait_agent_as_success() {
    let response = run_mock_llm_through_hub(
        "goal-worker",
        loopal_test_support::scenarios::simple_text("authoritative mock-LLM result"),
    )
    .await;

    assert_eq!(response.status, WaitAgentStatus::Completed);
    assert_eq!(response.reason, "goal");
    assert_eq!(response.output, "authoritative mock-LLM result");
    assert!(!response.timed_out);
}

#[tokio::test]
async fn mock_llm_fatal_error_with_partial_stream_fails_closed_at_hub_wait_agent() {
    let response = run_mock_llm_through_hub(
        "failed-worker",
        vec![vec![
            loopal_test_support::chunks::text("partial output before failure"),
            loopal_test_support::chunks::non_retryable_error("fatal mock provider error"),
        ]],
    )
    .await;

    assert_eq!(response.status, WaitAgentStatus::Failed);
    assert_eq!(response.reason, "error");
    assert_eq!(response.output, "partial output before failure");
    assert!(!response.timed_out);
}

#[tokio::test]
async fn mock_llm_empty_provider_error_cannot_reach_hub_as_goal() {
    let response = run_mock_llm_through_hub(
        "google-blocked-worker",
        vec![vec![loopal_test_support::chunks::non_retryable_error(
            "google prompt blocked: SAFETY",
        )]],
    )
    .await;

    assert_eq!(response.status, WaitAgentStatus::Failed);
    assert_eq!(response.reason, "error");
    assert!(response.output.contains("SAFETY"), "response: {response:?}");
    assert!(!response.timed_out);
}

#[tokio::test]
async fn hub_control_deadline_preserves_queued_suspend_until_runtime_boundary() {
    let fixture = TestFixture::new();
    let provider = Arc::new(HangingProvider) as Arc<dyn loopal_provider_api::Provider>;
    let (server_transport, hub_transport) = loopal_ipc::duplex_pair();
    let cwd = fixture.path().to_path_buf();
    let session_dir = fixture.path().join("sessions");
    let server_task = tokio::spawn(async move {
        loopal_agent_server::run_server_for_test(server_transport, provider, cwd, session_dir).await
    });

    let client = AgentClient::new(hub_transport);
    client.initialize().await.expect("initialize agent server");
    let (connection, incoming_rx) = client.into_parts();
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(64);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let agent_name = "queued-control-worker";
    register_agent_connection(
        hub.clone(),
        agent_name,
        connection.clone(),
        incoming_rx,
        None,
        None,
        None,
    )
    .await
    .expect("register real agent-server connection in Hub");
    AgentClient::start_agent_on(
        &connection,
        &StartAgentParams {
            cwd: fixture.path().to_path_buf(),
            lifecycle: Some("persistent".into()),
            ..Default::default()
        },
        E2E_TIMEOUT,
    )
    .await
    .expect("start persistent hanging agent");
    wait_for_payload(&mut event_rx, |payload| {
        matches!(payload, AgentEventPayload::AwaitingInput)
    })
    .await;

    loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub,
        methods::HUB_ROUTE.name,
        serde_json::to_value(Envelope::new(
            MessageSource::Human,
            agent_name,
            "keep the provider busy",
        ))
        .unwrap(),
        "ui".into(),
    )
    .await
    .expect("route kickoff through Hub");
    wait_for_payload(&mut event_rx, |payload| {
        matches!(payload, AgentEventPayload::Running)
    })
    .await;

    let response = tokio::time::timeout(
        E2E_TIMEOUT,
        loopal_agent_hub::dispatch::dispatch_hub_request(
            &hub,
            methods::HUB_CONTROL.name,
            serde_json::json!({
                "target": agent_name,
                "command": serde_json::to_value(ControlCommand::Suspend).unwrap(),
            }),
            "ui".into(),
        ),
    )
    .await
    .expect("Hub control deadline fired before queued response")
    .expect("Hub must forward the accepted queued response");
    assert_eq!(response["status"], "queued");
    assert!(
        connection.is_connected(),
        "Hub must not close a connection carrying an accepted queued control"
    );

    loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub,
        methods::HUB_INTERRUPT.name,
        serde_json::json!({"target": agent_name}),
        "ui".into(),
    )
    .await
    .expect("interrupt hanging provider through Hub");
    wait_for_payload(&mut event_rx, |payload| {
        matches!(
            payload,
            AgentEventPayload::ContinuationGateChanged(summary)
                if !summary.open
                    && summary.closed_reason == Some(GateCloseReason::UserSuspend)
        )
    })
    .await;

    loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub,
        methods::HUB_SHUTDOWN_AGENT.name,
        serde_json::json!({"target": agent_name}),
        "ui".into(),
    )
    .await
    .expect("shutdown real agent server");
    tokio::time::timeout(E2E_TIMEOUT, server_task)
        .await
        .expect("real agent server must stop")
        .expect("real agent server task must not panic")
        .expect("real agent server must exit cleanly");
}

async fn wait_for_payload(
    events: &mut mpsc::Receiver<AgentEvent>,
    predicate: impl Fn(&AgentEventPayload) -> bool,
) {
    tokio::time::timeout(E2E_TIMEOUT, async {
        while let Some(event) = events.recv().await {
            if predicate(&event.payload) {
                return;
            }
        }
        panic!("Hub agent event stream closed");
    })
    .await
    .expect("expected Hub agent event");
}
