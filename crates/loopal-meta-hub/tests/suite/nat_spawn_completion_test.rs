use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use loopal_ipc::connection::Incoming;
use loopal_protocol::{Envelope, MessageSource, QualifiedAddress};

use loopal_meta_hub::MetaHub;

use crate::test_helpers::*;

/// Cross-hub completion: hub-B child finishes → completion envelope reaches
/// hub-A parent with the child's hub stamped onto `source` (so the parent
/// can correlate the result with the originating hub even when child names
/// collide across the cluster).
#[tokio::test]
async fn cross_hub_completion_carries_origin_hub_in_source() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (hub_a, _hub_a_event_rx) = make_hub();
    let (hub_b, _hub_b_event_rx) = make_hub();
    let _hub_a_conn = wire_hub_to_meta("hub-a", &hub_a, &meta_hub).await;
    let hub_b_conn = wire_hub_to_meta("hub-b", &hub_b, &meta_hub).await;
    {
        let ul = Arc::new(loopal_agent_hub::HubUplink::new(hub_b_conn, "hub-b".into()));
        hub_b.lock().await.uplink = Some(ul);
    }

    // Set up a "parent" on hub-A — it's the receiver of the completion.
    let (_parent_conn, mut parent_rx) = register_mock_agent(&hub_a, "parent", None).await;
    hub_a
        .lock()
        .await
        .registry
        .register_shadow("child", QualifiedAddress::local("parent"))
        .unwrap();

    let (child_conn, _child_rx) = register_remote_agent(
        &hub_b,
        "child",
        QualifiedAddress::remote(["hub-a"], "parent"),
    )
    .await;
    child_conn
        .send_notification(
            loopal_ipc::protocol::methods::AGENT_COMPLETED.name,
            serde_json::json!({"reason": "goal", "result": "ok"}),
        )
        .await
        .unwrap();

    // hub-A's parent should observe the completion envelope.
    let msg = tokio::time::timeout(Duration::from_secs(2), parent_rx.recv())
        .await
        .expect("parent should receive completion")
        .expect("channel open");
    let Incoming::Request { params, .. } = msg else {
        panic!("expected request");
    };
    let env: Envelope = serde_json::from_value(params).unwrap();

    // Source: AgentResult{child=QA{hub=["hub-b"], agent="child"}} — proves
    // SNAT applied to the typed completion source.
    assert_eq!(
        env.source,
        MessageSource::AgentResult {
            child: QualifiedAddress::remote(["hub-b"], "child")
        },
        "completion source must carry origin hub"
    );
    // Target: local("parent") — proves DNAT consumed hub-A from the path.
    assert_eq!(env.target, QualifiedAddress::local("parent"));
    // Body is raw now — the <agent-result> wrapper is a projection concern.
    assert_eq!(env.content.text, "ok");
    assert_eq!(env.agent_completion.unwrap().reason, "goal");
}

/// Cross-hub spawn: a child registered with a qualified `hub/agent` parent
/// string lands typed in both `AgentInfo.parent` and the `SubAgentSpawned`
/// event payload — proving the spawn protocol's wire format flows into
/// the type system without lossy stringly-typed detours.
#[tokio::test]
async fn cross_hub_spawn_carries_qualified_parent_through_event_and_registry() {
    use loopal_protocol::AgentEventPayload;

    let (hub_b, mut event_rx) = make_hub();
    let qualified_parent = QualifiedAddress::remote(["hub-a"], "alpha");
    let (_client_conn, _client_rx) =
        register_remote_agent(&hub_b, "child", qualified_parent.clone()).await;

    // Drain events until the SubAgentSpawned arrives — Started may race ahead.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let spawned = loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let event = tokio::time::timeout(remaining, event_rx.recv())
            .await
            .expect("waiting for SubAgentSpawned timed out")
            .expect("event channel open");
        if matches!(event.payload, AgentEventPayload::SubAgentSpawned(_)) {
            break event;
        }
    };
    let AgentEventPayload::SubAgentSpawned(s) = spawned.payload else {
        unreachable!()
    };
    assert_eq!(s.name, "child");
    assert_eq!(
        s.parent,
        Some(qualified_parent.clone()),
        "event parent must be a typed qualified address"
    );

    // AgentInfo.parent on hub-B must mirror the same QA — it's the only
    // way `finish::finish_and_deliver` knows to route the completion via
    // uplink instead of looking for a local parent.
    let h = hub_b.lock().await;
    let info = h.registry.agent_info("child").expect("child registered");
    assert_eq!(
        info.parent,
        Some(qualified_parent),
        "AgentInfo.parent must hold the qualified address"
    );
    assert!(
        info.parent.as_ref().is_some_and(|p| p.is_remote()),
        "parent must be flagged remote so completion takes the uplink path"
    );
}
