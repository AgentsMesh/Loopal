use loopal_runtime::agent_loop::governance::{
    AggregatedVerdict, FirstDenyWins, Verdict, VerdictAggregator,
};

#[test]
fn empty_chain_yields_continue() {
    let agg = FirstDenyWins;
    let result = agg.aggregate(vec![]);
    assert!(matches!(result, AggregatedVerdict::Continue));
}

#[test]
fn all_continue_yields_continue() {
    let agg = FirstDenyWins;
    let result = agg.aggregate(vec![Verdict::Continue, Verdict::Continue]);
    assert!(matches!(result, AggregatedVerdict::Continue));
}

#[test]
fn warnings_collected_in_order() {
    let agg = FirstDenyWins;
    let result = agg.aggregate(vec![
        Verdict::InjectWarning("first".into()),
        Verdict::Continue,
        Verdict::InjectWarning("second".into()),
    ]);
    let AggregatedVerdict::Warnings(msgs) = result else {
        panic!("expected Warnings, got {result:?}");
    };
    assert_eq!(msgs, vec!["first".to_string(), "second".to_string()]);
}

#[test]
fn first_abort_short_circuits() {
    let agg = FirstDenyWins;
    let result = agg.aggregate(vec![
        Verdict::Continue,
        Verdict::AbortTurn {
            reason: "first abort".into(),
            feedback_to_model: "stop".into(),
        },
        // This abort is never reached.
        Verdict::AbortTurn {
            reason: "second abort".into(),
            feedback_to_model: "different".into(),
        },
    ]);
    let AggregatedVerdict::Abort {
        reason, feedback_to_model
    } = result else {
        panic!("expected Abort, got {result:?}");
    };
    assert_eq!(reason, "first abort");
    assert_eq!(feedback_to_model, "stop");
}

#[test]
fn warning_then_abort_discards_warnings() {
    // Once a chain decides to abort, prior warnings become moot — the abort
    // compensation message itself carries the explanation. This codifies
    // the convention so consumers can rely on AggregatedVerdict::Abort
    // being mutually exclusive with Warnings.
    let agg = FirstDenyWins;
    let result = agg.aggregate(vec![
        Verdict::InjectWarning("about to be aborted anyway".into()),
        Verdict::AbortTurn {
            reason: "loop detected".into(),
            feedback_to_model: "stop retrying".into(),
        },
    ]);
    assert!(matches!(result, AggregatedVerdict::Abort { .. }));
}

#[test]
fn custom_aggregator_can_replace_default() {
    // Demonstrates the mechanism/policy split: a non-default aggregator
    // can implement a different policy without dispatcher changes.
    struct AbortIfAnyWarning;
    impl VerdictAggregator for AbortIfAnyWarning {
        fn aggregate(&self, verdicts: Vec<Verdict>) -> AggregatedVerdict {
            for v in &verdicts {
                if matches!(v, Verdict::InjectWarning(_)) {
                    return AggregatedVerdict::Abort {
                        reason: "warning escalated".into(),
                        feedback_to_model: "any warning aborts under this policy".into(),
                    };
                }
            }
            AggregatedVerdict::Continue
        }
    }
    let agg = AbortIfAnyWarning;
    let result = agg.aggregate(vec![Verdict::InjectWarning("x".into())]);
    assert!(matches!(result, AggregatedVerdict::Abort { .. }));
}
