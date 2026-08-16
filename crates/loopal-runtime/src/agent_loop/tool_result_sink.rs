use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::ContentBlock;
use loopal_tool_api::{ImageOutputPolicy, ToolResult};
use loopal_tool_invocation::ToolResultMetadata;
use secrecy::SecretString;

use super::runner::AgentLoopRunner;

pub(super) struct PendingToolResult {
    id: String,
    name: String,
    result: ToolResult,
    duration_ms: Option<u64>,
    seed: Vec<(String, SecretString)>,
    image_policy: ImageOutputPolicy,
    context: loopal_protocol::event_id::TurnContext,
}

pub(super) struct FinalToolResult {
    pub(super) event: AgentEventPayload,
    pub(super) block: ContentBlock,
    pub(super) context: loopal_protocol::event_id::TurnContext,
}

impl PendingToolResult {
    pub(super) fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        content: impl Into<String>,
        is_error: bool,
        metadata: Option<ToolResultMetadata>,
    ) -> Self {
        Self::from_result(
            id,
            name,
            ToolResult {
                content: content.into(),
                images: Vec::new(),
                is_error,
                metadata,
            },
        )
    }

    pub(super) fn from_result(
        id: impl Into<String>,
        name: impl Into<String>,
        result: ToolResult,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            result,
            duration_ms: None,
            seed: Vec::new(),
            image_policy: ImageOutputPolicy::Deny,
            context: loopal_protocol::event_id::TurnContext::current_or_default(),
        }
    }

    pub(super) fn with_duration(mut self, duration: std::time::Duration) -> Self {
        self.duration_ms = Some(duration.as_millis() as u64);
        self
    }

    pub(super) fn with_guard(
        mut self,
        seed: Vec<(String, SecretString)>,
        image_policy: ImageOutputPolicy,
    ) -> Self {
        self.seed = seed;
        self.image_policy = image_policy;
        self
    }

    pub(super) fn append_content(&mut self, suffix: &str) {
        self.result.content.push_str(suffix);
    }

    pub(super) fn id(&self) -> &str {
        &self.id
    }

    pub(super) fn is_error(&self) -> bool {
        self.result.is_error
    }

    pub(super) async fn finalize(mut self, runner: &AgentLoopRunner) -> Result<FinalToolResult> {
        self.result = match crate::tool_result_guard::finalize(
            &self.name,
            self.result,
            &self.seed,
            &runner.tool_ctx.session_id,
            self.image_policy,
            runner.params.deps.kernel.settings().images.max_bytes,
        ) {
            Ok(result) => result,
            Err(error) => ToolResult::error(error.to_string()),
        };
        let mut images = std::mem::take(&mut self.result.images);
        if !images.is_empty()
            && let Some(store) = runner
                .params
                .resource_store
                .clone()
                .or_else(crate::hydrate::resource_store)
            && crate::hydrate::maybe_persist_inline_images(
                store.as_ref(),
                &runner.tool_ctx.session_id,
                &mut images,
                runner
                    .params
                    .deps
                    .kernel
                    .settings()
                    .images
                    .inline_threshold_bytes,
                runner.params.deps.kernel.settings().images.max_bytes,
            )
            .await
            .is_err()
        {
            self.result =
                ToolResult::error("tool result rejected: image persistence validation failed");
            images.clear();
        }
        Ok(self.into_final(images))
    }

    fn into_final(self, images: Vec<loopal_tool_invocation::ToolImageBlock>) -> FinalToolResult {
        let event = AgentEventPayload::ToolResult {
            id: self.id.clone(),
            name: self.name,
            result: self.result.content.clone(),
            is_error: self.result.is_error,
            duration_ms: self.duration_ms,
            metadata: self.result.metadata.clone(),
        };
        let block = ContentBlock::ToolResult {
            tool_use_id: self.id,
            content: self.result.content,
            images,
            is_error: self.result.is_error,
            metadata: self.result.metadata,
        };
        FinalToolResult {
            event,
            block,
            context: self.context,
        }
    }
}
