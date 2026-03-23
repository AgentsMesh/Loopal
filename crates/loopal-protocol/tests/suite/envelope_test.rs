use loopal_protocol::{Envelope, MessageSource};

#[test]
fn test_envelope_new_generates_unique_ids() {
    let a = Envelope::new(MessageSource::Human, "main", "hello");
    let b = Envelope::new(MessageSource::Human, "main", "hello");
    assert_ne!(a.id, b.id);
}

#[test]
fn test_envelope_fields_stored_correctly() {
    let env = Envelope::new(
        MessageSource::Agent("researcher".into()),
        "main",
        "found results",
    );
    assert_eq!(env.source, MessageSource::Agent("researcher".into()));
    assert_eq!(env.target, "main");
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
            from: "bot".into(),
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
    let agent = MessageSource::Agent("coder".into());
    let channel = MessageSource::Channel {
        channel: "updates".into(),
        from: "notifier".into(),
    };

    // Ensure PartialEq works across variants
    assert_ne!(human, agent);
    assert_ne!(agent, channel);
    assert_eq!(human, MessageSource::Human);
}
