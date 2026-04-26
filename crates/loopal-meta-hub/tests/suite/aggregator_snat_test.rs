//! Tests for `aggregator::prefix_agent_name` — the event-side SNAT that
//! stamps the relaying hub onto both `event.agent_name` and any still-local
//! qualified addresses inside `event.payload`.

use loopal_protocol::{AgentEvent, AgentEventPayload, MessageSource, QualifiedAddress};

#[tokio::test]
async fn prefix_agent_name_snats_subagent_spawned_local_parent() {
    let mut event = AgentEvent::named(
        QualifiedAddress::local("child"),
        AgentEventPayload::SubAgentSpawned {
            name: "child".into(),
            agent_id: "id-1".into(),
            parent: Some(QualifiedAddress::local("parent")),
            model: None,
            session_id: None,
        },
    );
    loopal_meta_hub::aggregator::prefix_agent_name(&mut event, "hub-a");

    assert_eq!(
        event.agent_name,
        Some(QualifiedAddress::remote(["hub-a"], "child"))
    );
    let AgentEventPayload::SubAgentSpawned { parent, .. } = event.payload else {
        unreachable!()
    };
    assert_eq!(
        parent,
        Some(QualifiedAddress::remote(["hub-a"], "parent")),
        "local parent must be prefixed by aggregator SNAT"
    );
}

#[tokio::test]
async fn prefix_agent_name_does_not_double_stamp_qualified_parent() {
    let mut event = AgentEvent::named(
        QualifiedAddress::local("child"),
        AgentEventPayload::SubAgentSpawned {
            name: "child".into(),
            agent_id: "id-2".into(),
            parent: Some(QualifiedAddress::remote(["hub-a"], "alpha")),
            model: None,
            session_id: None,
        },
    );
    loopal_meta_hub::aggregator::prefix_agent_name(&mut event, "hub-b");

    let AgentEventPayload::SubAgentSpawned { parent, .. } = event.payload else {
        unreachable!()
    };
    assert_eq!(
        parent,
        Some(QualifiedAddress::remote(["hub-a"], "alpha")),
        "already-qualified parent must not be double-stamped"
    );
}

#[tokio::test]
async fn prefix_agent_name_snats_message_routed_local_addresses() {
    let mut event = AgentEvent::root(AgentEventPayload::MessageRouted {
        source: MessageSource::Agent(QualifiedAddress::local("alpha")),
        target: QualifiedAddress::local("beta"),
        content_preview: "hi".into(),
    });
    loopal_meta_hub::aggregator::prefix_agent_name(&mut event, "hub-a");

    let AgentEventPayload::MessageRouted { source, target, .. } = event.payload else {
        unreachable!()
    };
    assert_eq!(
        source,
        MessageSource::Agent(QualifiedAddress::remote(["hub-a"], "alpha"))
    );
    assert_eq!(target, QualifiedAddress::remote(["hub-a"], "beta"));
}

#[tokio::test]
async fn prefix_agent_name_preserves_already_qualified_message_source() {
    // hub-B receives a message from hub-A and routes it locally — the
    // resulting MessageRouted source already carries hub-A; aggregator
    // must leave it alone but still stamp the local target.
    let mut event = AgentEvent::root(AgentEventPayload::MessageRouted {
        source: MessageSource::Agent(QualifiedAddress::remote(["hub-a"], "alpha")),
        target: QualifiedAddress::local("beta"),
        content_preview: "hi".into(),
    });
    loopal_meta_hub::aggregator::prefix_agent_name(&mut event, "hub-b");

    let AgentEventPayload::MessageRouted { source, target, .. } = event.payload else {
        unreachable!()
    };
    assert_eq!(
        source,
        MessageSource::Agent(QualifiedAddress::remote(["hub-a"], "alpha")),
        "cross-hub source must not be re-stamped"
    );
    assert_eq!(
        target,
        QualifiedAddress::remote(["hub-b"], "beta"),
        "local target must still be stamped with the relaying hub"
    );
}

// --- Channel source SNAT (parity with Agent source) ---

#[tokio::test]
async fn prefix_agent_name_snats_message_routed_channel_local_from() {
    let mut event = AgentEvent::root(AgentEventPayload::MessageRouted {
        source: MessageSource::Channel {
            channel: "general".into(),
            from: QualifiedAddress::local("alpha"),
        },
        target: QualifiedAddress::local("beta"),
        content_preview: "hi".into(),
    });
    loopal_meta_hub::aggregator::prefix_agent_name(&mut event, "hub-a");

    let AgentEventPayload::MessageRouted { source, target, .. } = event.payload else {
        unreachable!()
    };
    let MessageSource::Channel { channel, from } = source else {
        panic!("expected Channel source");
    };
    assert_eq!(channel, "general");
    assert_eq!(
        from,
        QualifiedAddress::remote(["hub-a"], "alpha"),
        "local Channel.from must be stamped just like Agent address"
    );
    assert_eq!(target, QualifiedAddress::remote(["hub-a"], "beta"));
}

#[tokio::test]
async fn prefix_agent_name_does_not_double_stamp_channel_qualified_from() {
    let mut event = AgentEvent::root(AgentEventPayload::MessageRouted {
        source: MessageSource::Channel {
            channel: "general".into(),
            from: QualifiedAddress::remote(["hub-a"], "alpha"),
        },
        target: QualifiedAddress::local("beta"),
        content_preview: "hi".into(),
    });
    loopal_meta_hub::aggregator::prefix_agent_name(&mut event, "hub-b");

    let AgentEventPayload::MessageRouted { source, .. } = event.payload else {
        unreachable!()
    };
    let MessageSource::Channel { from, .. } = source else {
        panic!("expected Channel source");
    };
    assert_eq!(
        from,
        QualifiedAddress::remote(["hub-a"], "alpha"),
        "already-qualified Channel.from must not be double-stamped"
    );
}
