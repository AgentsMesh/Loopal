use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_protocol::{Envelope, MessageSource, QualifiedAddress};
use serde_json::json;

use loopal_meta_hub::MetaHub;

use crate::test_helpers::*;

/// Self-reply test: β receives a stamped source from α and uses it
/// verbatim as the reply target. The reply must traverse the symmetric
/// NAT path back to α with the right source/target shapes at each hop.
#[tokio::test]
async fn nat_round_trip_reply_returns_to_origin() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (hub_a, _hub_a_event_rx) = make_hub();
    let (hub_b, _hub_b_event_rx) = make_hub();
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
