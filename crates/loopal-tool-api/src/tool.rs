use async_trait::async_trait;
use loopal_tool_invocation::{ToolImageBlock, ToolResultMetadata};
use serde::{Deserialize, Serialize};

use loopal_error::LoopalError;

use crate::backend_types::ImageResult;
use crate::permission::PermissionLevel;
use crate::tool_context::ToolContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolDispatch {
    Pipeline,
    RunnerDirect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageOutputPolicy {
    #[default]
    Deny,
    ValidatedInline,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> serde_json::Value;
    fn permission(&self) -> PermissionLevel;

    fn dispatch(&self) -> ToolDispatch {
        ToolDispatch::Pipeline
    }

    fn image_output_policy(&self) -> ImageOutputPolicy {
        ImageOutputPolicy::Deny
    }

    fn precheck(&self, _input: &serde_json::Value) -> Option<String> {
        None
    }

    /// JSON parameter names whose string values may contain `<secret_ref:NAME>`
    /// placeholders that the runtime should substitute with plaintext before
    /// `execute()` is called.
    ///
    /// **No default**: every Tool MUST declare this explicitly so adding a new
    /// tool forces an explicit choice about secret exposure.
    ///
    /// - Read-only / file-writing tools (Read/Glob/Grep/Write/Edit/...) → `&[]`
    /// - Shell/network tools that legitimately consume secrets (Bash) →
    ///   list of field names, e.g. `&["command", "env"]`
    /// - MCP adapter / sub-agent tools → `&[]` (secrets flow via env at spawn)
    fn secret_eligible_params(&self) -> &'static [&'static str];

    async fn execute(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> std::result::Result<ToolResult, LoopalError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<ToolImageBlock>,
    pub is_error: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ToolResultMetadata>,
}

impl ToolResult {
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            images: Vec::new(),
            is_error: false,
            metadata: None,
        }
    }

    pub fn error(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            images: Vec::new(),
            is_error: true,
            metadata: None,
        }
    }

    pub fn with_metadata(mut self, metadata: ToolResultMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn with_image(mut self, img: ImageResult) -> Self {
        self.images.push(ToolImageBlock::Inline {
            media_type: img.media_type,
            data: img.data,
        });
        self
    }

    pub fn with_images(mut self, imgs: Vec<ToolImageBlock>) -> Self {
        self.images.extend(imgs);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}
