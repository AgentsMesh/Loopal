use loopal_provider_api::MessageOrigin;

#[test]
fn task_boundaries_cover_external_and_internal_origins() {
    let boundaries = [
        MessageOrigin::Human,
        MessageOrigin::HumanSkill {
            name: "review".into(),
            user_args: "changes".into(),
        },
        MessageOrigin::Scheduled,
        MessageOrigin::Agent {
            label: "root/worker".into(),
        },
        MessageOrigin::Channel {
            name: "ops".into(),
            from: "alice".into(),
        },
        MessageOrigin::WorkflowResult {
            run_id: "run-1".into(),
            terminal_revision: 4,
            state: "succeeded".into(),
        },
    ];
    assert!(boundaries.iter().all(MessageOrigin::is_task_boundary));

    let internal = [
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
    assert!(internal.iter().all(|origin| !origin.is_task_boundary()));
}

#[test]
fn human_and_compaction_classifiers_are_exact() {
    assert!(MessageOrigin::Human.is_human_input());
    assert!(
        MessageOrigin::HumanSkill {
            name: "review".into(),
            user_args: String::new(),
        }
        .is_human_input()
    );
    assert!(!MessageOrigin::Scheduled.is_human_input());

    assert!(MessageOrigin::CompactionSummary.is_compaction_artifact());
    assert!(MessageOrigin::CompactionRehydrate.is_compaction_artifact());
    assert!(!MessageOrigin::ConfigRefresh.is_compaction_artifact());
}
