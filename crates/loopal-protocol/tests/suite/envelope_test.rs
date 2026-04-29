use loopal_protocol::{Envelope, MessageSource, QualifiedAddress};

#[test]
fn test_envelope_new_generates_unique_ids() {
    let a = Envelope::new(MessageSource::Human, "main", "hello");
    let b = Envelope::new(MessageSource::Human, "main", "hello");
    assert_ne!(a.id, b.id);
}

#[test]
fn test_envelope_fields_stored_correctly() {
    let env = Envelope::new(
        MessageSource::Agent(QualifiedAddress::local("researcher")),
        "main",
        "found results",
    );
    assert_eq!(
        env.source,
        MessageSource::Agent(QualifiedAddress::local("researcher"))
    );
    assert_eq!(env.target, QualifiedAddress::local("main"));
    assert_eq!(env.content.text, "found results");
}

#[test]
fn test_envelope_content_preview_short() {
    let env = Envelope::new(MessageSource::Human, "main", "short msg");
    assert_eq!(env.content_preview(), "short msg");
}

#[test]
fn test_envelope_content_preview_long_truncated() {
    let long = "a".repeat(200);
    let env = Envelope::new(MessageSource::Human, "main", long);
    assert_eq!(env.content_preview().len(), 80);
}

#[test]
fn test_envelope_serde_roundtrip() {
    let env = Envelope::new(
        MessageSource::Channel {
            channel: "general".into(),
            from: QualifiedAddress::local("bot"),
        },
        "worker-1",
        "task update",
    );
    let json = serde_json::to_string(&env).unwrap();
    let restored: Envelope = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.id, env.id);
    assert_eq!(restored.source, env.source);
    assert_eq!(restored.target, env.target);
    assert_eq!(restored.content.text, env.content.text);
}

#[test]
fn test_message_source_variants() {
    let human = MessageSource::Human;
    let agent = MessageSource::Agent(QualifiedAddress::local("coder"));
    let channel = MessageSource::Channel {
        channel: "updates".into(),
        from: QualifiedAddress::local("notifier"),
    };

    assert_ne!(human, agent);
    assert_ne!(agent, channel);
    assert_eq!(human, MessageSource::Human);
}

#[test]
fn test_scheduled_source_label() {
    assert_eq!(MessageSource::Scheduled.label(), "scheduled");
}

#[test]
fn test_agent_label_uses_qualified_form() {
    let local = MessageSource::Agent(QualifiedAddress::local("alpha"));
    assert_eq!(local.label(), "alpha");

    let remote = MessageSource::Agent(QualifiedAddress::remote(["hub-A"], "alpha"));
    assert_eq!(remote.label(), "hub-A/alpha");
}

#[test]
fn test_scheduled_source_serde_roundtrip() {
    let env = Envelope::new(MessageSource::Scheduled, "main", "check deploys");
    let json = serde_json::to_string(&env).unwrap();
    let restored: Envelope = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.source, MessageSource::Scheduled);
    assert_eq!(restored.content.text, "check deploys");
}

#[test]
fn test_apply_snat_stamps_source_with_self_hub() {
    let mut env = Envelope::new(
        MessageSource::Agent(QualifiedAddress::local("alpha")),
        "hub-B/beta",
        "hi",
    );
    env.apply_snat("hub-A");
    let MessageSource::Agent(addr) = &env.source else {
        panic!("expected Agent source");
    };
    assert_eq!(addr, &QualifiedAddress::remote(["hub-A"], "alpha"));
    assert_eq!(env.target, QualifiedAddress::remote(["hub-B"], "beta"));
}

#[test]
fn test_apply_dnat_pops_target_next_hop() {
    let mut env = Envelope::new(
        MessageSource::Agent(QualifiedAddress::remote(["hub-A"], "alpha")),
        "hub-B/beta",
        "hi",
    );
    let consumed = env.apply_dnat();
    assert_eq!(consumed.as_deref(), Some("hub-B"));
    assert_eq!(env.target, QualifiedAddress::local("beta"));
}

#[test]
fn test_apply_snat_is_noop_for_non_addressable_sources() {
    // Human / Scheduled / System sources have no qualified address to
    // stamp — apply_snat must leave them untouched. This guards against
    // future MessageSource refactors accidentally promoting these
    // variants into the NAT path.
    for source in [
        MessageSource::Human,
        MessageSource::Scheduled,
        MessageSource::System("agent-completed".into()),
    ] {
        let mut env = Envelope::new(source.clone(), "main", "x");
        env.apply_snat("hub-A");
        assert_eq!(env.source, source, "non-addressable source must not change");
        // Target SNAT is not envelope-side; target only changes via DNAT.
        assert_eq!(env.target, QualifiedAddress::local("main"));
    }
}

#[test]
fn test_apply_snat_stamps_channel_from() {
    let mut env = Envelope::new(
        MessageSource::Channel {
            channel: "general".into(),
            from: QualifiedAddress::local("alpha"),
        },
        "hub-B/beta",
        "hi",
    );
    env.apply_snat("hub-A");
    let MessageSource::Channel { from, .. } = &env.source else {
        panic!("expected Channel source");
    };
    assert_eq!(from, &QualifiedAddress::remote(["hub-A"], "alpha"));
}

#[test]
fn test_snat_dnat_compose_for_round_trip() {
    // α in hub-A → β in hub-B
    let mut out = Envelope::new(
        MessageSource::Agent(QualifiedAddress::local("alpha")),
        "hub-B/beta",
        "ping",
    );
    out.apply_snat("hub-A"); // hub-A uplink stamps source

    // arrives at hub-B; meta-hub strips next hop in target
    out.apply_dnat();

    // β replies using the source it received
    let reply_target = match &out.source {
        MessageSource::Agent(addr) => addr.clone(),
        _ => panic!("expected Agent source"),
    };
    let mut reply = Envelope::new(
        MessageSource::Agent(QualifiedAddress::local("beta")),
        reply_target,
        "pong",
    );
    reply.apply_snat("hub-B");
    reply.apply_dnat();

    // α receives the reply with hub-B in source
    assert_eq!(
        reply.source,
        MessageSource::Agent(QualifiedAddress::remote(["hub-B"], "beta"))
    );
    assert_eq!(reply.target, QualifiedAddress::local("alpha"));
}

#[test]
fn test_envelope_summary_default_none() {
    let env = Envelope::new(MessageSource::Human, "main", "hi");
    assert!(env.summary.is_none());
}

#[test]
fn test_envelope_with_summary_attaches_value() {
    let env = Envelope::new(MessageSource::Human, "main", "long content...").with_summary("ping");
    assert_eq!(env.summary.as_deref(), Some("ping"));
}

#[test]
fn test_envelope_summary_serde_roundtrip() {
    let env = Envelope::new(
        MessageSource::Agent(QualifiedAddress::local("a")),
        "b",
        "full body",
    )
    .with_summary("status update");
    let json = serde_json::to_string(&env).unwrap();
    assert!(json.contains("\"summary\":\"status update\""));
    let restored: Envelope = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.summary.as_deref(), Some("status update"));
}

#[test]
fn test_envelope_summary_absent_in_json_when_none() {
    let env = Envelope::new(MessageSource::Human, "main", "hi");
    let json = serde_json::to_string(&env).unwrap();
    assert!(!json.contains("summary"));
}

#[test]
fn test_envelope_summary_missing_field_deserializes_to_none() {
    let legacy = r#"{"id":"00000000-0000-0000-0000-000000000000",
        "source":"Human","target":{"hub":[],"agent":"main"},
        "content":{"text":"hi","images":[]},
        "timestamp":"2026-01-01T00:00:00Z"}"#;
    let restored: Envelope = serde_json::from_str(legacy).unwrap();
    assert!(restored.summary.is_none());
}

#[test]
fn test_human_source_is_optimistically_rendered() {
    assert!(MessageSource::Human.is_optimistically_rendered());
}

#[test]
fn test_non_human_sources_are_not_optimistically_rendered() {
    assert!(!MessageSource::Scheduled.is_optimistically_rendered());
    assert!(!MessageSource::System("rewake".into()).is_optimistically_rendered());
    assert!(!MessageSource::Agent(QualifiedAddress::local("a")).is_optimistically_rendered());
    assert!(
        !MessageSource::Channel {
            channel: "g".into(),
            from: QualifiedAddress::local("b"),
        }
        .is_optimistically_rendered()
    );
}
