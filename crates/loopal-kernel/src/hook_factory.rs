//! Hook executor factory — creates `HookExecutor` from `HookConfig`.
//!
//! GRASP Creator: Kernel implements this because constructing a
//! `PromptExecutor` requires Provider access that only Kernel has.

use std::sync::Arc;
use std::time::Duration;

use loopal_config::{HookConfig, HookType};
use loopal_hooks::executor::{ExecutorFactory, HookExecutor};
use loopal_hooks::executor_command::CommandExecutor;
use loopal_hooks::executor_http::HttpExecutor;
use loopal_hooks::executor_prompt::PromptExecutor;
use loopal_provider_api::Provider;
use tracing::error;

/// Factory that dispatches to Command, Http, or Prompt executors.
pub struct DefaultExecutorFactory {
    /// Provider for PromptExecutor (resolved at Kernel construction).
    provider: Option<Arc<dyn Provider>>,
}

impl DefaultExecutorFactory {
    pub fn new(provider: Option<Arc<dyn Provider>>) -> Self {
        Self { provider }
    }
}

impl ExecutorFactory for DefaultExecutorFactory {
    fn create(&self, config: &HookConfig) -> Option<Box<dyn HookExecutor>> {
        let timeout = Duration::from_millis(config.timeout_ms);
        match config.hook_type {
            HookType::Command => Some(Box::new(CommandExecutor {
                command: config.command.clone(),
                timeout,
            })),
            HookType::Http => {
                let Some(ref url) = config.url else {
                    error!("Http hook missing required `url` field, skipping");
                    return None;
                };
                if url.is_empty() {
                    error!("Http hook has empty `url` field, skipping");
                    return None;
                }
                Some(Box::new(HttpExecutor {
                    url: url.clone(),
                    headers: config.headers.clone(),
                    timeout,
                }))
            }
            HookType::Prompt => {
                let Some(ref provider) = self.provider else {
                    error!("Prompt hook requires a Provider but none available, skipping");
                    return None;
                };
                Some(Box::new(PromptExecutor {
                    system_prompt: config.prompt.clone().unwrap_or_default(),
                    model: config
                        .model
                        .clone()
                        .unwrap_or_else(|| "claude-haiku-4-5-20251001".into()),
                    provider: provider.clone(),
                    timeout,
                    max_tokens: 256,
                }))
            }
        }
    }
}
