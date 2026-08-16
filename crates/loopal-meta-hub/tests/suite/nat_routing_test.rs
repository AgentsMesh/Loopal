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
    let (hub_a, _hub_a_event_rx) = make_hub();
    let (hub_b, _hub_b_event_rx) = make_hub();
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
    let (hub_a, _hub_a_event_rx) = make_hub();
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
    let (hub_a, _hub_a_event_rx) = make_hub();
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
