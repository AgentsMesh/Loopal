use loopal_protocol::WorkflowOutputContract;

use super::WorkflowCoordinatorError;

pub(super) fn validate_output_contract(
    contract: &WorkflowOutputContract,
) -> Result<(), WorkflowCoordinatorError> {
    if let WorkflowOutputContract::Json { schema, .. } = contract {
        loopal_workflow_schema::validate_workflow_schema(schema)?;
    }
    Ok(())
}
