/// MCP client handler implementing rmcp::ClientHandler.
///
/// Handles server-initiated requests (sampling, roots, elicitation)
/// and notifications (progress, logging).
use std::sync::Arc;

use rmcp::handler::client::ClientHandler;
use rmcp::model::{
    ClientCapabilities, ClientInfo, CreateMessageRequestParams, CreateMessageResult, ErrorCode,
    ErrorData, Implementation, InitializeRequestParams, LoggingMessageNotificationParam,
    ProgressNotificationParam, Role, SamplingMessage, SamplingMessageContent,
};
use rmcp::service::{NotificationContext, RequestContext, RoleClient};
use tracing::{debug, warn};

/// Callback for MCP sampling requests (server → LLM).
///
/// Implemented by the runtime layer to call the LLM provider without
/// introducing a provider dependency in the MCP crate.
#[async_trait::async_trait]
pub trait SamplingCallback: Send + Sync {
    /// Complete a conversation using the host's LLM.
    async fn create_message(
        &self,
        system_prompt: Option<&str>,
        messages: Vec<(String, String)>, // (role, text)
        max_tokens: Option<u32>,
    ) -> Result<(String, String), String>; // (model_name, response_text)
}

/// Loopal's implementation of the MCP ClientHandler trait.
pub struct LoopalClientHandler {
    sampling: Option<Arc<dyn SamplingCallback>>,
}

impl LoopalClientHandler {
    pub fn new(sampling: Option<Arc<dyn SamplingCallback>>) -> Self {
        Self { sampling }
    }
}

impl ClientHandler for LoopalClientHandler {
    fn get_info(&self) -> ClientInfo {
        let capabilities = if self.sampling.is_some() {
            ClientCapabilities::builder().enable_sampling().build()
        } else {
            ClientCapabilities::builder().build()
        };
        InitializeRequestParams::new(
            capabilities,
            Implementation::new("loopal", env!("CARGO_PKG_VERSION")),
        )
    }

    async fn create_message(
        &self,
        params: CreateMessageRequestParams,
        _context: RequestContext<RoleClient>,
    ) -> Result<CreateMessageResult, ErrorData> {
        let Some(callback) = &self.sampling else {
            return Err(ErrorData::new(
                ErrorCode::METHOD_NOT_FOUND,
                "sampling not enabled",
                None,
            ));
        };

        // Convert MCP messages to simple (role, text) pairs.
        let messages: Vec<(String, String)> = params
            .messages
            .iter()
            .map(|msg| {
                let role = match msg.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                };
                let text = msg
                    .content
                    .first()
                    .and_then(|c| c.as_text())
                    .map(|t| t.text.to_string())
                    .unwrap_or_default();
                (role.to_string(), text)
            })
            .collect();

        let max_tokens = Some(params.max_tokens);

        match callback
            .create_message(params.system_prompt.as_deref(), messages, max_tokens)
            .await
        {
            Ok((model, text)) => Ok(CreateMessageResult::new(
                SamplingMessage::new(Role::Assistant, SamplingMessageContent::text(&text)),
                model,
            )),
            Err(e) => {
                warn!(error = %e, "sampling callback failed");
                Err(ErrorData::new(ErrorCode::INTERNAL_ERROR, e, None))
            }
        }
    }

    async fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        debug!(
            token = ?params.progress_token,
            progress = params.progress,
            total = params.total,
            "MCP server progress"
        );
    }

    async fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        debug!(
            level = ?params.level,
            logger = ?params.logger,
            "MCP server log: {:?}",
            params.data
        );
    }
}
