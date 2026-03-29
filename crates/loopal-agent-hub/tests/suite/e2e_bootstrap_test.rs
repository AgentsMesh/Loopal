//! End-to-end bootstrap test: real Hub → real agent process → message roundtrip.
//!
//! Uses LOOPAL_TEST_PROVIDER to inject mock LLM responses into the real
//! agent process, verifying the full Hub→stdio→AgentServer→AgentLoop chain.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_client::AgentProcess;
use loopal_agent_hub::Hub;
use loopal_agent_hub::agent_io;
use loopal_ipc::protocol::methods;
use loopal_protocol::AgentEvent;
use serde_json::json;

/// Full bootstrap e2e: Hub spawns real agent process with mock provider,
/// agent starts, emits AwaitingInput, TUI sends message, agent responds.
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
        &[],
        &[("LOOPAL_TEST_PROVIDER", mock_file.to_str().unwrap())],
    )
    .await
    .expect("should spawn agent process");

    let client = loopal_agent_client::AgentClient::new(agent_proc.transport());
    client.initialize().await.expect("initialize should work");

    let cwd = std::env::temp_dir();
    client
        .start_agent(
            &cwd,
            None, // use default model
            Some("act"),
            None, // no initial prompt
            None,
            true, // no sandbox
            None,
        )
        .await
        .expect("start_agent should work");

    // 4. Register root agent stdio in Hub
    let (root_conn, incoming_rx) = client.into_parts();
    agent_io::start_agent_io(hub.clone(), "main", root_conn.clone(), incoming_rx, true);

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

/// Find the loopal binary. Checks LOOPAL_BINARY env var first (set by Bazel),
/// then falls back to Cargo target directory layout.
fn resolve_loopal_binary() -> String {
    if let Ok(path) = std::env::var("LOOPAL_BINARY") {
        if std::path::Path::new(&path).exists() {
            return path;
        }
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
