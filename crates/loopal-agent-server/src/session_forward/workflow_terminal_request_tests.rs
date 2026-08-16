use loopal_ipc::protocol::methods;
use loopal_protocol::WorkflowTerminalDisposition;
use loopal_runtime::agent_input::AgentInput;

use super::forwarding_test_support::{peers, pending_handle, session, terminal_notification};
use super::{ForwardResult, forward_loop};

#[tokio::test]
async fn workflow_terminal_request_uses_tracked_forwarding_path() {
    let ((server, mut server_rx), (client, _client_rx)) = peers();
    let (session, mut input_rx) = session("terminal-session", 4);
    let mut handle = pending_handle(session);

    let (outcome, disposition) =
        tokio::join!(forward_loop(&mut server_rx, &server, &mut handle), async {
            let terminal_request = client.send_request(
                methods::AGENT_WORKFLOW_TERMINAL.name,
                serde_json::to_value(terminal_notification("terminal-session")).unwrap(),
            );
            let acknowledge = async {
                let AgentInput::WorkflowTerminal(request) = input_rx.recv().await.unwrap() else {
                    panic!("expected workflow terminal")
                };
                request
                    .acknowledge(WorkflowTerminalDisposition::Applied)
                    .await;
            };
            let (response, ()) = tokio::join!(terminal_request, acknowledge);
            let disposition: WorkflowTerminalDisposition =
                serde_json::from_value(response.unwrap()).unwrap();
            assert_eq!(
                client
                    .send_request(methods::AGENT_SHUTDOWN.name, serde_json::json!({}))
                    .await
                    .unwrap()["ok"],
                true
            );
            disposition
        });

    assert!(matches!(outcome, ForwardResult::Shutdown));
    assert_eq!(disposition, WorkflowTerminalDisposition::Applied);
    handle.agent_task.abort();
}
