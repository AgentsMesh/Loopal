use loopal_output_guard::{
    MAX_AGENT_COMPLETION_REASON_BYTES, MAX_AGENT_COMPLETION_RESULT_BYTES,
    MAX_AGENT_EVENT_PAYLOAD_BYTES, OUTPUT_GUARD_REJECTED_REASON, guard_agent_completion,
    guard_agent_completion_with_result_limit, guard_agent_event, guard_or_reject_agent_completion,
    guard_or_reject_agent_event,
};
use loopal_protocol::{AgentCompletion, AgentEvent, AgentEventPayload};
use secrecy::SecretString;

fn seed() -> Vec<(String, SecretString)> {
    vec![("token".into(), SecretString::from("plaintext-canary"))]
}

#[test]
fn completion_guard_redacts_reason_and_result() {
    let completion = AgentCompletion::new(
        "failed: plaintext-canary",
        Some("result plaintext-canary".into()),
    );
    let guarded = guard_agent_completion(completion, &seed())
        .unwrap()
        .into_completion();

    assert_eq!(guarded.reason, "failed: <secret_ref:token>");
    assert_eq!(guarded.result.as_deref(), Some("result <secret_ref:token>"));
}

#[test]
fn empty_seed_completion_and_event_pass_within_exact_limits() {
    let completion = AgentCompletion::new(
        "r".repeat(MAX_AGENT_COMPLETION_REASON_BYTES),
        Some("o".repeat(MAX_AGENT_COMPLETION_RESULT_BYTES)),
    );
    let guarded = guard_agent_completion(completion.clone(), &[]).unwrap();
    assert_eq!(guarded.as_completion(), &completion);

    let event = AgentEvent::root(AgentEventPayload::Stream { text: "ok".into() });
    assert!(matches!(
        guard_agent_event(event, &[]).unwrap().into_event().payload,
        AgentEventPayload::Stream { ref text } if text == "ok"
    ));
}

#[test]
fn result_limit_is_checked_independently_of_reason() {
    let completion = AgentCompletion::new(
        "goal",
        Some("y".repeat(MAX_AGENT_COMPLETION_RESULT_BYTES + 1)),
    );

    assert!(guard_agent_completion(completion, &[]).is_err());
}

#[test]
fn workflow_result_limit_can_exceed_the_generic_agent_limit() {
    let workflow_limit = MAX_AGENT_COMPLETION_RESULT_BYTES + 17;
    let exact = AgentCompletion::goal(Some("w".repeat(workflow_limit)));
    assert!(
        guard_agent_completion_with_result_limit(exact, &[], workflow_limit).is_ok(),
        "the trusted workflow transport limit must replace the generic agent limit"
    );

    let oversized = AgentCompletion::goal(Some("w".repeat(workflow_limit + 1)));
    assert!(guard_agent_completion_with_result_limit(oversized, &[], workflow_limit).is_err());
}

#[test]
fn completion_limits_fail_closed() {
    let oversized = AgentCompletion::new(
        "x".repeat(MAX_AGENT_COMPLETION_REASON_BYTES + 1),
        Some("y".repeat(MAX_AGENT_COMPLETION_RESULT_BYTES + 1)),
    );
    assert!(guard_agent_completion(oversized.clone(), &[]).is_err());

    let rejected = guard_or_reject_agent_completion(oversized, &[]).into_completion();
    assert_eq!(rejected.reason, OUTPUT_GUARD_REJECTED_REASON);
    assert!(!rejected.output().contains(&"x".repeat(32)));
    assert!(!rejected.output().contains(&"y".repeat(32)));
}

#[test]
fn event_guard_redacts_json_and_preserves_metadata() {
    let mut event = AgentEvent::named(
        "worker",
        AgentEventPayload::ToolCall {
            id: "call".into(),
            name: "Read".into(),
            input: serde_json::json!({"key": "plaintext-canary"}),
        },
    );
    event.event_id = 7;
    event.turn_id = 8;
    event.correlation_id = 9;
    event.rev = Some(10);
    event.routing_generation = Some(11);

    let guarded = guard_agent_event(event, &seed()).unwrap().into_event();
    assert_eq!(guarded.event_id, 7);
    assert_eq!(guarded.turn_id, 8);
    assert_eq!(guarded.correlation_id, 9);
    assert_eq!(guarded.rev, Some(10));
    assert_eq!(guarded.routing_generation, Some(11));
    let AgentEventPayload::ToolCall { input, .. } = guarded.payload else {
        panic!("expected tool call");
    };
    assert_eq!(input["key"], "<secret_ref:token>");
}

#[test]
fn oversized_event_is_replaced_without_raw_content() {
    let event = AgentEvent::named(
        "worker",
        AgentEventPayload::Stream {
            text: "z".repeat(MAX_AGENT_EVENT_PAYLOAD_BYTES + 1),
        },
    );
    let guarded = guard_or_reject_agent_event(event, &[]).into_event();

    assert!(matches!(
        guarded.payload,
        AgentEventPayload::Error { ref message }
            if message == "agent event rejected by output guard"
    ));
}

#[test]
fn redacted_json_key_collision_is_rejected() {
    let event = AgentEvent::root(AgentEventPayload::ServerToolResult {
        tool_use_id: "server".into(),
        content: serde_json::json!({"plaintext-canary": 1, "<secret_ref:token>": 2}),
    });

    assert!(guard_agent_event(event, &seed()).is_err());
}
