use crate::workflow_support::{node, text_spec};
use loopal_protocol::{
    MAX_WORKFLOW_DEPENDENCY_RESULTS_BYTES, MAX_WORKFLOW_OUTPUT_BYTES, WorkflowValidationError,
    validate_workflow_spec,
};

#[test]
fn dependency_result_budget_is_enforced_at_admission() {
    let mut spec = text_spec();
    let dependencies: Vec<String> = (0..8).map(|index| format!("source_{index}")).collect();
    spec.nodes = dependencies
        .iter()
        .map(|id| node(id, &[]))
        .chain(std::iter::once(node(
            "output",
            &dependencies.iter().map(String::as_str).collect::<Vec<_>>(),
        )))
        .collect();
    spec.limits.max_nodes = 10;
    spec.limits.max_attempts = 10;
    spec.limits.max_output_bytes = MAX_WORKFLOW_OUTPUT_BYTES;
    spec.output_contract = loopal_protocol::WorkflowOutputContract::Text {
        max_bytes: MAX_WORKFLOW_OUTPUT_BYTES,
    };

    validate_workflow_spec(&spec).unwrap();

    spec.nodes.push(node("source_8", &[]));
    spec.nodes
        .iter_mut()
        .find(|node| node.id.as_str() == "output")
        .unwrap()
        .dependencies
        .push("source_8".into());

    assert_eq!(
        validate_workflow_spec(&spec),
        Err(WorkflowValidationError::DependencyOutputBudgetExceeded {
            node_id: "output".into(),
            potential_bytes: 9 * u64::from(MAX_WORKFLOW_OUTPUT_BYTES),
            max_bytes: MAX_WORKFLOW_DEPENDENCY_RESULTS_BYTES,
        })
    );
}
