use std::sync::Arc;

use tokio::sync::mpsc;

use loopal_agent::shared::AgentShared;
use loopal_agent::task_store::TaskStore;
use loopal_config::Settings;
use loopal_kernel::Kernel;
use loopal_protocol::{AgentEvent, ControlCommand, Envelope, UserQuestionResponse};
use loopal_provider_api::Provider;
use loopal_runtime::UnifiedFrontend;
use loopal_runtime::agent_loop::AgentLoopRunner;
use loopal_runtime::frontend::PermissionHandler;
use loopal_runtime::frontend::{DenyAllHandler, ManualPermissionHandler};
use loopal_session::SessionController;
use loopal_tool_api::PermissionMode;

use crate::fixture::TestFixture;
use crate::harness::{HarnessBuilder, SpawnedHarness};
use crate::mock_provider::MultiCallProvider;

pub(crate) async fn wire(builder: HarnessBuilder) -> (SpawnedHarness, AgentLoopRunner) {
    let fixture = TestFixture::new();
    let seed_messages = builder.messages.clone();

    let (event_tx, event_rx) = mpsc::channel::<AgentEvent>(256);
    let (mailbox_tx, mailbox_rx) = mpsc::channel::<Envelope>(16);
    let (control_tx, control_rx) = mpsc::channel::<ControlCommand>(16);
    let (permission_tx, permission_rx) = mpsc::channel::<bool>(16);
    let (question_tx, question_rx) = mpsc::channel::<UserQuestionResponse>(16);
    let interrupt = loopal_runtime::InterruptHandle::new();
    let interrupt_signal_for_test = interrupt.signal.clone();

    // Bypass policy never returns Ask → DenyAllHandler is a no-op that flags
    // misconfig if it's ever called. Other modes need a real channel.
    let perm_handler: Box<dyn PermissionHandler> =
        if builder.permission_mode == PermissionMode::Bypass {
            Box::new(DenyAllHandler)
        } else {
            Box::new(ManualPermissionHandler::new(
                event_tx.clone(),
                permission_rx,
            ))
        };

    let frontend = Arc::new(UnifiedFrontend::new(
        None,
        event_tx.clone(),
        mailbox_rx,
        control_rx,
        None,
        perm_handler,
        Box::new(loopal_runtime::frontend::ManualQuestionHandler::new(
            event_tx.clone(),
            question_rx,
        )),
    ));

    // Goal tools are root-only — only register when test wired a goal_session.
    let settings = Settings {
        hooks: builder.hooks,
        ..Settings::default()
    };
    let mut kernel = Kernel::new(settings).unwrap();
    if builder.goal_session.is_some() {
        kernel.register_goal_tools();
    }
    loopal_agent::tools::register_all(&mut kernel);
    let mock_provider = MultiCallProvider::new(builder.calls);
    let mock_provider = if let Some(d) = builder.llm_chunk_delay {
        mock_provider.with_delay(d)
    } else {
        mock_provider
    };
    let recorded_messages = mock_provider.messages_handle();
    kernel.register_provider(Arc::new(mock_provider) as Arc<dyn Provider>);
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

    // Mock hub connection (in-memory duplex; hub side dropped).
    let (hub_conn, _hub_peer) = loopal_ipc::duplex_pair();
    let (hub_connection, _hub_rx) = loopal_ipc::Connection::new(hub_conn).into_listening();

    // AgentShared — mirrors bootstrap.rs:103-115
    let tasks_dir = fixture.path().join("tasks");
    let scheduler_for_params = builder
        .scheduler
        .clone()
        .unwrap_or_else(|| Arc::new(loopal_scheduler::CronScheduler::new()));
    let (scheduler_handle, scheduled_rx) =
        loopal_agent::shared::SchedulerHandle::create_with_scheduler(scheduler_for_params.clone());
    // Test fixtures don't switch sessions; activate the tick loop now.
    scheduler_handle.start();
    let shared = Arc::new(AgentShared {
        kernel: kernel.clone(),
        task_store: Arc::new(TaskStore::with_sessions_root(tasks_dir)),
        hub_connection,
        cwd,
        depth: 0,
        agent_name: "main".to_string(),
        parent_event_tx: Some(event_tx.clone()),
        cancel_token: None,
        scheduler_handle,
        message_snapshot: Arc::new(std::sync::RwLock::new(Vec::new())),
        goal_session: builder.goal_session.clone(),
    });
    let shared_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(shared);

    let session_ctrl = SessionController::new(
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

    let params = loopal_runtime::AgentLoopParamsBuilder::new(
        loopal_runtime::AgentConfig {
            lifecycle: builder.lifecycle,
            router: {
                let mut routing = std::collections::HashMap::new();
                if let Some(m) = builder.summarization_model {
                    routing.insert(loopal_provider_api::TaskType::Summarization, m);
                }
                loopal_provider_api::SharedModelRouter::from_parts(builder.model, routing)
            },
            system_prompt: builder.system_prompt,
            mode: builder.mode,
            permission_mode: builder.permission_mode,
            tool_filter: builder.tool_filter,
            thinking_config: builder.thinking_config,
            context_tokens_cap: 200_000,
            microcompact_idle: std::time::Duration::from_secs(60 * 60),
            plan_state: None,
        },
        loopal_runtime::AgentDeps {
            kernel,
            frontend,
            session_manager: fixture.session_manager(),
            decision_context: loopal_runtime::frontend::DecisionContext::with_cwd("/tmp/test"),
        },
        if has_cwd_override {
            let mut s = fixture.unique_test_session("integration-test");
            s.cwd = session_cwd.to_string_lossy().into_owned();
            s
        } else {
            fixture.unique_test_session("integration-test")
        },
        budget,
        interrupt,
    )
    .shared(shared_any)
    .scheduled_rx(scheduled_rx)
    .scheduler(scheduler_for_params)
    .goal_session_opt(builder.goal_session)
    .outstanding_tasks_opt(builder.outstanding_tasks)
    .build();

    let harness = SpawnedHarness {
        event_tx: event_tx.clone(),
        event_rx,
        mailbox_tx,
        control_tx,
        interrupt: interrupt_signal_for_test,
        session_ctrl,
        fixture,
        recorded_messages,
    };
    let mut runner = AgentLoopRunner::new(params);
    let harness_cfg = loopal_config::HarnessConfig::default();
    runner.governance = loopal_runtime::agent_loop::governance::build_governance(&harness_cfg);
    if !seed_messages.is_empty() {
        let turns = crate::seed_history::reverse_project_messages_to_turns(seed_messages);
        runner.seed_test_turns(turns);
    }
    // Don't auto-open a Resume turn for empty seed — run_loop handles empty
    // store naturally (needs_input=true → idle phase). Pre-opening conflicted
    // with F7 (try_start_turn refuses to overwrite an in-progress turn): user
    // message ingestion saw Resume still open and failed to start UserInput.
    (harness, runner)
}
