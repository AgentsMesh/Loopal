use loopal_error::Result;
use loopal_tool_api::{ImageOutputPolicy, ToolResult};
use secrecy::SecretString;

pub(crate) struct ToolExecutionOutput {
    pub(crate) outcome: Result<ToolResult>,
    pub(crate) seed: Vec<(String, SecretString)>,
    pub(crate) image_policy: ImageOutputPolicy,
}

impl ToolExecutionOutput {
    pub(crate) fn unseeded(result: ToolResult) -> Self {
        Self {
            outcome: Ok(result),
            seed: Vec::new(),
            image_policy: ImageOutputPolicy::Deny,
        }
    }
}
