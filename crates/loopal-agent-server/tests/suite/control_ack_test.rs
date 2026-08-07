use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_protocol::{
    AgentEvent, AgentEventPayload, AgentMode, ControlCommand, ControlDisposition, Envelope,
};
use loopal_test_support::TestFixture;
use loopal_test_support::mock_provider::{HangingProvider, MultiCallProvider};

use super::control_ack_transport::ControlAckTransport;

const WAIT: Duration = Duration::from_secs(10);
const DISPATCH_WAIT: Duration = Duration::from_secs(4);

#[tokio::test]
async fn control_ack_follows_application_and_rejection_keeps_agent_alive() {
    let provider = Arc::new(MultiCallProvider::new(vec![]));
    let (connection, mut incoming, _fixture, _) = server(provider).await;
    initialize_and_start(&connection).await;
    wait_for(&mut incoming, |payload| {
        matches!(payload, AgentEventPayload::AwaitingInput)
    })
    .await;

    let applied = control(&connection, ControlCommand::ModeSwitch(AgentMode::Plan))
        .await
        .expect("mode control");
    assert_eq!(applied["status"], "applied");
    wait_for(
        &mut incoming,
        |payload| matches!(payload, AgentEventPayload::ModeChanged { mode } if mode == "plan"),
    )
    .await;

    let rejected = control(
        &connection,
        ControlCommand::DecisionModeSwitch("agent".into()),
    )
    .await
    .expect("application rejection is a typed disposition");
    let ControlDisposition::Rejected { reason } =
        ControlDisposition::from_wire_value(rejected).unwrap()
    else {
        panic!("unsupported decision mode must return rejected disposition");
    };
    assert!(reason.contains("not implemented"), "reason: {reason}");

    let alive = control(&connection, ControlCommand::ModeSwitch(AgentMode::Act))
        .await
        .expect("agent remains controllable");
    assert_eq!(alive["status"], "applied");
    connection.close().await;
}

#[tokio::test]
async fn pending_control_does_not_block_other_requests_and_drops_on_disconnect() {
    let provider = Arc::new(HangingProvider);
    let (connection, mut incoming, _fixture, control_sent) = server(provider).await;
    initialize_and_start(&connection).await;
    wait_for(&mut incoming, |payload| {
        matches!(payload, AgentEventPayload::AwaitingInput)
    })
    .await;
    let envelope = Envelope::new(loopal_protocol::MessageSource::Human, "main", "stay busy");
    connection
        .send_request(
            methods::AGENT_MESSAGE.name,
            serde_json::to_value(envelope).unwrap(),
        )
        .await
        .unwrap();
    wait_for(&mut incoming, |payload| {
        matches!(payload, AgentEventPayload::Running)
    })
    .await;

    let pending_connection = connection.clone();
    let pending = tokio::spawn(async move {
        control(
            &pending_connection,
            ControlCommand::ModeSwitch(AgentMode::Plan),
        )
        .await
    });
    tokio::time::timeout(WAIT, control_sent.notified())
        .await
        .expect("control request frame was not sent");
    tokio::time::timeout(
        DISPATCH_WAIT,
        connection.send_request(methods::AGENT_STATE_SNAPSHOT.name, serde_json::json!({})),
    )
    .await
    .expect("pending control blocked request dispatch")
    .expect("state snapshot request failed");

    connection.close().await;
    let outcome = tokio::time::timeout(WAIT, pending).await;
    assert!(
        outcome.is_ok(),
        "disconnected control caller must be released"
    );
}

#[tokio::test]
async fn suspend_queued_past_ack_deadline_is_applied_after_inflight_turn_cancels() {
    let provider = Arc::new(HangingProvider);
    let (connection, mut incoming, _fixture, control_sent) = server(provider).await;
    initialize_and_start(&connection).await;
    wait_for(&mut incoming, |payload| {
        matches!(payload, AgentEventPayload::AwaitingInput)
    })
    .await;
    let envelope = Envelope::new(loopal_protocol::MessageSource::Human, "main", "stay busy");
    connection
        .send_request(
            methods::AGENT_MESSAGE.name,
            serde_json::to_value(envelope).unwrap(),
        )
        .await
        .unwrap();
    wait_for(&mut incoming, |payload| {
        matches!(payload, AgentEventPayload::Running)
    })
    .await;

    let pending_connection = connection.clone();
    let pending =
        tokio::spawn(async move { control(&pending_connection, ControlCommand::Suspend).await });
    tokio::time::timeout(WAIT, control_sent.notified())
        .await
        .expect("suspend control request frame was not sent");
    let accepted = tokio::time::timeout(Duration::from_secs(12), pending)
        .await
        .expect("control must return queued after its application deadline")
        .unwrap()
        .expect("queued control response");
    assert_eq!(accepted["status"], "queued");

    connection
        .send_request(methods::AGENT_INTERRUPT.name, serde_json::Value::Null)
        .await
        .unwrap();
    wait_for(&mut incoming, |payload| {
        matches!(
            payload,
            AgentEventPayload::ContinuationGateChanged(summary)
                if !summary.open
                    && summary.closed_reason
                        == Some(loopal_protocol::GateCloseReason::UserSuspend)
        )
    })
    .await;
    connection.close().await;
}

async fn server(
    provider: Arc<dyn loopal_provider_api::Provider>,
) -> (
    Arc<Connection<Listening>>,
    tokio::sync::mpsc::Receiver<Incoming>,
    TestFixture,
    Arc<tokio::sync::Notify>,
) {
    let fixture = TestFixture::new();
    let (agent_write, client_read) = tokio::io::duplex(16_384);
    let (client_write, agent_read) = tokio::io::duplex(16_384);
    let agent: Arc<dyn Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(agent_read)),
        Box::new(agent_write),
    ));
    let client: Arc<dyn Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(client_read)),
        Box::new(client_write),
    ));
    let (client, control_sent) = ControlAckTransport::wrap(client);
    let cwd = fixture.path().to_path_buf();
    let sessions = fixture.path().join("sessions");
    tokio::spawn(async move {
        let _ = loopal_agent_server::run_server_for_test(agent, provider, cwd, sessions).await;
    });
    let (connection, incoming) = Connection::new(client).into_listening();
    (connection, incoming, fixture, control_sent)
}

async fn initialize_and_start(connection: &Connection<Listening>) {
    tokio::time::timeout(
        WAIT,
        connection.send_request("initialize", serde_json::json!({"protocol_version": 1})),
    )
    .await
    .unwrap()
    .unwrap();
    tokio::time::timeout(
        WAIT,
        connection.send_request(methods::AGENT_START.name, serde_json::json!({})),
    )
    .await
    .unwrap()
    .unwrap();
}

async fn control(
    connection: &Connection<Listening>,
    command: ControlCommand,
) -> Result<serde_json::Value, loopal_ipc::RpcError> {
    connection
        .send_request(
            methods::AGENT_CONTROL.name,
            serde_json::to_value(command).unwrap(),
        )
        .await
}

async fn wait_for(
    incoming: &mut tokio::sync::mpsc::Receiver<Incoming>,
    predicate: impl Fn(&AgentEventPayload) -> bool,
) {
    tokio::time::timeout(WAIT, async {
        while let Some(Incoming::Notification { method, params }) = incoming.recv().await {
            if method != methods::AGENT_EVENT.name {
                continue;
            }
            let event: AgentEvent = serde_json::from_value(params).unwrap();
            if predicate(&event.payload) {
                return;
            }
        }
        panic!("agent event stream closed");
    })
    .await
    .expect("expected agent event");
}
