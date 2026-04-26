//! End-to-end NAT routing tests — verify the SNAT/DNAT invariant across
//! a real Sub-Hub ↔ MetaHub ↔ Sub-Hub topology.
//!
//! These tests are the system-level companion to the unit-level address
//! tests in `loopal-protocol/tests/suite/envelope_test.rs`.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_protocol::{Envelope, MessageSource, QualifiedAddress};
use serde_json::json;

use loopal_meta_hub::MetaHub;

use crate::test_helpers::*;

/// α (hub-A) → hub-B/β: β must observe `source.hub = ["hub-A"]` so it
/// can reply via the symmetric NAT path.
#[tokio::test]
async fn nat_stamps_origin_hub_into_source_for_cross_hub_messages() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (hub_a, _) = make_hub();
    let (hub_b, _) = make_hub();
    let hub_a_conn = wire_hub_to_meta("hub-a", &hub_a, &meta_hub).await;
    let _hub_b_conn = wire_hub_to_meta("hub-b", &hub_b, &meta_hub).await;
    {
        let ul = Arc::new(loopal_agent_hub::HubUplink::new(hub_a_conn, "hub-a".into()));
        hub_a.lock().await.uplink = Some(ul);
    }
    let (_agent_conn, mut beta_rx) = register_mock_agent(&hub_b, "beta", None).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // α sends with a *local* source (no hub) — hub-A's uplink will SNAT it.
    let envelope = json!({
        "id": "00000000-0000-0000-0000-0000000000aa",
        "source": {"Agent": {"hub": [], "agent": "alpha"}},
        "target": {"hub": ["hub-b"], "agent": "beta"},
        "content": {"text": "hi beta", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let result = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub_a,
        methods::HUB_ROUTE.name,
        envelope,
        "alpha".into(),
    )
    .await;
    assert!(result.is_ok(), "cross-hub route failed: {result:?}");

    let msg = tokio::time::timeout(Duration::from_secs(2), beta_rx.recv())
        .await
        .expect("beta should receive a message")
        .expect("channel closed");

    let Incoming::Request { method, params, .. } = msg else {
        panic!("expected request, got {msg:?}");
    };
    assert_eq!(method, methods::AGENT_MESSAGE.name);
    let env: Envelope = serde_json::from_value(params).expect("envelope deserializes at receiver");

    // SNAT: source carries hub-A.
    assert_eq!(
        env.source,
        MessageSource::Agent(QualifiedAddress::remote(["hub-a"], "alpha")),
        "receiver must see hub-prefixed source"
    );
    // DNAT: target hub stripped down to local view.
    assert_eq!(
        env.target,
        QualifiedAddress::local("beta"),
        "target should appear local at the destination hub"
    );
}

/// Local-only routes must remain hub-free in the source (no SNAT applied
/// when the message never crosses an outbound boundary).
#[tokio::test]
async fn local_route_does_not_inject_hub_into_source() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (hub_a, _) = make_hub();
    let hub_a_conn = wire_hub_to_meta("hub-a", &hub_a, &meta_hub).await;
    {
        let ul = Arc::new(loopal_agent_hub::HubUplink::new(hub_a_conn, "hub-a".into()));
        hub_a.lock().await.uplink = Some(ul);
    }
    let (_agent_conn, mut peer_rx) = register_mock_agent(&hub_a, "peer", None).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    let envelope = json!({
        "id": "00000000-0000-0000-0000-0000000000bb",
        "source": {"Agent": {"hub": [], "agent": "alpha"}},
        "target": {"hub": [], "agent": "peer"},
        "content": {"text": "intra-hub", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let result = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub_a,
        methods::HUB_ROUTE.name,
        envelope,
        "alpha".into(),
    )
    .await;
    assert!(result.is_ok());

    let msg = tokio::time::timeout(Duration::from_secs(2), peer_rx.recv())
        .await
        .expect("peer should receive")
        .expect("channel closed");
    let Incoming::Request { params, .. } = msg else {
        panic!("expected request");
    };
    let env: Envelope = serde_json::from_value(params).unwrap();

    // No SNAT happened — source remains local.
    assert_eq!(
        env.source,
        MessageSource::Agent(QualifiedAddress::local("alpha"))
    );
    assert_eq!(env.target, QualifiedAddress::local("peer"));
}

/// MetaHub must reject envelopes whose next-hop hub is the originating hub
/// — this catches loops before they cause a self-deliver storm.
#[tokio::test]
async fn metahub_rejects_self_routing() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (hub_a, _) = make_hub();
    let _hub_a_conn = wire_hub_to_meta("hub-a", &hub_a, &meta_hub).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    let envelope = json!({
        "id": "00000000-0000-0000-0000-0000000000cc",
        "source": {"Agent": {"hub": [], "agent": "alpha"}},
        "target": {"hub": ["hub-a"], "agent": "anyone"},
        "content": {"text": "boomerang", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let result = loopal_meta_hub::dispatch::dispatch_meta_request(
        &meta_hub,
        methods::META_ROUTE.name,
        envelope,
        "hub-a".into(),
    )
    .await;
    let err = result.expect_err("self-routing must be rejected");
    assert!(err.contains("self-routing"), "unexpected error: {err}");
}

/// Self-reply test: β receives a stamped source from α and uses it
/// verbatim as the reply target. The reply must traverse the symmetric
/// NAT path back to α with the right source/target shapes at each hop.
#[tokio::test]
async fn nat_round_trip_reply_returns_to_origin() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (hub_a, _) = make_hub();
    let (hub_b, _) = make_hub();
    let hub_a_conn = wire_hub_to_meta("hub-a", &hub_a, &meta_hub).await;
    let hub_b_conn = wire_hub_to_meta("hub-b", &hub_b, &meta_hub).await;
    {
        let ul = Arc::new(loopal_agent_hub::HubUplink::new(hub_a_conn, "hub-a".into()));
        hub_a.lock().await.uplink = Some(ul);
    }
    {
        let ul = Arc::new(loopal_agent_hub::HubUplink::new(hub_b_conn, "hub-b".into()));
        hub_b.lock().await.uplink = Some(ul);
    }
    let (_alpha_conn, mut alpha_rx) = register_mock_agent(&hub_a, "alpha", None).await;
    let (_beta_conn, mut beta_rx) = register_mock_agent(&hub_b, "beta", None).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // ── Outbound: α → hub-B/β ──────────────────────────────────────
    let outbound = json!({
        "id": "00000000-0000-0000-0000-000000000100",
        "source": {"Agent": {"hub": [], "agent": "alpha"}},
        "target": {"hub": ["hub-b"], "agent": "beta"},
        "content": {"text": "ping", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let r = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub_a,
        methods::HUB_ROUTE.name,
        outbound,
        "alpha".into(),
    )
    .await;
    assert!(r.is_ok(), "outbound route failed: {r:?}");

    let beta_msg = tokio::time::timeout(Duration::from_secs(2), beta_rx.recv())
        .await
        .expect("beta receives outbound")
        .expect("channel open");
    let Incoming::Request { params, .. } = beta_msg else {
        panic!("expected request");
    };
    let outbound_env: Envelope = serde_json::from_value(params).unwrap();
    let reply_target = match outbound_env.source.clone() {
        MessageSource::Agent(qa) => qa,
        other => panic!("expected Agent source, got {other:?}"),
    };
    assert_eq!(
        reply_target,
        QualifiedAddress::remote(["hub-a"], "alpha"),
        "β should receive a hub-stamped source it can reply to"
    );

    // ── Reply: β uses the received source verbatim as target ───────
    let reply = json!({
        "id": "00000000-0000-0000-0000-000000000101",
        "source": {"Agent": {"hub": [], "agent": "beta"}},
        "target": {"hub": reply_target.hub, "agent": reply_target.agent},
        "content": {"text": "pong", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let r = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub_b,
        methods::HUB_ROUTE.name,
        reply,
        "beta".into(),
    )
    .await;
    assert!(r.is_ok(), "reply route failed: {r:?}");

    // ── α receives the reply with hub-B stamped, target local ──────
    let alpha_msg = tokio::time::timeout(Duration::from_secs(2), alpha_rx.recv())
        .await
        .expect("alpha receives reply")
        .expect("channel open");
    let Incoming::Request { params, .. } = alpha_msg else {
        panic!("expected request");
    };
    let reply_env: Envelope = serde_json::from_value(params).unwrap();
    assert_eq!(
        reply_env.source,
        MessageSource::Agent(QualifiedAddress::remote(["hub-b"], "beta")),
        "α should see the symmetric hub-B stamp on the reply source"
    );
    assert_eq!(
        reply_env.target,
        QualifiedAddress::local("alpha"),
        "α should see a local target after MetaHub DNAT"
    );
    assert_eq!(reply_env.content.text, "pong");
}

/// Cross-hub completion: hub-B child finishes → completion envelope reaches
/// hub-A parent with the child's hub stamped onto `source` (so the parent
/// can correlate the result with the originating hub even when child names
/// collide across the cluster).
#[tokio::test]
async fn cross_hub_completion_carries_origin_hub_in_source() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (hub_a, _) = make_hub();
    let (hub_b, _) = make_hub();
    let _hub_a_conn = wire_hub_to_meta("hub-a", &hub_a, &meta_hub).await;
    let hub_b_conn = wire_hub_to_meta("hub-b", &hub_b, &meta_hub).await;
    {
        let ul = Arc::new(loopal_agent_hub::HubUplink::new(hub_b_conn, "hub-b".into()));
        hub_b.lock().await.uplink = Some(ul);
    }

    // Set up a "parent" on hub-A — it's the receiver of the completion.
    let (_parent_conn, mut parent_rx) = register_mock_agent(&hub_a, "parent", None).await;

    // Register a child on hub-B whose parent is *remote* (lives on hub-A).
    // No completion_tx — finish_and_deliver will fall through to uplink.
    let (client_t, server_t) = loopal_ipc::duplex_pair();
    let child_conn = Arc::new(loopal_ipc::connection::Connection::new(server_t));
    let _client_conn = Arc::new(loopal_ipc::connection::Connection::new(client_t));
    let _server_rx = child_conn.start();
    let _client_rx = _client_conn.start();
    {
        let mut h = hub_b.lock().await;
        h.registry
            .register_connection_with_parent(
                "child",
                child_conn.clone(),
                Some(QualifiedAddress::remote(["hub-a"], "parent")),
                None,
                None,
            )
            .unwrap();
    }
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Trigger finish. finish_and_deliver detects the remote parent and
    // routes the completion envelope through hub-B's uplink.
    loopal_agent_hub::finish::finish_and_deliver(&hub_b, "child", Some("ok".into()), &child_conn)
        .await;

    // hub-A's parent should observe the completion envelope.
    let msg = tokio::time::timeout(Duration::from_secs(2), parent_rx.recv())
        .await
        .expect("parent should receive completion")
        .expect("channel open");
    let Incoming::Request { params, .. } = msg else {
        panic!("expected request");
    };
    let env: Envelope = serde_json::from_value(params).unwrap();

    // Source: Agent(QA{hub=["hub-b"], agent="child"}) — proves SNAT applied.
    assert_eq!(
        env.source,
        MessageSource::Agent(QualifiedAddress::remote(["hub-b"], "child")),
        "completion source must carry origin hub"
    );
    // Target: local("parent") — proves DNAT consumed hub-A from the path.
    assert_eq!(env.target, QualifiedAddress::local("parent"));
    assert!(env.content.text.contains("<agent-result name=\"child\">"));
    assert!(env.content.text.contains("ok"));
}

/// Cross-hub spawn: a child registered with a qualified `hub/agent` parent
/// string lands typed in both `AgentInfo.parent` and the `SubAgentSpawned`
/// event payload — proving the spawn protocol's wire format flows into
/// the type system without lossy stringly-typed detours.
#[tokio::test]
async fn cross_hub_spawn_carries_qualified_parent_through_event_and_registry() {
    use loopal_agent_hub::spawn_manager::register_agent_connection;
    use loopal_ipc::connection::Connection;
    use loopal_protocol::AgentEventPayload;

    let (hub_b, mut event_rx) = make_hub();

    // Simulate the IPC inbound side of a remote spawn — a client connection
    // arrives over duplex and registers with a qualified parent string
    // (the form produced by `dispatch_handlers::handle_spawn_agent` on the
    // originating hub when target_hub is set).
    let (client_t, server_t) = loopal_ipc::duplex_pair();
    let server_conn = Arc::new(Connection::new(server_t));
    let _client_conn = Arc::new(Connection::new(client_t));
    let server_rx = server_conn.start();
    let _client_rx = _client_conn.start();

    let _ = register_agent_connection(
        hub_b.clone(),
        "child",
        server_conn,
        server_rx,
        Some("hub-a/alpha"), // qualified parent over the wire
        None,
        None,
    )
    .await
    .expect("registration succeeds");

    // Drain events until the SubAgentSpawned arrives — Started may race ahead.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let spawned = loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let event = tokio::time::timeout(remaining, event_rx.recv())
            .await
            .expect("waiting for SubAgentSpawned timed out")
            .expect("event channel open");
        if matches!(event.payload, AgentEventPayload::SubAgentSpawned { .. }) {
            break event;
        }
    };
    let AgentEventPayload::SubAgentSpawned { name, parent, .. } = spawned.payload else {
        unreachable!()
    };
    assert_eq!(name, "child");
    assert_eq!(
        parent,
        Some(QualifiedAddress::remote(["hub-a"], "alpha")),
        "event parent must be a typed qualified address"
    );

    // AgentInfo.parent on hub-B must mirror the same QA — it's the only
    // way `finish::finish_and_deliver` knows to route the completion via
    // uplink instead of looking for a local parent.
    let h = hub_b.lock().await;
    let info = h.registry.agent_info("child").expect("child registered");
    assert_eq!(
        info.parent,
        Some(QualifiedAddress::remote(["hub-a"], "alpha")),
        "AgentInfo.parent must hold the qualified address"
    );
    assert!(
        info.parent.as_ref().is_some_and(|p| p.is_remote()),
        "parent must be flagged remote so completion takes the uplink path"
    );
}
