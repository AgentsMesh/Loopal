use loopal_protocol::{AgentCompletion, WaitAgentResponse, WaitAgentStatus};

#[test]
fn completion_result_roundtrips_with_stable_wire_shape() {
    let completion = AgentCompletion {
        reason: "goal".into(),
        result: Some("authoritative result".into()),
    };

    let value = serde_json::to_value(&completion).unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "reason": "goal",
            "result": "authoritative result",
        })
    );
    assert_eq!(
        serde_json::from_value::<AgentCompletion>(value).unwrap(),
        completion
    );
}

#[test]
fn legacy_completion_without_result_decodes_as_none() {
    let completion: AgentCompletion =
        serde_json::from_value(serde_json::json!({"reason": "shutdown"})).unwrap();

    assert_eq!(completion.reason, "shutdown");
    assert_eq!(completion.result, None);
}

#[test]
fn completion_success_is_fail_closed_for_unknown_reasons() {
    assert!(AgentCompletion::goal(Some("done".into())).is_success());
    assert!(!AgentCompletion::new("end_turn", None).is_success());
    assert!(!AgentCompletion::new("error", None).is_success());
    assert!(!AgentCompletion::new("aborted", None).is_success());
    assert!(!AgentCompletion::new("future_failure", None).is_success());
}

#[test]
fn wait_response_preserves_success_output_wire_compatibility() {
    let response = WaitAgentResponse::from_completion(AgentCompletion::goal(Some(String::new())));

    assert_eq!(response.status, WaitAgentStatus::Completed);
    assert_eq!(response.reason, "goal");
    assert_eq!(response.output, "");
    assert_eq!(
        serde_json::to_value(response).unwrap(),
        serde_json::json!({
            "status": "completed",
            "reason": "goal",
            "output": "",
        })
    );
}

#[test]
fn wait_response_distinguishes_all_terminal_statuses() {
    let failed = WaitAgentResponse::from_completion(AgentCompletion::new(
        "error",
        Some("partial result".into()),
    ));
    assert_eq!(failed.status, WaitAgentStatus::Failed);
    assert_eq!(failed.reason, "error");
    assert_eq!(failed.output, "partial result");

    let timed_out = WaitAgentResponse::timed_out();
    assert_eq!(timed_out.status, WaitAgentStatus::TimedOut);
    assert!(timed_out.timed_out);
    assert_eq!(serde_json::to_value(timed_out).unwrap()["timed_out"], true);
    assert_eq!(
        WaitAgentResponse::not_found().status,
        WaitAgentStatus::NotFound
    );
}

#[test]
fn legacy_projection_marks_every_non_success_without_changing_typed_output() {
    let responses = [
        WaitAgentResponse::from_completion(AgentCompletion::new(
            "error",
            Some("partial result".into()),
        )),
        WaitAgentResponse::timed_out(),
        WaitAgentResponse::not_found(),
    ];

    for response in responses {
        let raw_output = response.output.clone();
        let projected = response.legacy_safe_output();
        assert!(
            projected.starts_with(&format!(
                "[agent completion failed; reason: {}]",
                response.reason
            )),
            "legacy failure marker missing: {projected:?}"
        );
        assert!(projected.contains(&raw_output));
        assert_eq!(response.output, raw_output, "typed response was mutated");
    }

    let success = WaitAgentResponse::from_completion(AgentCompletion::goal(Some("done".into())));
    assert_eq!(success.legacy_safe_output(), "done");
}
