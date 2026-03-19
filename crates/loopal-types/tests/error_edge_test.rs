use loopal_types::error::{
    AgentOutput, LoopalError, ProviderError, TerminateReason,
};

// --- ContextOverflow ---

#[test]
fn test_context_overflow_not_retryable() {
    let err = ProviderError::ContextOverflow {
        message: "prompt is too long".into(),
    };
    assert!(!err.is_retryable());
}

#[test]
fn test_context_overflow_is_context_overflow() {
    let err = ProviderError::ContextOverflow {
        message: "prompt is too long".into(),
    };
    assert!(err.is_context_overflow());
}

#[test]
fn test_api_400_prompt_too_long_is_context_overflow() {
    let err = ProviderError::Api {
        status: 400,
        message: "prompt is too long for model".into(),
    };
    assert!(err.is_context_overflow());
    assert!(!err.is_retryable());
}

#[test]
fn test_api_400_max_context_is_context_overflow() {
    let err = ProviderError::Api {
        status: 400,
        message: "maximum context length exceeded".into(),
    };
    assert!(err.is_context_overflow());
    assert!(!err.is_retryable());
}

#[test]
fn test_api_400_invalid_request_not_retryable() {
    let err = ProviderError::Api {
        status: 400,
        message: r#"{"type":"error","error":{"type":"invalid_request_error"}}"#.into(),
    };
    assert!(!err.is_retryable());
}

#[test]
fn test_api_500_not_context_overflow() {
    let err = ProviderError::Api {
        status: 500,
        message: "internal".into(),
    };
    assert!(!err.is_context_overflow());
    assert!(err.is_retryable());
}

#[test]
fn test_loopal_context_overflow_delegation() {
    let err = LoopalError::Provider(ProviderError::ContextOverflow {
        message: "overflow".into(),
    });
    assert!(err.is_context_overflow());
    assert!(!err.is_retryable());
}

#[test]
fn test_context_overflow_display() {
    let err = ProviderError::ContextOverflow {
        message: "too big".into(),
    };
    assert_eq!(format!("{err}"), "Context overflow: too big");
}

// --- TerminateReason ---

#[test]
fn test_terminate_reason_equality() {
    assert_eq!(TerminateReason::Goal, TerminateReason::Goal);
    assert_eq!(TerminateReason::Error, TerminateReason::Error);
    assert_eq!(TerminateReason::MaxTurns, TerminateReason::MaxTurns);
    assert_eq!(TerminateReason::Aborted, TerminateReason::Aborted);
    assert_ne!(TerminateReason::Goal, TerminateReason::Error);
    assert_ne!(TerminateReason::MaxTurns, TerminateReason::Aborted);
}

#[test]
fn test_terminate_reason_clone() {
    let reason = TerminateReason::Goal;
    let cloned = reason.clone();
    assert_eq!(reason, cloned);
}

#[test]
fn test_terminate_reason_debug() {
    assert_eq!(format!("{:?}", TerminateReason::Goal), "Goal");
    assert_eq!(format!("{:?}", TerminateReason::Error), "Error");
    assert_eq!(format!("{:?}", TerminateReason::MaxTurns), "MaxTurns");
    assert_eq!(format!("{:?}", TerminateReason::Aborted), "Aborted");
}

// --- AgentOutput ---

#[test]
fn test_agent_output_construction() {
    let output = AgentOutput {
        result: "hello".to_string(),
        terminate_reason: TerminateReason::Goal,
    };
    assert_eq!(output.result, "hello");
    assert_eq!(output.terminate_reason, TerminateReason::Goal);
}

#[test]
fn test_agent_output_clone() {
    let output = AgentOutput {
        result: "data".to_string(),
        terminate_reason: TerminateReason::Error,
    };
    let cloned = output.clone();
    assert_eq!(cloned.result, "data");
    assert_eq!(cloned.terminate_reason, TerminateReason::Error);
}

#[test]
fn test_agent_output_empty_result_on_error() {
    let output = AgentOutput {
        result: String::new(),
        terminate_reason: TerminateReason::Error,
    };
    assert!(output.result.is_empty());
    assert_eq!(output.terminate_reason, TerminateReason::Error);
}

#[test]
fn test_agent_output_non_empty_result_on_max_turns() {
    let output = AgentOutput {
        result: "partial work".to_string(),
        terminate_reason: TerminateReason::MaxTurns,
    };
    assert_eq!(output.result, "partial work");
    assert_eq!(output.terminate_reason, TerminateReason::MaxTurns);
}
