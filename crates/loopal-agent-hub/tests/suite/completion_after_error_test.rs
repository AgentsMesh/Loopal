use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_agent_hub::{AgentLifecycle, Hub, start_event_loop};
use loopal_ipc::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentCompletion, AgentEvent, AgentEventPayload};
use tokio::sync::{Mutex, mpsc};

#[tokio::test]
async fn current_completion_is_accepted_after_error_projects_failed() {
    let (events, event_rx) = mpsc::channel(16);
    let hub = Arc::new(Mutex::new(Hub::new(events)));
    let _event_loop = start_event_loop(hub.clone(), event_rx);
    let (agent_transport, hub_transport) = loopal_ipc::duplex_pair();
    let (agent, _agent_rx) = Connection::new(agent_transport).into_listening();
    let (server, server_rx) = Connection::new(hub_transport).into_listening();
    register_agent_connection(hub.clone(), "worker", server, server_rx, None, None, None)
        .await
        .unwrap();

    agent
        .send_notification(
            methods::AGENT_EVENT.name,
            serde_json::to_value(AgentEvent::named(
                "worker",
                AgentEventPayload::Error {
                    message: "provider failed".into(),
                },
            ))
            .unwrap(),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if matches!(
                hub.lock().await.registry.agent_info("worker"),
                Some(info) if info.lifecycle == AgentLifecycle::Failed("provider failed".into())
            ) {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("Error was not projected before completion");

    agent
        .send_notification(
            methods::AGENT_COMPLETED.name,
            serde_json::to_value(AgentCompletion::new(
                "error",
                Some("provider failed".into()),
            ))
            .unwrap(),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if hub.lock().await.registry.completion("worker").is_some() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("current terminal completion was rejected");

    let hub = hub.lock().await;
    let completion = hub.registry.completion("worker").unwrap();
    assert_eq!(completion.reason, "error");
    assert_eq!(completion.result.as_deref(), Some("provider failed"));
    assert!(hub.registry.get_agent_connection("worker").is_none());
    assert!(matches!(
        hub.registry
            .agent_info("worker")
            .map(|info| &info.lifecycle),
        Some(AgentLifecycle::Failed(_))
    ));
}
