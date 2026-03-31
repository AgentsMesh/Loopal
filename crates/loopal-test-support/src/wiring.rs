//! Core channel wiring logic — mirrors `bootstrap.rs:75-186`.
//!
//! When `permission_mode == Bypass`, uses `AutoDenyHandler` (no channel needed).
//! Otherwise, uses `RelayPermissionHandler` with a real permission channel so that
//! `SessionController.approve_permission()` flows through to the agent loop.

use std::sync::Arc;

use tokio::sync::mpsc;

use loopal_agent::shared::AgentShared;
use loopal_agent::task_store::TaskStore;
use loopal_config::Settings;
use loopal_context::ContextStore;
use loopal_kernel::Kernel;
use loopal_protocol::{AgentEvent, ControlCommand, Envelope, UserQuestionResponse};
use loopal_provider_api::Provider;
use loopal_runtime::agent_loop::AgentLoopRunner;
use loopal_runtime::frontend::PermissionHandler;
use loopal_runtime::frontend::{
    AutoCancelQuestionHandler, AutoDenyHandler, RelayPermissionHandler,
};
use loopal_runtime::{AgentLoopParams, UnifiedFrontend};
use loopal_session::SessionController;
use loopal_tool_api::PermissionMode;

use crate::fixture::TestFixture;
use crate::harness::{HarnessBuilder, SpawnedHarness};
use crate::mock_provider::MultiCallProvider;

/// Wire all channels and construct the agent loop.
///
/// Async because `MessageRouter::register()` requires it.
pub(crate) async fn wire(builder: HarnessBuilder) -> (SpawnedHarness, AgentLoopRunner) {
    let fixture = TestFixture::new();

    let (event_tx, event_rx) = mpsc::channel::<AgentEvent>(256);
    let (mailbox_tx, mailbox_rx) = mpsc::channel::<Envelope>(16);
    let (control_tx, control_rx) = mpsc::channel::<ControlCommand>(16);
    let (permission_tx, permission_rx) = mpsc::channel::<bool>(16);
    let (question_tx, _question_rx) = mpsc::channel::<UserQuestionResponse>(16);
    let interrupt = loopal_runtime::InterruptHandle::new();

    // Permission handler: Bypass → auto-deny; Supervised/Default → real channel
    let perm_handler: Box<dyn PermissionHandler> =
        if builder.permission_mode == PermissionMode::Bypass {
            Box::new(AutoDenyHandler)
        } else {
            Box::new(RelayPermissionHandler::new(event_tx.clone(), permission_rx))
        };

    let frontend = Arc::new(UnifiedFrontend::new(
        None,
        event_tx.clone(),
        mailbox_rx,
        control_rx,
        None,
        perm_handler,
        Box::new(AutoCancelQuestionHandler),
    ));

    // Kernel: register builtin tools + agent tools + mock provider
    let settings = Settings {
        hooks: builder.hooks,
        ..Settings::default()
    };
    let mut kernel = Kernel::new(settings).unwrap();
    loopal_agent::tools::register_all(&mut kernel);
    kernel.register_provider(Arc::new(MultiCallProvider::new(builder.calls)) as Arc<dyn Provider>);
    if let Some(setup) = builder.kernel_setup {
        setup(&mut kernel);
    }
    let kernel = Arc::new(kernel);

    let has_cwd_override = builder.cwd.is_some();
    let cwd = builder
        .cwd
        .as_ref()
        .map(|p| p.canonicalize().unwrap_or_else(|_| p.clone()))
        .unwrap_or_else(|| fixture.path().to_path_buf());
    let session_cwd = cwd.clone();

    // Mock hub connection (in-memory duplex — hub side is dropped).
    let (hub_conn, _hub_peer) = loopal_ipc::duplex_pair();
    let hub_connection = Arc::new(loopal_ipc::Connection::new(hub_conn));

    // AgentShared — mirrors bootstrap.rs:103-115
    let tasks_dir = fixture.path().join("tasks");
    let (scheduler_handle, scheduled_rx) = if let Some(sched) = builder.scheduler {
        loopal_agent::shared::SchedulerHandle::create_with_scheduler(sched)
    } else {
        loopal_agent::shared::SchedulerHandle::create()
    };
    let shared = Arc::new(AgentShared {
        kernel: kernel.clone(),
        task_store: Arc::new(TaskStore::new(tasks_dir)),
        hub_connection,
        cwd,
        depth: 0,
        max_depth: 3,
        agent_name: "main".to_string(),
        parent_event_tx: Some(event_tx),
        cancel_token: None,
        scheduler_handle,
    });
    let shared_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(shared);

    let session_ctrl = SessionController::new(
        builder.model.clone(),
        "act".into(),
        control_tx.clone(),
        permission_tx,
        question_tx,
        interrupt.signal.clone(),
        interrupt.tx.clone(),
    );

    let budget = loopal_runtime::build_initial_budget(
        &builder.model,
        200_000, // fixed cap for deterministic test behavior
        &builder.system_prompt,
        0,
    );

    let params = AgentLoopParams {
        config: loopal_runtime::AgentConfig {
            router: {
                let mut routing = std::collections::HashMap::new();
                if let Some(m) = builder.summarization_model {
                    routing.insert(loopal_provider_api::TaskType::Summarization, m);
                }
                loopal_provider_api::ModelRouter::from_parts(builder.model, routing)
            },
            system_prompt: builder.system_prompt,
            mode: builder.mode,
            permission_mode: builder.permission_mode,
            max_turns: builder.max_turns,
            tool_filter: builder.tool_filter,
            thinking_config: builder.thinking_config,
            context_tokens_cap: 200_000,
        },
        deps: loopal_runtime::AgentDeps {
            kernel,
            frontend,
            session_manager: fixture.session_manager(),
        },
        session: if has_cwd_override {
            let mut s = fixture.test_session("integration-test");
            s.cwd = session_cwd.to_string_lossy().into_owned();
            s
        } else {
            fixture.test_session("integration-test")
        },
        store: ContextStore::from_messages(builder.messages, budget),
        interrupt,
        shared: Some(shared_any),
        memory_channel: None,
        scheduled_rx: Some(scheduled_rx),
        auto_classifier: None,
    };

    let harness = SpawnedHarness {
        event_rx,
        mailbox_tx,
        control_tx,
        session_ctrl,
        fixture,
    };
    (harness, AgentLoopRunner::new(params))
}
