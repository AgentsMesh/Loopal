use async_trait::async_trait;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::ToolResult;
use crate::permission::PermissionLevel;
use crate::tool::ToolDispatch;
use crate::tool_context::ToolContext;
use loopal_error::LoopalError;

pub trait Params: DeserializeOwned + JsonSchema + Send + 'static {}
impl<T: DeserializeOwned + JsonSchema + Send + 'static> Params for T {}

#[async_trait]
pub trait TypedTool<P: Params>: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn permission(&self) -> PermissionLevel;

    fn dispatch(&self) -> ToolDispatch {
        ToolDispatch::Pipeline
    }

    fn precheck(&self, _input: &P) -> Option<String> {
        None
    }

    async fn execute(&self, input: P, ctx: &ToolContext) -> Result<ToolResult, LoopalError>;
}
