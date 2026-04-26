//! Integration test for the completion notification injection pipeline:
//! child agent finishes → Hub deliver_to_parent → completion bridge → IPC → parent recv_input.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::Hub;
use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, Envelope, MessageSource};
use serde_json::json;

fn make_hub() -> (Arc<Mutex<Hub>>, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel::<AgentEvent>(64);
    (Arc::new(Mutex::new(Hub::new(tx))), rx)
}

/// Helper: register an agent, return client-side connection.
async fn register_agent(
    hub: &Arc<Mutex<Hub>>,
    name: &str,
    parent: Option<&str>,
) -> (Arc<Connection>, mpsc::Receiver<Incoming>) {
    let (client_transport, server_transport) = loopal_ipc::duplex_pair();
    let client_conn = Arc::new(Connection::new(client_transport));
    let server_conn = Arc::new(Connection::new(server_transport));
    let client_rx = client_conn.start();
    let server_rx = server_conn.start();
    let _ = register_agent_connection(
        hub.clone(),
        name,
        server_conn,
        server_rx,
        parent,
        None,
        None,
    )
    .await;
    (client_conn, client_rx)
}

/// When a child completes, parent receives an agent/message notification
/// containing an Envelope with MessageSource::System.
#[tokio::test]
async fn child_completion_delivered_to_parent_via_bridge() {
    let (hub, _) = make_hub();

    // Register parent — listen on its client_rx for incoming notifications
    let (parent_conn, mut parent_rx) = register_agent(&hub, "parent", None).await;
    // Drain events from parent so the connection doesn't get congested
    let parent_msg_rx = {
        let (tx, rx) = mpsc::channel::<Envelope>(16);
        let pc = parent_conn.clone();
        tokio::spawn(async move {
            while let Some(msg) = parent_rx.recv().await {
                match msg {
                    Incoming::Notification { method, params }
                        if method == methods::AGENT_MESSAGE.name =>
                    {
                        if let Ok(env) = serde_json::from_value::<Envelope>(params) {
                            let _ = tx.send(env).await;
                        }
                    }
                    Incoming::Request { id, .. } => {
                        let _ = pc.respond(id, json!({"ok": true})).await;
                    }
                    _ => {}
                }
            }
        });
        rx
    };
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Register child under parent
    let (child_conn, _child_rx) = register_agent(&hub, "child-a", Some("parent")).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Child sends agent/completed with result
    child_conn
        .send_notification(
            methods::AGENT_COMPLETED.name,
            json!({"reason": "goal", "result": "Analysis: 42 issues found."}),
        )
        .await
        .unwrap();

    // Parent should receive an agent/message notification with the result
    let mut parent_msg_rx = parent_msg_rx;
    let envelope = tokio::time::timeout(Duration::from_secs(5), parent_msg_rx.recv())
        .await
        .expect("parent should receive notification")
        .expect("channel should not close");

    assert!(
        matches!(
            envelope.source,
            MessageSource::Agent(ref qa) if qa.agent == "child-a" && qa.is_local()
        ),
        "source should be Agent(local('child-a')), got: {:?}",
        envelope.source
    );
    let text = &envelope.content.text;
    assert!(
        text.contains("child-a") && text.contains("42 issues"),
        "should contain child name and result, got: {text}"
    );
}

/// Multiple children completing → parent receives all notifications.
#[tokio::test]
async fn multiple_children_all_delivered() {
    let (hub, _) = make_hub();

    let (parent_conn, mut parent_rx) = register_agent(&hub, "parent", None).await;
    let (tx, rx) = mpsc::channel::<Envelope>(16);
    let pc = parent_conn.clone();
    tokio::spawn(async move {
        while let Some(msg) = parent_rx.recv().await {
            match msg {
                Incoming::Notification { method, params }
                    if method == methods::AGENT_MESSAGE.name =>
                {
                    if let Ok(env) = serde_json::from_value::<Envelope>(params) {
                        let _ = tx.send(env).await;
                    }
                }
                Incoming::Request { id, .. } => {
                    let _ = pc.respond(id, json!({"ok": true})).await;
                }
                _ => {}
            }
        }
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Spawn 3 children
    let (c1, _) = register_agent(&hub, "child-1", Some("parent")).await;
    let (c2, _) = register_agent(&hub, "child-2", Some("parent")).await;
    let (c3, _) = register_agent(&hub, "child-3", Some("parent")).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // All three complete
    for (conn, name) in [(&c2, "child-2"), (&c3, "child-3"), (&c1, "child-1")] {
        conn.send_notification(
            methods::AGENT_COMPLETED.name,
            json!({"reason": "goal", "result": format!("Result from {name}")}),
        )
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Collect all 3 notifications
    let mut rx = rx;
    let mut results = Vec::new();
    for _ in 0..3 {
        let env = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .unwrap()
            .unwrap();
        results.push(env.content.text);
    }

    assert!(results.iter().any(|r| r.contains("child-1")));
    assert!(results.iter().any(|r| r.contains("child-2")));
    assert!(results.iter().any(|r| r.contains("child-3")));
}
