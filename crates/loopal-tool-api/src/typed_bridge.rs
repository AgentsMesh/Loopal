use std::marker::PhantomData;
use std::sync::OnceLock;

use async_trait::async_trait;
use schemars::schema_for;
use serde_json::Value;

use crate::ToolResult;
use crate::input_normalize::strip_empty_optionals;
use crate::permission::PermissionLevel;
use crate::schema_normalize::normalize_schema;
use crate::tool::{ImageOutputPolicy, Tool, ToolDispatch};
use crate::tool_context::ToolContext;
use crate::typed_tool::{Params, TypedTool};
use loopal_error::LoopalError;

pub struct TypedBridge<T, P: Params> {
    inner: T,
    schema: OnceLock<Value>,
    _phantom: PhantomData<fn() -> P>,
}

impl<T, P: Params> TypedBridge<T, P> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            schema: OnceLock::new(),
            _phantom: PhantomData,
        }
    }

    fn cached_schema(&self) -> &Value {
        self.schema.get_or_init(|| {
            let raw = serde_json::to_value(schema_for!(P)).unwrap_or_default();
            normalize_schema(raw)
        })
    }

    fn required_fields(&self) -> Vec<&str> {
        self.cached_schema()["required"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default()
    }
}

#[async_trait]
impl<T: TypedTool<P>, P: Params> Tool for TypedBridge<T, P> {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn parameters_schema(&self) -> Value {
        self.cached_schema().clone()
    }

    fn permission(&self) -> PermissionLevel {
        self.inner.permission()
    }

    fn dispatch(&self) -> ToolDispatch {
        self.inner.dispatch()
    }

    fn image_output_policy(&self) -> ImageOutputPolicy {
        self.inner.image_output_policy()
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        self.inner.secret_eligible_params()
    }

    fn precheck(&self, input: &Value) -> Option<String> {
        let mut normalized = input.clone();
        strip_empty_optionals(&mut normalized, &self.required_fields());
        let parsed: P = serde_json::from_value(normalized).ok()?;
        self.inner.precheck(&parsed)
    }

    async fn execute(
        &self,
        mut input: Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        strip_empty_optionals(&mut input, &self.required_fields());
        let parsed: P = serde_json::from_value(input)
            .map_err(|e| LoopalError::Tool(loopal_error::ToolError::InvalidInput(e.to_string())))?;
        self.inner.execute(parsed, ctx).await
    }
}
