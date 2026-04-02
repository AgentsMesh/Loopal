//! Full chain test: spawn-like flow where sub-agent result is delivered to parent.
//! Simulates the exact flow inside spawn_agent without spawning a real process.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use loopal_agent::bridge::bridge_child_events;
use loopal_agent_client::AgentClient;
use loopal_protocol::AgentEvent;
use loopal_test_support::TestFixture;
use loopal_test_support::mock_provider::MultiCallProvider;
use loopal_test_support::scenarios;

use super::bridge_child_test::make_duplex_pair;

const T: Duration = Duration::from_secs(10);

/// The most fundamental scenario: sub-agent finishes -> result delivered to parent
/// via oneshot channel -> parent can continue. Simulates the exact flow inside
/// spawn_agent (bridge task + result_tx/rx) without spawning a real process.
#[tokio::test]
async fn full_chain_sub_agent_result_delivered_to_parent() {
    let fixture = TestFixture::new();
    let cwd = fixture.path().to_path_buf();
    let session_dir = fixture.path().join("sessions");
    let provider = Arc::new(MultiCallProvider::new(scenarios::simple_text(
        "# Research Report\n\nFound 42 crates with 200k lines.",
    ))) as Arc<dyn loopal_provider_api::Provider>;

    let (server_t, client_t) = make_duplex_pair();
    tokio::spawn(async move {
        let _ =
            loopal_agent_server::run_server_for_test(server_t, provider, cwd, session_dir).await;
    });

    // Simulate spawn_agent internals
    let client = AgentClient::new(client_t);
    client.initialize().await.expect("initialize");
    client
        .start_agent(
            fixture.path(),
            None,
            None,
            Some("research this project"),
            None,
            false,
            None,
            None,
        )
        .await
        .expect("start_agent");

    let (result_tx, result_rx) = tokio::sync::oneshot::channel();
    let (event_tx, _event_rx) = mpsc::channel::<AgentEvent>(16);
    let cancel = CancellationToken::new();

    // Bridge task — same as spawn_agent's join_handle
    tokio::spawn(async move {
        let result = bridge_child_events(client, &event_tx, "researcher", &cancel).await;
        let _ = result_tx.send(result);
    });

    // Simulate Agent tool's handle_spawn_result
    let result = tokio::time::timeout(T, result_rx)
        .await
        .expect("result should arrive within timeout")
        .expect("oneshot channel should not be dropped")
        .expect("bridge should succeed");

    // Parent got the sub-agent's stream output
    assert!(
        result.contains("Research Report"),
        "parent should receive sub-agent's result, got: {}",
        &result[..result.len().min(100)]
    );
    assert!(result.contains("42 crates"));
}
