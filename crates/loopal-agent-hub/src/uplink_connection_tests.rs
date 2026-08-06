use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::sync::{Mutex, mpsc};

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::tcp::TcpTransport;
use loopal_protocol::{AgentEvent, AgentEventPayload};

use super::{connect, disconnect};
use crate::pending_relay::{InteractionAudience, PendingQuestionInfo, PendingRemoteQuestionInfo};
use crate::{Hub, HubUplink};

fn test_hub() -> Arc<Mutex<Hub>> {
    let (events, _) = mpsc::channel::<AgentEvent>(8);
    Arc::new(Mutex::new(Hub::new(events)))
}

#[tokio::test]
async fn register_blackhole_times_out_without_installing_uplink() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let transport: Arc<dyn loopal_ipc::transport::Transport> =
            Arc::new(TcpTransport::new(stream));
        let (_conn, mut rx) = Connection::new(transport).into_listening();
        assert!(matches!(rx.recv().await, Some(Incoming::Request { .. })));
        while rx.recv().await.is_some() {}
    });
    let hub = test_hub();

    let error = connect(&hub, &address, "secret", "desktop")
        .await
        .unwrap_err();

    assert!(
        error.contains("register timed out"),
        "unexpected error: {error}"
    );
    assert!(hub.lock().await.uplink.is_none());
    server.await.unwrap();
}

#[tokio::test]
async fn heartbeat_blackhole_cleans_installed_uplink_by_identity() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let transport: Arc<dyn loopal_ipc::transport::Transport> =
            Arc::new(TcpTransport::new(stream));
        let (conn, mut rx) = Connection::new(transport).into_listening();
        let Some(Incoming::Request { id, .. }) = rx.recv().await else {
            panic!("missing register request");
        };
        conn.respond(id, serde_json::json!({"ok": true}))
            .await
            .unwrap();
        assert!(matches!(rx.recv().await, Some(Incoming::Request { .. })));
        while rx.recv().await.is_some() {}
    });
    let hub = test_hub();

    let error = connect(&hub, &address, "secret", "desktop")
        .await
        .unwrap_err();

    assert!(error.contains("heartbeat timed out"));
    assert!(hub.lock().await.uplink.is_none());
    server.await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn leave_blackhole_is_bounded_and_always_closes_connection() {
    let (client_transport, peer_transport) = loopal_ipc::duplex_pair();
    let (client, _client_rx) = Connection::new(client_transport).into_listening();
    let (_peer, mut peer_rx) = Connection::new(peer_transport).into_listening();
    let hub = test_hub();
    hub.lock().await.uplink = Some(Arc::new(HubUplink::new(client.clone(), "desktop".into())));
    let peer = tokio::spawn(async move {
        assert!(matches!(
            peer_rx.recv().await,
            Some(Incoming::Request { .. })
        ));
        while peer_rx.recv().await.is_some() {}
    });

    disconnect(&hub).await.unwrap();

    assert!(hub.lock().await.uplink.is_none());
    assert!(!client.is_connected());
    peer.await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn explicit_disconnect_cleans_uplink_interactions_before_unregister_finishes() {
    let (uplink_transport, meta_transport) = loopal_ipc::duplex_pair();
    let (uplink_conn, _uplink_rx) = Connection::new(uplink_transport).into_listening();
    let (_meta_conn, mut meta_rx) = Connection::new(meta_transport).into_listening();
    let uplink = Arc::new(HubUplink::new(uplink_conn.clone(), "desktop".into()));
    let (agent_transport, hub_agent_transport) = loopal_ipc::duplex_pair();
    let (agent, _agent_rx) = Connection::new(agent_transport).into_listening();
    let (hub_agent, mut hub_agent_rx) = Connection::new(hub_agent_transport).into_listening();
    let pending_response = tokio::spawn(async move {
        agent
            .send_request("test/pending_question", serde_json::json!({}))
            .await
    });
    let agent_ipc_id = match hub_agent_rx.recv().await.unwrap() {
        Incoming::Request { id, .. } => id,
        other => panic!("expected pending agent request, got {other:?}"),
    };
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(8);
    let mut hub = Hub::new(event_tx);
    hub.uplink = Some(uplink.clone());
    hub.pending_questions.insert(
        ("worker".into(), "origin-logical".into()),
        PendingQuestionInfo {
            agent_conn: hub_agent,
            agent_ipc_id,
            agent_name: "worker".into(),
            interaction_id: "origin-token".into(),
            logical_id: "origin-logical".into(),
            audience: InteractionAudience::RemoteUi {
                target_hub: "destination".into(),
                uplink: uplink.clone(),
            },
        },
    );
    hub.pending_remote_questions.insert(
        ("origin/worker".into(), "destination-token".into()),
        PendingRemoteQuestionInfo {
            origin_hub: "origin".into(),
            origin_agent: "worker".into(),
            qualified_agent: "origin/worker".into(),
            interaction_id: "destination-token".into(),
            logical_id: "destination-logical".into(),
            request: AgentEventPayload::UserQuestionRequest {
                id: "destination-token".into(),
                logical_id: "destination-logical".into(),
                questions: Vec::new(),
                classifier_running: false,
            },
            uplink,
            deadline: tokio::time::Instant::now() + Duration::from_secs(60),
            forwarding: false,
        },
    );
    let hub = Arc::new(Mutex::new(hub));

    let leave = tokio::spawn({
        let hub = hub.clone();
        async move { disconnect(&hub).await }
    });
    assert!(matches!(
        meta_rx.recv().await,
        Some(Incoming::Request { .. })
    ));
    assert!(
        !leave.is_finished(),
        "unregister blackhole should still be pending"
    );
    {
        let h = hub.lock().await;
        assert!(h.pending_questions.is_empty());
        assert!(h.pending_remote_questions.is_empty());
    }
    let response = pending_response.await.unwrap().unwrap();
    assert_eq!(response["kind"], "cancelled");
    assert_eq!(response["question_id"], "origin-logical");

    let mut resolved = Vec::new();
    for _ in 0..2 {
        if let AgentEventPayload::UserQuestionResolved { id, .. } =
            event_rx.recv().await.unwrap().payload
        {
            resolved.push(id);
        }
    }
    resolved.sort();
    assert_eq!(resolved, ["destination-token", "origin-token"]);

    tokio::time::advance(Duration::from_secs(3)).await;
    leave.await.unwrap().unwrap();
    assert!(!uplink_conn.is_connected());
}

#[tokio::test(start_paused = true)]
async fn periodic_heartbeat_blackhole_removes_and_closes_current_uplink() {
    let (client_transport, peer_transport) = loopal_ipc::duplex_pair();
    let (client, client_rx) = Connection::new(client_transport).into_listening();
    let (_peer, mut peer_rx) = Connection::new(peer_transport).into_listening();
    let hub = test_hub();
    let uplink = Arc::new(HubUplink::new(client.clone(), "desktop".into()));
    hub.lock().await.uplink = Some(uplink.clone());
    crate::uplink_tasks::start(hub.clone(), uplink, client_rx);
    let peer = tokio::spawn(async move {
        assert!(matches!(
            peer_rx.recv().await,
            Some(Incoming::Request { .. })
        ));
        while peer_rx.recv().await.is_some() {}
    });

    tokio::task::yield_now().await;
    tokio::time::advance(std::time::Duration::from_secs(15)).await;
    tokio::task::yield_now().await;
    tokio::time::advance(std::time::Duration::from_secs(6)).await;
    tokio::task::yield_now().await;

    assert!(hub.lock().await.uplink.is_none());
    assert!(!client.is_connected());
    peer.await.unwrap();
}
