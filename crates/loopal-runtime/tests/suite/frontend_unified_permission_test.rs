use loopal_protocol::{ControlCommand, Envelope};

use super::permission_request_support::permission_request;
use loopal_runtime::frontend::AgentFrontend;
use loopal_runtime::frontend::{DenyAllHandler, UnifiedFrontend, UnsupportedQuestionHandler};
use loopal_tool_api::PermissionDecision;
use tokio::sync::mpsc;

fn make_unified(
    agent_name: Option<String>,
    event_tx: mpsc::Sender<loopal_protocol::AgentEvent>,
    mailbox_rx: mpsc::Receiver<Envelope>,
    control_rx: mpsc::Receiver<ControlCommand>,
    handler: Box<dyn loopal_runtime::frontend::PermissionHandler>,
) -> UnifiedFrontend {
    UnifiedFrontend::new(
        agent_name,
        event_tx,
        mailbox_rx,
        control_rx,
        None,
        handler,
        Box::new(UnsupportedQuestionHandler),
    )
}

#[tokio::test]
async fn test_unified_permission_auto_deny() {
    let (event_tx, _event_rx) = mpsc::channel(16);
    let (_mb_tx, mb_rx) = mpsc::channel(16);
    let (_ctrl_tx, ctrl_rx) = mpsc::channel(16);

    let f = make_unified(
        Some("sub".into()),
        event_tx,
        mb_rx,
        ctrl_rx,
        Box::new(DenyAllHandler),
    );
    let request = permission_request("id1", "Bash", serde_json::json!({}));
    let d = f.request_permission(&request).await;
    assert_eq!(d, PermissionDecision::Deny);
}
