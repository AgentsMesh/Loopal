//! End-to-end bootstrap test: real Hub → real agent process → message roundtrip.
//!
//! Uses LOOPAL_TEST_PROVIDER to inject mock LLM responses into the real
//! agent process, verifying the full Hub→stdio→AgentServer→AgentLoop chain.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_client::AgentProcess;
use loopal_agent_hub::agent_io;
use loopal_agent_hub::{Hub, UiSession};
use loopal_ipc::protocol::methods;
use loopal_protocol::{
    AgentEvent, AgentEventPayload, Envelope, MessageSource, UiCapabilities, UserContent,
};
use serde_json::json;

/// Full bootstrap e2e: Hub spawns real agent process with mock provider,
/// agent starts, emits AwaitingInput, TUI sends message, agent responds.
#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn full_bootstrap_hub_to_agent_roundtrip() {
    // 1. Create mock provider JSON file
    let mock_file =
        std::env::temp_dir().join(format!("loopal_e2e_mock_{}.json", std::process::id()));
    let mock_data = json!([
        [
            {"type": "text", "text": "Hello from mock agent!"},
            {"type": "usage", "input": 10, "output": 5},
            {"type": "done"}
        ]
    ]);
    std::fs::write(&mock_file, serde_json::to_string(&mock_data).unwrap()).unwrap();

    // 2. Start Hub
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(256);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));

    // 3. Spawn real agent process with mock provider
    // Resolve loopal binary from target directory (same profile as this test)
    let exe = resolve_loopal_binary();
    let agent_proc = AgentProcess::spawn_with_env(
        Some(&exe),
        &[("LOOPAL_TEST_PROVIDER", mock_file.to_str().unwrap())],
    )
    .await
    .expect("should spawn agent process");

    let client = loopal_agent_client::AgentClient::new(agent_proc.transport());
    client.initialize().await.expect("initialize should work");

    let cwd = std::env::temp_dir();
    client
        .start_agent(&loopal_agent_client::StartAgentParams {
            cwd: cwd.clone(),
            // Keep the fixture independent of the developer's configured
            // default model; JsonMockProvider registers as "anthropic".
            model: Some("claude-opus-4-8".to_string()),
            mode: Some("act".to_string()),
            no_sandbox: true,
            ..Default::default()
        })
        .await
        .expect("start_agent should work");

    // 4. Register root agent stdio in Hub
    let (root_conn, incoming_rx) = client.into_parts();
    let dispatcher = std::sync::Arc::new(loopal_agent_hub::dispatch::build_hub_dispatcher(
        hub.clone(),
    ));
    agent_io::start_agent_io(
        hub.clone(),
        dispatcher,
        "main",
        root_conn.clone(),
        incoming_rx,
        None,
    );

    // 5. Wait for AwaitingInput event (agent is ready for input)
    let mut got_awaiting = false;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        while let Ok(event) = event_rx.try_recv() {
            if matches!(
                event.payload,
                loopal_protocol::AgentEventPayload::AwaitingInput
            ) {
                got_awaiting = true;
            }
        }
        if got_awaiting {
            break;
        }
    }
    assert!(got_awaiting, "should receive AwaitingInput from agent");

    // 6. Send user message via Hub route (agent/message)
    let envelope = loopal_protocol::Envelope::new(
        loopal_protocol::MessageSource::Human,
        "main",
        "What is 2+2?",
    );
    let params = serde_json::to_value(&envelope).unwrap();
    let conn = hub
        .lock()
        .await
        .registry
        .get_agent_connection("main")
        .unwrap();
    conn.send_request(methods::AGENT_MESSAGE.name, params)
        .await
        .expect("should deliver message to agent");

    // 7. Collect agent response events (Stream text + Done)
    let mut collected_text = String::new();
    let mut got_stream = false;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        while let Ok(event) = event_rx.try_recv() {
            if let loopal_protocol::AgentEventPayload::Stream { text } = event.payload {
                collected_text.push_str(&text);
                got_stream = true;
            }
        }
        if got_stream {
            break;
        }
    }
    assert!(
        collected_text.contains("Hello from mock agent!"),
        "should receive mock response, got: '{collected_text}'"
    );

    // 8. Cleanup
    let _ = agent_proc.shutdown().await;
    let _ = std::fs::remove_file(&mock_file);
}

/// Interrupting a real ExitPlanMode approval must cross every ownership
/// boundary and return a typed cancelled ToolResult instead of stranding the
/// runtime on the agent -> Hub approval RPC.
#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn exit_plan_interrupt_unblocks_full_hub_runtime_chain() {
    let temp = tempfile::tempdir().expect("create e2e tempdir");
    let mock_file = temp.path().join("exit_plan_interrupt.json");
    let session_dir = temp.path().join("sessions");
    let mock_data = json!([
        [
            {"type": "tool_use", "id": "p1", "name": "ExitPlanMode", "input": {}},
            {"type": "done"}
        ],
        [
            {"type": "text", "text": "write the plan first"},
            {"type": "done"}
        ],
        [
            {"type": "tool_use", "id": "p2", "name": "ExitPlanMode", "input": {}},
            {"type": "done"}
        ]
    ]);
    std::fs::write(&mock_file, serde_json::to_vec(&mock_data).unwrap())
        .expect("write mock provider fixture");

    let (event_tx, raw_rx) = mpsc::channel::<AgentEvent>(256);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let event_loop = loopal_agent_hub::start_event_loop(hub.clone(), raw_rx);
    let mut ui = UiSession::connect(hub.clone(), "exit-plan-e2e", UiCapabilities::ALL).await;

    let exe = resolve_loopal_binary();
    let mock_path = mock_file.to_string_lossy().into_owned();
    let session_path = session_dir.to_string_lossy().into_owned();
    let mut agent_proc = AgentProcess::spawn_with_env(
        Some(&exe),
        &[
            ("LOOPAL_TEST_PROVIDER", mock_path.as_str()),
            ("LOOPAL_TEST_SESSION_DIR", session_path.as_str()),
        ],
    )
    .await
    .expect("spawn real agent process");
    let client = loopal_agent_client::AgentClient::new(agent_proc.transport());
    client.initialize().await.expect("initialize agent process");
    client
        .start_agent(&loopal_agent_client::StartAgentParams {
            cwd: temp.path().to_path_buf(),
            model: Some("claude-opus-4-8".to_string()),
            mode: Some("plan".to_string()),
            permission_mode: Some("bypass".to_string()),
            no_sandbox: true,
            ..Default::default()
        })
        .await
        .expect("start real agent runtime");

    let (root_conn, incoming_rx) = client.into_parts();
    let dispatcher = Arc::new(loopal_agent_hub::dispatch::build_hub_dispatcher(
        hub.clone(),
    ));
    agent_io::start_agent_io(
        hub.clone(),
        dispatcher,
        "main",
        root_conn,
        incoming_rx,
        None,
    );

    recv_event_matching(&mut ui.event_rx, Duration::from_secs(5), |payload| {
        matches!(payload, AgentEventPayload::AwaitingInput)
    })
    .await;

    route_user_message(&ui, "locate the runtime plan file").await;
    let blocked = recv_event_matching(&mut ui.event_rx, Duration::from_secs(5), |payload| {
        matches!(
            payload,
            AgentEventPayload::ToolResult { id, name, .. }
                if id == "p1" && name == "ExitPlanMode"
        )
    })
    .await;
    let plan_path = match blocked.payload {
        AgentEventPayload::ToolResult {
            result, is_error, ..
        } => {
            assert!(is_error, "ExitPlanMode without a plan must be blocked");
            parse_missing_plan_path(&result)
        }
        _ => unreachable!(),
    };
    std::fs::create_dir_all(plan_path.parent().expect("plan path parent"))
        .expect("create runtime-selected plan directory");
    std::fs::write(&plan_path, "# E2E Plan\n1. verify interrupt propagation\n")
        .expect("write runtime-selected plan file");

    recv_event_matching(&mut ui.event_rx, Duration::from_secs(5), |payload| {
        matches!(payload, AgentEventPayload::AwaitingInput)
    })
    .await;
    route_user_message(&ui, "exit plan mode now").await;

    let approval = recv_event_matching(&mut ui.event_rx, Duration::from_secs(5), |payload| {
        matches!(payload, AgentEventPayload::PlanApprovalRequest { .. })
    })
    .await;
    let interaction_id = match approval.payload {
        AgentEventPayload::PlanApprovalRequest {
            id,
            plan_content,
            plan_path: requested_path,
        } => {
            assert!(plan_content.contains("verify interrupt propagation"));
            assert_eq!(std::path::Path::new(&requested_path), plan_path);
            id
        }
        _ => unreachable!(),
    };
    let logical_id = {
        let h = hub.lock().await;
        let ((agent, logical_id), info) = h
            .pending_plan_approvals
            .iter()
            .find(|((agent, _), info)| agent == "main" && info.interaction_id == interaction_id)
            .expect("Hub must own the pending plan approval before interrupt");
        assert_eq!(agent, "main");
        assert_eq!(info.agent_name, "main");
        logical_id.clone()
    };

    let interrupt_ack = tokio::time::timeout(
        Duration::from_secs(2),
        ui.client
            .connection()
            .send_request(methods::HUB_INTERRUPT.name, json!({"target": "main"})),
    )
    .await
    .expect("Hub interrupt request must be acknowledged within two seconds")
    .expect("Hub interrupt request must succeed");
    assert_eq!(interrupt_ack["ok"], true);
    {
        let h = hub.lock().await;
        assert!(
            !h.pending_plan_approvals
                .contains_key(&("main".to_string(), logical_id)),
            "Hub interrupt ACK must imply pending approval cleanup"
        );
        assert!(h.pending_plan_approvals.is_empty());
    }

    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let mut got_approval_resolved = false;
    let mut got_cancelled_result = false;
    let mut got_turn_terminal = false;
    while !(got_approval_resolved && got_cancelled_result && got_turn_terminal) {
        let event = tokio::time::timeout_at(deadline, ui.event_rx.recv())
            .await
            .expect("cancelled ToolResult and turn terminal must arrive within two seconds")
            .expect("UI event stream closed while waiting for cancellation");
        match event.payload {
            AgentEventPayload::PlanApprovalResolved { id } if id == interaction_id => {
                got_approval_resolved = true;
            }
            AgentEventPayload::ToolResult {
                id,
                name,
                result,
                is_error,
                metadata,
                ..
            } if id == "p2" && name == "ExitPlanMode" => {
                assert!(is_error);
                assert_eq!(result, "Interrupted by user");
                assert_eq!(
                    serde_json::to_value(metadata.expect("typed cancellation metadata")).unwrap(),
                    json!({"kind": "cancelled", "cause": "user_interrupt"})
                );
                got_cancelled_result = true;
            }
            AgentEventPayload::TurnCancelled { cause } => {
                assert_eq!(cause, "UserInterrupt");
                got_turn_terminal = true;
            }
            AgentEventPayload::AwaitingInput => got_turn_terminal = true,
            _ => {}
        }
    }

    assert!(
        agent_proc.is_running(),
        "interrupting a blocked approval must not kill the agent process"
    );
    let shutdown_ack = tokio::time::timeout(
        Duration::from_secs(2),
        ui.client
            .connection()
            .send_request(methods::HUB_SHUTDOWN_AGENT.name, json!({"target": "main"})),
    )
    .await
    .expect("agent shutdown request must not hang after interrupted approval")
    .expect("agent shutdown request must succeed");
    assert_eq!(shutdown_ack["ok"], true);
    agent_proc
        .shutdown()
        .await
        .expect("real agent process must shut down cleanly");
    event_loop.abort();
}

async fn route_user_message(ui: &UiSession, text: &str) {
    let envelope = Envelope::new(MessageSource::Human, "main", UserContent::text_only(text));
    ui.client
        .route_envelope(&envelope)
        .await
        .expect("route user message through Hub");
}

async fn recv_event_matching(
    rx: &mut tokio::sync::broadcast::Receiver<AgentEvent>,
    timeout: Duration,
    mut predicate: impl FnMut(&AgentEventPayload) -> bool,
) -> AgentEvent {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let event = tokio::time::timeout_at(deadline, rx.recv())
            .await
            .expect("timed out waiting for matching agent event")
            .expect("UI event stream closed");
        if predicate(&event.payload) {
            return event;
        }
    }
}

fn parse_missing_plan_path(result: &str) -> std::path::PathBuf {
    let path = result
        .split_once("No plan file at ")
        .and_then(|(_, rest)| rest.split_once(". Write your plan"))
        .map(|(path, _)| path)
        .expect("blocked ExitPlanMode result must name the runtime plan path");
    std::path::PathBuf::from(path)
}

/// Find the loopal binary. Checks LOOPAL_BINARY env var first (set by Bazel),
/// then falls back to Cargo target directory layout.
fn resolve_loopal_binary() -> String {
    if let Ok(path) = std::env::var("LOOPAL_BINARY")
        && std::path::Path::new(&path).exists()
    {
        return path;
    }
    let test_exe = std::env::current_exe().expect("current_exe");
    let target_dir = test_exe
        .parent() // deps/
        .and_then(|p| p.parent()) // debug/ or release/
        .expect("target dir");
    let binary_name = format!("loopal{}", std::env::consts::EXE_SUFFIX);
    let loopal = target_dir.join(binary_name);
    assert!(
        loopal.exists(),
        "loopal binary not found at {}. Set LOOPAL_BINARY or run `cargo build` first.",
        loopal.display()
    );
    loopal.to_string_lossy().to_string()
}
