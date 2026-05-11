use loopal_protocol::{AgentEventPayload, UserQuestionResponse};
use loopal_runtime::frontend::permission_handler::PermissionOutcome;
use loopal_runtime::frontend::question_handler::QuestionOutcome;
use loopal_runtime::frontend::{into_permission_decided, into_question_decided};
use loopal_tool_api::PermissionDecision;

#[test]
fn permission_decided_preserves_decision_and_fields() {
    let outcome = PermissionOutcome {
        decision: PermissionDecision::Allow,
        reason: "user approved".into(),
        duration_ms: 42,
    };
    let (decision, payload) = into_permission_decided("Bash", outcome);
    assert_eq!(decision, PermissionDecision::Allow);
    match payload {
        AgentEventPayload::PermissionDecided {
            tool_name,
            decision,
            reason,
            duration_ms,
        } => {
            assert_eq!(tool_name, "Bash");
            assert_eq!(decision, "allow");
            assert_eq!(reason, "user approved");
            assert_eq!(duration_ms, 42);
        }
        other => panic!("expected PermissionDecided, got {other:?}"),
    }
}

#[test]
fn permission_decided_serializes_deny_as_lowercase() {
    let outcome = PermissionOutcome::deny("dangerous");
    let (_, payload) = into_permission_decided("rm -rf", outcome);
    if let AgentEventPayload::PermissionDecided { decision, .. } = payload {
        assert_eq!(decision, "deny");
    } else {
        panic!("expected PermissionDecided");
    }
}

#[test]
fn question_decided_preserves_response_and_count() {
    let outcome = QuestionOutcome::cancelled("q-1", "timed out");
    let (response, payload) = into_question_decided(3, outcome);
    assert!(matches!(response, UserQuestionResponse::Cancelled { .. }));
    match payload {
        AgentEventPayload::QuestionDecided {
            question_count,
            duration_ms,
            reason,
        } => {
            assert_eq!(question_count, 3);
            assert_eq!(duration_ms, 0);
            assert_eq!(reason, "timed out");
        }
        other => panic!("expected QuestionDecided, got {other:?}"),
    }
}
