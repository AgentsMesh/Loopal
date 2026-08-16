use serde::{Deserialize, Serialize};

use super::WorkflowOutputContract;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum WorkflowOutput {
    Text(String),
    Json(serde_json::Value),
}

/// Semantic JSON Schema validation boundary. The protocol validates schema size and shape only;
/// a coordinator must provide a complete validator before committing output-node success.
pub trait WorkflowJsonValidator {
    type Error: ToString;

    fn validate(
        &self,
        schema: &serde_json::Value,
        value: &serde_json::Value,
    ) -> Result<(), Self::Error>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkflowOutputValidationError {
    ContractMismatch { expected: String, actual: String },
    TooLarge { actual_bytes: usize, max_bytes: u32 },
    SchemaViolation { detail: String },
}

pub fn validate_workflow_output<V: WorkflowJsonValidator>(
    contract: &WorkflowOutputContract,
    output: &WorkflowOutput,
    json_validator: &V,
) -> Result<(), WorkflowOutputValidationError> {
    match (contract, output) {
        (WorkflowOutputContract::Text { max_bytes }, WorkflowOutput::Text(text)) => {
            enforce_size(text.len(), *max_bytes)
        }
        (WorkflowOutputContract::Json { max_bytes, schema }, WorkflowOutput::Json(value)) => {
            let actual_bytes = serde_json::to_vec(value)
                .expect("serde_json::Value always serializes")
                .len();
            enforce_size(actual_bytes, *max_bytes)?;
            json_validator.validate(schema, value).map_err(|error| {
                WorkflowOutputValidationError::SchemaViolation {
                    detail: error.to_string(),
                }
            })
        }
        (WorkflowOutputContract::Text { .. }, WorkflowOutput::Json(_)) => {
            Err(mismatch("text", "json"))
        }
        (WorkflowOutputContract::Json { .. }, WorkflowOutput::Text(_)) => {
            Err(mismatch("json", "text"))
        }
    }
}

fn enforce_size(actual_bytes: usize, max_bytes: u32) -> Result<(), WorkflowOutputValidationError> {
    if actual_bytes <= max_bytes as usize {
        Ok(())
    } else {
        Err(WorkflowOutputValidationError::TooLarge {
            actual_bytes,
            max_bytes,
        })
    }
}

fn mismatch(expected: &str, actual: &str) -> WorkflowOutputValidationError {
    WorkflowOutputValidationError::ContractMismatch {
        expected: expected.into(),
        actual: actual.into(),
    }
}
