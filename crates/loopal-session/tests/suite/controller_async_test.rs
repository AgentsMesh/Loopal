use loopal_protocol::{ControlCommand, UserQuestionResponse};
use loopal_session::SessionController;
use tokio::sync::mpsc;

fn make_controller() -> (
    SessionController,
    mpsc::Receiver<ControlCommand>,
    mpsc::Receiver<bool>,
) {
    let (control_tx, control_rx) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, perm_rx) = mpsc::channel::<bool>(16);
    let (question_tx, _question_rx) = mpsc::channel::<UserQuestionResponse>(16);
    let ctrl = SessionController::new(
        control_tx,
        perm_tx,
        question_tx,
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
    );
    (ctrl, control_rx, perm_rx)
}

#[tokio::test]
async fn respond_permission_local_sends_true() {
    let (ctrl, _, mut perm_rx) = make_controller();
    ctrl.respond_permission("main", "tc-1", None, true).await;
    assert_eq!(perm_rx.recv().await, Some(true));
}

#[tokio::test]
async fn respond_permission_local_sends_false() {
    let (ctrl, _, mut perm_rx) = make_controller();
    ctrl.respond_permission("main", "tc-1", None, false).await;
    assert_eq!(perm_rx.recv().await, Some(false));
}

#[tokio::test]
async fn switch_mode_dispatches_control() {
    let (ctrl, mut control_rx, _) = make_controller();
    ctrl.switch_mode(loopal_protocol::AgentMode::Plan).await;
    match control_rx.recv().await {
        Some(ControlCommand::ModeSwitch(m)) => {
            assert!(matches!(m, loopal_protocol::AgentMode::Plan));
        }
        other => panic!("expected ModeSwitch, got {other:?}"),
    }
}

#[tokio::test]
async fn switch_model_dispatches_control() {
    let (ctrl, mut control_rx, _) = make_controller();
    ctrl.switch_model("gpt-4".to_string()).await;
    match control_rx.recv().await {
        Some(ControlCommand::ModelSwitch(m)) => assert_eq!(m, "gpt-4"),
        other => panic!("expected ModelSwitch, got {other:?}"),
    }
}

#[tokio::test]
async fn clear_dispatches_control() {
    let (ctrl, mut control_rx, _) = make_controller();
    ctrl.clear().await;
    match control_rx.recv().await {
        Some(ControlCommand::Clear) => {}
        other => panic!("expected Clear, got {other:?}"),
    }
}

#[tokio::test]
async fn compact_dispatches_control() {
    let (ctrl, mut control_rx, _) = make_controller();
    ctrl.compact(None).await;
    match control_rx.recv().await {
        Some(ControlCommand::Compact { .. }) => {}
        other => panic!("expected Compact, got {other:?}"),
    }
}

#[tokio::test]
async fn hub_respond_permission_sends_request() {
    use loopal_agent_hub::HubClient;
    use std::sync::Arc;

    let (client_side, server_side) = tokio::io::duplex(4096);
    let client_transport: Arc<dyn loopal_ipc::transport::Transport> =
        Arc::new(loopal_ipc::StdioTransport::new(
            Box::new(tokio::io::BufReader::new(client_side)),
            Box::new(server_side),
        ));
    let (conn, _rx) = loopal_ipc::connection::Connection::new(client_transport).into_listening();
    let hub_client = Arc::new(HubClient::new(conn));
    let ctrl = SessionController::with_hub(hub_client);
    let _handle = tokio::spawn(async move {
        ctrl.respond_permission("main", "tc-42", None, true).await;
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
}

async fn capture_plan_response(approve: bool) -> serde_json::Value {
    use loopal_agent_hub::HubClient;
    use std::sync::Arc;
    use tokio::io::AsyncBufReadExt as _;

    let (client_end, server_end) = tokio::io::duplex(4096);
    let (client_reader, client_writer) = tokio::io::split(client_end);
    let (server_reader, _server_writer) = tokio::io::split(server_end);
    let transport: Arc<dyn loopal_ipc::transport::Transport> =
        Arc::new(loopal_ipc::StdioTransport::new(
            Box::new(tokio::io::BufReader::new(client_reader)),
            Box::new(client_writer),
        ));
    let (conn, _rx) = loopal_ipc::connection::Connection::new(transport).into_listening();
    let ctrl = SessionController::with_hub(Arc::new(HubClient::new(conn)));
    let send = tokio::spawn(async move {
        ctrl.respond_plan_approval("main", "plan-42", approve).await;
    });

    let mut line = String::new();
    tokio::time::timeout(
        std::time::Duration::from_millis(500),
        tokio::io::BufReader::new(server_reader).read_line(&mut line),
    )
    .await
    .expect("server side must receive a request")
    .expect("read ok");
    send.abort();
    serde_json::from_str(line.trim()).expect("valid JSON-RPC request")
}

#[tokio::test]
async fn hub_plan_approval_sends_approve_response() {
    let request = capture_plan_response(true).await;
    assert_eq!(request["method"], "hub/plan_approval_response");
    assert_eq!(request["params"]["agent_name"], "main");
    assert_eq!(request["params"]["request_id"], "plan-42");
    assert_eq!(request["params"]["decision"], "approve");
}

#[tokio::test]
async fn hub_plan_approval_sends_reject_response() {
    let request = capture_plan_response(false).await;
    assert_eq!(request["params"]["decision"], "reject");
    assert!(request["params"]["edited_plan"].is_null());
}

#[tokio::test]
async fn shutdown_hub_local_mode_is_noop() {
    let (ctrl, mut control_rx, mut perm_rx) = make_controller();
    ctrl.shutdown_hub().await;
    assert!(control_rx.try_recv().is_err());
    assert!(perm_rx.try_recv().is_err());
}

#[tokio::test]
async fn shutdown_hub_with_hub_backend_sends_request() {
    use loopal_agent_hub::HubClient;
    use std::sync::Arc;
    use tokio::io::AsyncReadExt as _;

    let (client_end, server_end) = tokio::io::duplex(4096);
    let (client_reader, client_writer) = tokio::io::split(client_end);
    let (mut server_reader, _server_writer) = tokio::io::split(server_end);
    let client_transport: Arc<dyn loopal_ipc::transport::Transport> =
        Arc::new(loopal_ipc::StdioTransport::new(
            Box::new(tokio::io::BufReader::new(client_reader)),
            Box::new(client_writer),
        ));
    let (conn, _rx) = loopal_ipc::connection::Connection::new(client_transport).into_listening();
    let hub_client = Arc::new(HubClient::new(conn));
    let ctrl = SessionController::with_hub(hub_client);

    let send = tokio::spawn(async move { ctrl.shutdown_hub().await });

    let mut buf = vec![0u8; 1024];
    let n = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        server_reader.read(&mut buf),
    )
    .await
    .expect("server side must receive a request")
    .expect("read ok");
    assert!(n > 0);
    let body = String::from_utf8_lossy(&buf[..n]);
    assert!(
        body.contains("\"hub/shutdown\""),
        "expected hub/shutdown in {body}"
    );
    send.abort();
}
