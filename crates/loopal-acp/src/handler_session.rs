//! Session bootstrap and prompt handling — split from `handler.rs` for SRP.

use std::sync::Arc;

use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use loopal_agent::registry::AgentRegistry;
use loopal_agent::router::MessageRouter;
use loopal_agent::shared::AgentShared;
use loopal_agent::task_store::TaskStore;
use loopal_context::system_prompt::build_system_prompt;
use loopal_context::{ContextBudget, ContextPipeline, ContextStore};
use loopal_kernel::Kernel;
use loopal_runtime::{AgentLoopParams, AgentMode, SessionManager};

use crate::frontend::AcpFrontend;
use crate::handler::{AcpHandler, ActiveSession};
use crate::types::*;

impl AcpHandler {
    pub(crate) async fn handle_new_session(&self, id: i64, params: Value) {
        let params: NewSessionParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => {
                self.transport
                    .respond_error(id, crate::jsonrpc::INVALID_REQUEST, &e.to_string())
                    .await;
                return;
            }
        };

        let cwd = if params.cwd.is_absolute() {
            params.cwd
        } else {
            self.cwd.clone()
        };

        // Bootstrap kernel using resolved config
        let mut kernel = match Kernel::new(self.config.settings.clone()) {
            Ok(k) => k,
            Err(e) => {
                self.transport
                    .respond_error(id, crate::jsonrpc::INTERNAL_ERROR, &e.to_string())
                    .await;
                return;
            }
        };
        if let Err(e) = kernel.start_mcp().await {
            error!(error = %e, "MCP start failed");
        }
        loopal_agent::tools::register_all(&mut kernel);
        let kernel = Arc::new(kernel);

        // Session
        let session_manager = match SessionManager::new() {
            Ok(sm) => sm,
            Err(e) => {
                self.transport
                    .respond_error(id, crate::jsonrpc::INTERNAL_ERROR, &e.to_string())
                    .await;
                return;
            }
        };
        let model = &self.config.settings.model;
        let session = match session_manager.create_session(&cwd, model) {
            Ok(s) => s,
            Err(e) => {
                self.transport
                    .respond_error(id, crate::jsonrpc::INTERNAL_ERROR, &e.to_string())
                    .await;
                return;
            }
        };
        let session_id = session.id.clone();

        // Channels
        let cancel_token = CancellationToken::new();
        let (event_tx, event_rx) = mpsc::channel(256);
        let (input_tx, input_rx) = mpsc::channel(16);

        let frontend = Arc::new(AcpFrontend::new(
            None,
            event_tx.clone(),
            input_rx,
            self.transport.clone(),
            session_id.clone(),
            cancel_token.clone(),
        ));

        // Build system prompt + context pipeline from resolved config
        let (system_prompt, context_pipeline, shared) =
            self.build_agent_context(&kernel, &session_id, &cwd, event_tx.clone());

        let budget = ContextBudget::calculate(200_000, &system_prompt, 0, 16_384);

        let agent_params = AgentLoopParams {
            kernel: kernel.clone(),
            session,
            store: ContextStore::from_messages(Vec::new(), budget),
            model: model.clone(),
            compact_model: self.config.settings.compact_model.clone(),
            system_prompt,
            mode: AgentMode::Act,
            permission_mode: self.config.settings.permission_mode,
            max_turns: self.config.settings.max_turns,
            frontend,
            session_manager,
            context_pipeline,
            tool_filter: None,
            shared: Some(shared),
            interactive: true,
            thinking_config: self.config.settings.thinking.clone(),
            interrupt: loopal_protocol::InterruptSignal::new(),
            interrupt_tx: std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
            memory_channel: None, // ACP does not yet support memory observer
        };

        tokio::spawn(async move {
            if let Err(e) = loopal_runtime::agent_loop(agent_params).await {
                error!(error = %e, "ACP agent loop error");
            }
        });

        // Store session
        *self.session.lock().await = Some(ActiveSession {
            id: session_id.clone(),
            input_tx,
            event_rx: tokio::sync::Mutex::new(event_rx),
            cancel_token,
        });

        let result = NewSessionResult { session_id };
        let value = serde_json::to_value(result).unwrap_or_default();
        self.transport.respond(id, value).await;
        info!("ACP session created");
    }

    fn build_agent_context(
        &self,
        kernel: &Arc<Kernel>,
        session_id: &str,
        cwd: &std::path::Path,
        event_tx: mpsc::Sender<loopal_protocol::AgentEvent>,
    ) -> (
        String,
        ContextPipeline,
        Arc<dyn std::any::Any + Send + Sync>,
    ) {
        let skills: Vec<_> = self
            .config
            .skills
            .values()
            .map(|e| e.skill.clone())
            .collect();
        let skills_summary = loopal_config::format_skills_summary(&skills);
        let tool_defs = kernel.tool_definitions();
        let system_prompt = build_system_prompt(
            &self.config.instructions,
            &tool_defs,
            "act",
            &cwd.to_string_lossy(),
            &skills_summary,
            &self.config.memory,
        );

        let pipeline = ContextPipeline::new();

        let router = Arc::new(MessageRouter::new(event_tx.clone()));
        let tasks_dir = loopal_config::session_tasks_dir(session_id)
            .unwrap_or_else(|_| std::env::temp_dir().join("loopal/tasks"));

        let shared = Arc::new(AgentShared {
            kernel: kernel.clone(),
            registry: Arc::new(tokio::sync::Mutex::new(AgentRegistry::new())),
            task_store: Arc::new(TaskStore::new(tasks_dir)),
            router,
            cwd: cwd.to_path_buf(),
            depth: 0,
            max_depth: 3,
            agent_name: "main".to_string(),
            parent_event_tx: Some(event_tx),
            cancel_token: None,
            worktree_state: Default::default(),
        });
        let shared_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(shared);

        (system_prompt, pipeline, shared_any)
    }
}
