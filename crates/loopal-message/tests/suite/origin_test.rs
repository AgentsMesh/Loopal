use loopal_message::MessageOrigin;
use serde_json::json;

#[test]
fn origin_wire_format_is_kind_tagged() {
    let h = serde_json::to_value(&MessageOrigin::Human).unwrap();
    assert_eq!(h, json!({"kind": "human"}));

    let g = serde_json::to_value(&MessageOrigin::GovernanceCompensation).unwrap();
    assert_eq!(g, json!({"kind": "governance_compensation"}));

    let s = serde_json::to_value(&MessageOrigin::Scheduled).unwrap();
    assert_eq!(s, json!({"kind": "scheduled"}));
}

#[test]
fn origin_round_trips() {
    let orig = MessageOrigin::Agent {
        label: "hub-a:agent-x".into(),
    };
    let wire = serde_json::to_value(&orig).unwrap();
    let back: MessageOrigin = serde_json::from_value(wire).unwrap();
    assert_eq!(orig, back);
}

#[test]
fn unknown_system_kind_falls_into_other() {
    // Wire-level: kind="other" + nested label. Backward-compat for future
    // system kinds added on the producer side that consumers don't recognise.
    let wire = json!({"kind": "other", "label": "system:future_kind"});
    let parsed: MessageOrigin = serde_json::from_value(wire).unwrap();
    assert!(matches!(parsed, MessageOrigin::Other { label } if label == "system:future_kind"));
}

#[test]
fn human_is_task_boundary_and_human_input() {
    let h = MessageOrigin::Human;
    assert!(h.is_task_boundary());
    assert!(h.is_human_input());
}

#[test]
fn scheduled_is_task_boundary_but_not_human() {
    let s = MessageOrigin::Scheduled;
    assert!(s.is_task_boundary());
    assert!(!s.is_human_input());
}

#[test]
fn system_kinds_are_not_task_boundary() {
    for o in [
        MessageOrigin::GoalContinuation,
        MessageOrigin::GovernanceCompensation,
        MessageOrigin::GovernanceFeedback,
        MessageOrigin::StopFeedback,
        MessageOrigin::ConfigRefresh,
        MessageOrigin::CompactionSummary,
        MessageOrigin::CompactionRehydrate,
        MessageOrigin::Other { label: "x".into() },
    ] {
        assert!(
            !o.is_task_boundary(),
            "{o:?} must NOT be treated as task boundary"
        );
        assert!(!o.is_human_input());
    }
}

#[test]
fn agent_and_channel_are_task_boundary_not_human() {
    let a = MessageOrigin::Agent {
        label: "hub-a:agent-x".into(),
    };
    assert!(a.is_task_boundary());
    assert!(!a.is_human_input());

    let c = MessageOrigin::Channel {
        name: "main".into(),
        from: "hub-b:agent-y".into(),
    };
    assert!(c.is_task_boundary());
    assert!(!c.is_human_input());
}

#[test]
fn task_boundary_classification_is_exhaustive() {
    // Sentinel: when MessageOrigin gains a variant, the inner `match`
    // fails to compile until the author classifies it (boundary or not).
    // Then the assertion catches drift between the table and the
    // `is_task_boundary` implementation.
    fn expected(o: &MessageOrigin) -> bool {
        match o {
            MessageOrigin::Human
            | MessageOrigin::Scheduled
            | MessageOrigin::Agent { .. }
            | MessageOrigin::Channel { .. } => true,
            MessageOrigin::GoalContinuation
            | MessageOrigin::GovernanceCompensation
            | MessageOrigin::GovernanceFeedback
            | MessageOrigin::StopFeedback
            | MessageOrigin::ConfigRefresh
            | MessageOrigin::CompactionSummary
            | MessageOrigin::CompactionRehydrate
            | MessageOrigin::Other { .. } => false,
        }
    }
    let table = [
        MessageOrigin::Human,
        MessageOrigin::Scheduled,
        MessageOrigin::Agent {
            label: "addr".into(),
        },
        MessageOrigin::Channel {
            name: "n".into(),
            from: "f".into(),
        },
        MessageOrigin::GoalContinuation,
        MessageOrigin::GovernanceCompensation,
        MessageOrigin::GovernanceFeedback,
        MessageOrigin::StopFeedback,
        MessageOrigin::ConfigRefresh,
        MessageOrigin::CompactionSummary,
        MessageOrigin::CompactionRehydrate,
        MessageOrigin::Other {
            label: "future".into(),
        },
    ];
    for o in table {
        let exp = expected(&o);
        assert_eq!(
            o.is_task_boundary(),
            exp,
            "is_task_boundary drift for {o:?}: expected={exp}"
        );
    }
}

#[test]
fn compaction_artifacts_classified_consistently() {
    assert!(MessageOrigin::CompactionSummary.is_compaction_artifact());
    assert!(MessageOrigin::CompactionRehydrate.is_compaction_artifact());
    assert!(!MessageOrigin::Human.is_compaction_artifact());
    assert!(!MessageOrigin::GovernanceCompensation.is_compaction_artifact());
    assert!(!MessageOrigin::Other { label: "x".into() }.is_compaction_artifact());
}

#[test]
fn compaction_rehydrate_serializes_to_snake_case() {
    let r = serde_json::to_value(&MessageOrigin::CompactionRehydrate).unwrap();
    assert_eq!(r, serde_json::json!({"kind": "compaction_rehydrate"}));
}
