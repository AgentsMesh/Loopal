use loopal_message::MessageOrigin;
use loopal_protocol::{MessageSource, QualifiedAddress};

fn all_variants() -> Vec<MessageSource> {
    vec![
        MessageSource::Human,
        MessageSource::Scheduled,
        MessageSource::Agent(QualifiedAddress::local("alpha")),
        MessageSource::Channel {
            channel: "general".into(),
            from: QualifiedAddress::local("bot"),
        },
        MessageSource::System("goal_continuation".into()),
        MessageSource::System("governance_compensation".into()),
        MessageSource::System("governance_feedback".into()),
        MessageSource::System("stop_feedback".into()),
        MessageSource::System("config_refresh".into()),
        MessageSource::System("compaction_summary".into()),
        MessageSource::System("compaction_rehydrate".into()),
        MessageSource::System("unrecognised_label".into()),
    ]
}

#[test]
fn human_scheduled_agent_channel_are_boundaries() {
    assert!(MessageSource::Human.is_task_boundary());
    assert!(MessageSource::Scheduled.is_task_boundary());
    assert!(MessageSource::Agent(QualifiedAddress::local("a")).is_task_boundary());
    assert!(
        MessageSource::Channel {
            channel: "g".into(),
            from: QualifiedAddress::local("b"),
        }
        .is_task_boundary()
    );
}

#[test]
fn system_sources_are_not_boundaries() {
    for kind in [
        "goal_continuation",
        "governance_compensation",
        "governance_feedback",
        "stop_feedback",
        "config_refresh",
        "compaction_summary",
        "compaction_rehydrate",
        "unrecognised",
    ] {
        assert!(
            !MessageSource::System(kind.into()).is_task_boundary(),
            "System({kind}) must not be a task boundary"
        );
    }
}

// SSOT invariant: `MessageSource::is_task_boundary` (hot-path predicate used
// by LoopDetector::on_envelope_received) must agree with the typed
// projection through `MessageOrigin::is_task_boundary` for every variant.
#[test]
fn message_source_predicate_matches_origin_projection() {
    for source in all_variants() {
        let projected = MessageOrigin::from(&source);
        assert_eq!(
            source.is_task_boundary(),
            projected.is_task_boundary(),
            "predicate divergence at {source:?} → {projected:?}"
        );
    }
}
