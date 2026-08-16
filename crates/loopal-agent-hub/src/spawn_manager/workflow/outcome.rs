use loopal_protocol::{
    AgentCompletion, WorkflowAttemptFailure, WorkflowFailureClass, WorkflowOutput,
    WorkflowOutputContract,
};

use crate::workflow::scheduler::{WorkflowSpawnFailure, WorkflowWorkerOutcome};

pub(super) fn worker(
    completion: AgentCompletion,
    contract: Option<WorkflowOutputContract>,
) -> WorkflowWorkerOutcome {
    if !completion.is_success() {
        return failed(completion, WorkflowFailureClass::Permanent, None);
    }
    match parse_output(&completion, contract) {
        Ok(output) => WorkflowWorkerOutcome::Succeeded { completion, output },
        Err(reason) => failed(
            AgentCompletion::new("workflow_output_rejected", None),
            WorkflowFailureClass::Permanent,
            Some(reason),
        ),
    }
}

fn parse_output(
    completion: &AgentCompletion,
    contract: Option<WorkflowOutputContract>,
) -> Result<Option<WorkflowOutput>, String> {
    let Some(contract) = contract else {
        return Ok(None);
    };
    let value = completion.result.as_ref().ok_or_else(|| {
        "workflow output node completed without an authoritative result".to_string()
    })?;
    if value.len() > contract.max_bytes() as usize {
        return Err("workflow output exceeds its contract byte limit".into());
    }
    match contract {
        WorkflowOutputContract::Text { .. } => Ok(Some(WorkflowOutput::Text(value.clone()))),
        WorkflowOutputContract::Json { schema, .. } => {
            let json = serde_json::from_str(value)
                .map_err(|_| "workflow output is not valid JSON".to_string())?;
            loopal_workflow_schema::validate_workflow_json(&schema, &json)
                .map_err(|_| "workflow output does not satisfy its JSON Schema".to_string())?;
            Ok(Some(WorkflowOutput::Json(json)))
        }
    }
}

fn failed(
    completion: AgentCompletion,
    class: WorkflowFailureClass,
    reason: Option<String>,
) -> WorkflowWorkerOutcome {
    let reason = reason.unwrap_or_else(|| completion.failure_detail().to_string());
    WorkflowWorkerOutcome::Failed(WorkflowSpawnFailure {
        completion,
        failure: WorkflowAttemptFailure { class, reason },
    })
}
