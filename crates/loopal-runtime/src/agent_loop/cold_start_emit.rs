use loopal_error::Result;
use loopal_protocol::AgentEventPayload;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    pub(super) async fn emit_cold_start_observables(&self) -> Result<()> {
        self.emit(AgentEventPayload::ModelChanged {
            model: self.params.config.model().to_string(),
        })
        .await?;
        self.emit(AgentEventPayload::ModeChanged {
            mode: self.params.config.mode.as_str().to_string(),
        })
        .await?;
        let thinking_json = serde_json::to_string(&self.model_config.thinking)
            .unwrap_or_else(|_| r#"{"type":"auto"}"#.to_string());
        self.emit(AgentEventPayload::ThinkingChanged {
            thinking_config: thinking_json,
        })
        .await
    }
}
