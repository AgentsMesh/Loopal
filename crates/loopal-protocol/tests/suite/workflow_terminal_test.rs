use loopal_protocol::{
    MAX_WORKFLOW_TERMINAL_CONTENT_BYTES, MAX_WORKFLOW_TERMINAL_DETAIL_BYTES,
    MAX_WORKFLOW_TERMINAL_GOAL_BYTES, WorkflowFailureClass, WorkflowRunId, WorkflowRunState,
    WorkflowTerminalDeliveryId, WorkflowTerminalDisposition, WorkflowTerminalNotification,
    WorkflowTerminalOutcome, WorkflowTerminalValidationError, truncate_workflow_terminal_text,
};

fn success() -> WorkflowTerminalNotification {
    WorkflowTerminalNotification {
        delivery_id: WorkflowTerminalDeliveryId::new(
            "session-one",
            WorkflowRunId::new("wrun_one"),
            7,
        ),
        state: WorkflowRunState::Succeeded,
        run_goal: "ship it".into(),
        outcome: WorkflowTerminalOutcome::Succeeded {
            result: "done".into(),
        },
        content: "Workflow completed: done".into(),
    }
}

#[test]
fn delivery_identity_round_trips_all_deterministic_fields() {
    let notification = success();
    notification.validate().unwrap();
    let encoded = serde_json::to_value(&notification).unwrap();
    let decoded: WorkflowTerminalNotification = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded.delivery_id, notification.delivery_id);
    assert_eq!(decoded.state, WorkflowRunState::Succeeded);
    assert_eq!(decoded.payload_digest(), notification.payload_digest());
    let mut changed = notification.clone();
    changed.content.push('!');
    assert_ne!(changed.payload_digest(), notification.payload_digest());
}

#[test]
fn identity_and_terminal_state_fail_closed() {
    let mut notification = success();
    notification.delivery_id.terminal_revision = 0;
    assert_eq!(
        notification.validate(),
        Err(WorkflowTerminalValidationError::InvalidTerminalRevision)
    );
    notification.delivery_id.terminal_revision = 1;
    notification.state = WorkflowRunState::Failed;
    notification.outcome = WorkflowTerminalOutcome::Failed {
        class: WorkflowFailureClass::Permanent,
        reason: "failed".into(),
    };
    assert!(notification.validate().is_ok());
    notification.state = WorkflowRunState::Running;
    assert_eq!(
        notification.validate(),
        Err(WorkflowTerminalValidationError::StateMismatch)
    );
}

#[test]
fn session_id_matches_safe_path_component_contract() {
    for session_id in ["session.with.dots", "会话-一"] {
        let mut notification = success();
        notification.delivery_id.session_id = session_id.into();
        assert!(notification.validate().is_ok(), "{session_id}");
    }

    for session_id in ["", ".", "..", "session/one", "session\\one"] {
        let mut notification = success();
        notification.delivery_id.session_id = session_id.into();
        assert_eq!(
            notification.validate(),
            Err(WorkflowTerminalValidationError::InvalidSessionId),
            "{session_id:?}"
        );
    }

    let mut notification = success();
    notification.delivery_id.session_id = "界".repeat(43);
    assert!(notification.delivery_id.session_id.len() > 128);
    assert_eq!(
        notification.validate(),
        Err(WorkflowTerminalValidationError::InvalidSessionId)
    );
}

#[test]
fn wire_rejects_unknown_fields_and_disposition_is_typed() {
    let mut value = serde_json::to_value(success()).unwrap();
    value["unexpected"] = serde_json::json!(true);
    assert!(serde_json::from_value::<WorkflowTerminalNotification>(value).is_err());
    for disposition in [
        WorkflowTerminalDisposition::Applied,
        WorkflowTerminalDisposition::AlreadyApplied,
        WorkflowTerminalDisposition::Queued,
        WorkflowTerminalDisposition::Retryable {
            reason: "try again".into(),
        },
        WorkflowTerminalDisposition::Rejected {
            reason: "no".into(),
        },
    ] {
        let value = serde_json::to_value(&disposition).unwrap();
        let decoded: WorkflowTerminalDisposition = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, disposition);
    }
}

#[test]
fn truncation_is_utf8_safe_bounded_and_visible() {
    let source = "界".repeat(MAX_WORKFLOW_TERMINAL_CONTENT_BYTES);
    let bounded = truncate_workflow_terminal_text(&source, MAX_WORKFLOW_TERMINAL_CONTENT_BYTES);
    assert!(bounded.len() <= MAX_WORKFLOW_TERMINAL_CONTENT_BYTES);
    assert!(bounded.ends_with("[workflow result truncated]"));
    assert!(std::str::from_utf8(bounded.as_bytes()).is_ok());
}

#[test]
fn terminal_identity_outcome_and_size_bounds_fail_closed() {
    let mut invalid_run = success();
    invalid_run.delivery_id.run_id = WorkflowRunId::new("../escape");
    assert_eq!(
        invalid_run.validate(),
        Err(WorkflowTerminalValidationError::InvalidRunId)
    );

    let mut mismatched = success();
    mismatched.state = WorkflowRunState::Failed;
    assert_eq!(
        mismatched.validate(),
        Err(WorkflowTerminalValidationError::StateMismatch)
    );

    let oversized = [
        ("run_goal", MAX_WORKFLOW_TERMINAL_GOAL_BYTES + 1),
        ("outcome", MAX_WORKFLOW_TERMINAL_DETAIL_BYTES + 1),
        ("content", MAX_WORKFLOW_TERMINAL_CONTENT_BYTES + 1),
    ];
    for (field, size) in oversized {
        let mut notification = success();
        match field {
            "run_goal" => notification.run_goal = "x".repeat(size),
            "outcome" => {
                notification.outcome = WorkflowTerminalOutcome::Succeeded {
                    result: "x".repeat(size),
                }
            }
            "content" => notification.content = "x".repeat(size),
            _ => unreachable!(),
        }
        assert_eq!(
            notification.validate(),
            Err(WorkflowTerminalValidationError::TooLarge {
                field,
                actual_bytes: size,
                max_bytes: size - 1,
            })
        );
    }
}

#[test]
fn terminal_validation_errors_and_short_truncation_are_stable() {
    let errors = [
        (
            WorkflowTerminalValidationError::InvalidSessionId,
            "invalid workflow terminal session id".to_string(),
        ),
        (
            WorkflowTerminalValidationError::InvalidRunId,
            "invalid workflow terminal run id".to_string(),
        ),
        (
            WorkflowTerminalValidationError::InvalidTerminalRevision,
            "workflow terminal revision must be greater than zero".to_string(),
        ),
        (
            WorkflowTerminalValidationError::StateMismatch,
            "workflow terminal state does not match outcome".to_string(),
        ),
        (
            WorkflowTerminalValidationError::TooLarge {
                field: "content",
                actual_bytes: 9,
                max_bytes: 8,
            },
            "workflow terminal content is 9 bytes; limit is 8".to_string(),
        ),
    ];
    for (error, expected) in errors {
        assert_eq!(error.to_string(), expected);
    }

    assert_eq!(truncate_workflow_terminal_text("short", 5), "short");
    assert_eq!(truncate_workflow_terminal_text("long", 0), "");
    assert!(truncate_workflow_terminal_text("boundary", 1).len() <= 1);
}
