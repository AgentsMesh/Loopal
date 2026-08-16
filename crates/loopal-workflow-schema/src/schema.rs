use serde_json::Value;

use crate::{WorkflowSchemaError, policy};

const REGEX_COMPILED_BYTES: usize = 256 * 1_024;
const REGEX_DFA_BYTES: usize = 256 * 1_024;

struct DenyExternalReferences;

impl jsonschema::Retrieve for DenyExternalReferences {
    fn retrieve(
        &self,
        _uri: &jsonschema::Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Err(Box::new(WorkflowSchemaError::ExternalReference))
    }
}

pub(crate) fn compile(schema: &Value) -> Result<jsonschema::Validator, WorkflowSchemaError> {
    loopal_protocol::validate_workflow_schema_bounds(schema)
        .map_err(|_| WorkflowSchemaError::Bounds)?;
    policy::validate(schema)?;
    jsonschema::draft202012::meta::validate(schema)
        .map_err(|_| WorkflowSchemaError::InvalidSchema)?;
    jsonschema::draft202012::options()
        .with_retriever(DenyExternalReferences)
        .with_pattern_options(
            jsonschema::PatternOptions::regex()
                .size_limit(REGEX_COMPILED_BYTES)
                .dfa_size_limit(REGEX_DFA_BYTES),
        )
        .should_validate_formats(true)
        .build(schema)
        .map_err(|_| WorkflowSchemaError::InvalidSchema)
}
