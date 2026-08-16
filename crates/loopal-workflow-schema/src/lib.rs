mod policy;
mod schema;

use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum WorkflowSchemaError {
    #[error("workflow JSON Schema exceeds protocol bounds")]
    Bounds,
    #[error("workflow JSON Schema dialect must be Draft 2020-12")]
    UnsupportedDialect,
    #[error("workflow JSON Schema cannot use external references")]
    ExternalReference,
    #[error("workflow JSON Schema is invalid")]
    InvalidSchema,
    #[error("workflow output does not satisfy its JSON Schema")]
    InstanceMismatch,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WorkflowSchemaValidator;

pub fn validate_workflow_schema(schema: &Value) -> Result<(), WorkflowSchemaError> {
    schema::compile(schema).map(drop)
}

pub fn validate_workflow_json(schema: &Value, value: &Value) -> Result<(), WorkflowSchemaError> {
    schema::compile(schema)?
        .validate(value)
        .map_err(|_| WorkflowSchemaError::InstanceMismatch)
}

impl loopal_protocol::WorkflowJsonValidator for WorkflowSchemaValidator {
    type Error = WorkflowSchemaError;

    fn validate(&self, schema: &Value, value: &Value) -> Result<(), Self::Error> {
        validate_workflow_json(schema, value)
    }
}
