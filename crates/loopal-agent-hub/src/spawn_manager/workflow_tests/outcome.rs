use loopal_protocol::{
    AgentCompletion, WorkflowFailureClass, WorkflowOutput, WorkflowOutputContract,
};

use crate::workflow::scheduler::WorkflowWorkerOutcome;

fn contract() -> WorkflowOutputContract {
    WorkflowOutputContract::Json {
        max_bytes: 256,
        schema: serde_json::json!({
            "type": "object",
            "required": ["answer"],
            "properties": {"answer": {"type": "string"}},
            "additionalProperties": false
        }),
    }
}

#[test]
fn malformed_json_output_fails_closed() {
    let outcome = super::super::outcome::worker(
        AgentCompletion::goal(Some("not-json".into())),
        Some(contract()),
    );

    let WorkflowWorkerOutcome::Failed(failure) = outcome else {
        panic!("invalid JSON must fail the workflow attempt")
    };
    assert_eq!(failure.completion.reason, "workflow_output_rejected");
    assert_eq!(failure.failure.class, WorkflowFailureClass::Permanent);
    assert!(failure.failure.reason.contains("not valid JSON"));
}

#[test]
fn schema_mismatch_and_missing_result_fail_closed() {
    for completion in [
        AgentCompletion::goal(Some(r#"{"answer": 7}"#.into())),
        AgentCompletion::goal(None),
    ] {
        let WorkflowWorkerOutcome::Failed(failure) =
            super::super::outcome::worker(completion, Some(contract()))
        else {
            panic!("contract violation must fail the workflow attempt")
        };
        assert_eq!(failure.completion.reason, "workflow_output_rejected");
    }
}

#[test]
fn valid_json_output_remains_typed() {
    let completion = AgentCompletion::goal(Some(r#"{"answer":"done"}"#.into()));
    let WorkflowWorkerOutcome::Succeeded {
        completion: returned,
        output: Some(WorkflowOutput::Json(value)),
    } = super::super::outcome::worker(completion.clone(), Some(contract()))
    else {
        panic!("valid JSON must remain a typed workflow output")
    };
    assert_eq!(returned, completion);
    assert_eq!(value, serde_json::json!({"answer": "done"}));
}
