//! Internal helpers split out from `agent_setup` to keep the main file
//! small and each helper single-purpose.

use std::sync::Arc;
use std::time::Duration;

use loopal_config::{CompactionSettings, ResolvedConfig, Settings};
use loopal_protocol::{AgentEvent, AgentEventPayload};
use loopal_provider_api::{ContentBlock, Message, MessageRole, ModelRouter, ThinkingConfig};
use loopal_runtime::frontend::traits::AgentFrontend;
use loopal_turn::{InjectionKind, Turn, TurnStep, TurnTrigger};

use crate::params::StartParams;

/// No hardcoded per-task default: unconfigured tasks fall back to the main
/// model via `ModelRouter::resolve` — a pinned default could reference an
/// unconfigured provider and silently break compaction.
pub fn build_model_router(settings: &Settings) -> ModelRouter {
    ModelRouter::from_parts(settings.model.clone(), settings.model_routing.clone())
}

/// Resolve the microcompact idle duration. Logs a hint when the user has
/// disabled the feature (idle=0) so it's easy to debug "why didn't
/// microcompact fire".
pub fn build_microcompact_idle(settings: &CompactionSettings) -> Duration {
    if settings.microcompact_idle_minutes == 0 {
        tracing::info!("microcompact disabled (microcompact_idle_minutes=0)");
        return Duration::ZERO;
    }
    Duration::from_secs(settings.microcompact_idle_minutes * 60)
}

pub fn thinking_inputs(config: &ResolvedConfig) -> (ThinkingConfig, Option<ThinkingConfig>) {
    (
        config.settings.thinking.clone(),
        config.workflow_preset_thinking_recommendation.clone(),
    )
}

/// Spawn the sub-agent lifecycle forwarder. Listens for `SubAgentSpawned`
/// events on `event_rx` and forwards them to the root frontend so the TUI
/// can auto-attach to newly spawned child agents.
///
/// Returns the sender side of the channel that `AgentShared` should use
/// as `parent_event_tx` for the root agent.
pub fn spawn_sub_agent_forwarder(
    frontend: Arc<dyn AgentFrontend>,
) -> tokio::sync::mpsc::Sender<AgentEvent> {
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<AgentEvent>(256);
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            if matches!(event.payload, AgentEventPayload::SubAgentSpawned(_)) {
                frontend
                    .emit_best_effort(event.payload, "agent_setup_helpers::sub_agent_spawned")
                    .await;
            }
        }
    });
    event_tx
}

/// Convert a parent agent's wire-format fork context (`Vec<Message>`) into
/// a synthetic `Turn` carrying an `Injection { SystemNote }` step. This
/// lands in the sub-agent's `TurnStore` so the LLM sees the parent's
/// history on its first call (wire-build reads from `TurnStore`, never
/// from the projected `ContextStore` view).
///
/// Returns an in-progress kickoff turn. The setup path persists it before
/// starting the runtime so the first LLM call and crash recovery share one
/// durable turn. Returns `None` when fork is absent / empty / unparseable, OR
/// when the session is resuming (resume takes precedence over fork).
pub fn build_fork_synthetic_turn(start: &StartParams) -> Option<Turn> {
    if start.resume.is_some() {
        return None;
    }
    let fork_value = start.fork_context.as_ref()?;
    let messages: Vec<Message> = serde_json::from_value(fork_value.clone())
        .map_err(|e| tracing::warn!("fork context deserialization failed, skipping: {e}"))
        .ok()?;
    if messages.is_empty() {
        return None;
    }
    let mut text = render_fork_as_text(&messages);
    if let Some(prompt) = start.prompt.as_deref() {
        text.push_str(loopal_context::fork::FORK_BOILERPLATE);
        text.push_str(prompt);
    }
    let mut turn = Turn::new(TurnTrigger::Resume);
    turn.body.steps.push(TurnStep::Injection {
        kind: InjectionKind::SystemNote,
        text,
    });
    Some(turn)
}

fn render_fork_as_text(messages: &[Message]) -> String {
    let mut buf = String::from("[Parent agent conversation context]\n\n");
    for msg in messages {
        let role_label = match msg.role {
            MessageRole::User => "USER",
            MessageRole::Assistant => "ASSISTANT",
            MessageRole::System => "SYSTEM",
        };
        buf.push_str(role_label);
        buf.push_str(": ");
        for block in &msg.content {
            match block {
                ContentBlock::Text { text } => buf.push_str(text),
                ContentBlock::ToolUse { name, input, .. } => {
                    buf.push_str(&format!("[calls {name}({input})]"));
                }
                ContentBlock::ToolResult {
                    content, is_error, ..
                } => {
                    let prefix = if *is_error {
                        "tool error"
                    } else {
                        "tool result"
                    };
                    buf.push_str(&format!("[{prefix}: {content}]"));
                }
                _ => {}
            }
        }
        buf.push('\n');
    }
    buf
}

/// Collect runtime feature tags for the system prompt preamble.
pub fn collect_feature_tags(config: &ResolvedConfig, has_memory_channel: bool) -> Vec<String> {
    let mut features = Vec::new();
    if config.settings.memory.enabled && has_memory_channel {
        features.push("memory".into());
    }
    if !config.settings.hooks.is_empty() {
        features.push("hooks".into());
    }
    features.push("subagent".into());
    if !config.settings.output_style.is_empty() {
        features.push(format!("style_{}", config.settings.output_style));
    }
    if config.secrets.is_some() {
        features.push("secrets".into());
    }
    features
}

#[cfg(test)]
mod thinking_input_tests {
    use std::sync::Arc;

    use loopal_config::ConfigResolver;
    use loopal_provider_api::EffortLevel;
    use loopal_vault_api::{SecretString, Vault, VaultResult};

    use super::*;

    struct EmptyVault;

    #[async_trait::async_trait]
    impl Vault for EmptyVault {
        async fn get(&self, _name: &str) -> Option<SecretString> {
            None
        }

        async fn list_names(&self) -> Vec<String> {
            Vec::new()
        }

        async fn put(&self, _name: &str, _value: SecretString) -> VaultResult<()> {
            Ok(())
        }

        async fn delete(&self, _name: &str) -> VaultResult<()> {
            Ok(())
        }

        async fn rekey(&self) -> VaultResult<()> {
            Ok(())
        }
    }

    #[test]
    fn keeps_explicit_setting_and_recommendation_distinct() {
        let mut config = ConfigResolver::new().resolve().unwrap();
        config.settings.thinking = ThinkingConfig::Disabled;
        config.workflow_preset_thinking_recommendation = Some(ThinkingConfig::Effort {
            level: EffortLevel::Max,
        });

        let (configured, recommendation) = thinking_inputs(&config);

        assert!(matches!(configured, ThinkingConfig::Disabled));
        assert!(matches!(
            recommendation,
            Some(ThinkingConfig::Effort {
                level: EffortLevel::Max
            })
        ));
    }

    #[test]
    fn default_thinking_has_no_preset_recommendation() {
        let config = ConfigResolver::new().resolve().unwrap();
        let (configured, recommendation) = thinking_inputs(&config);

        assert!(matches!(configured, ThinkingConfig::Auto));
        assert!(recommendation.is_none());
    }

    #[test]
    fn microcompact_minutes_map_to_duration_and_zero_disables() {
        let mut settings = CompactionSettings {
            microcompact_idle_minutes: 0,
        };
        assert_eq!(build_microcompact_idle(&settings), Duration::ZERO);

        settings.microcompact_idle_minutes = 7;
        assert_eq!(build_microcompact_idle(&settings), Duration::from_secs(420));
    }

    #[test]
    fn feature_tags_respect_disabled_memory_and_present_vault() {
        let mut config = ConfigResolver::new().resolve().unwrap();
        config.settings.memory.enabled = false;
        config.secrets = Some(Arc::new(EmptyVault));

        let features = collect_feature_tags(&config, true);

        assert!(!features.iter().any(|feature| feature == "memory"));
        assert!(features.iter().any(|feature| feature == "secrets"));
    }
}
