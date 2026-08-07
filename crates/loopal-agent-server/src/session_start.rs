use std::sync::Arc;

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, info};

use loopal_config::load_config;
use loopal_error::AgentOutput;
use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::InterruptSignal;
use loopal_runtime::agent_input::AgentInput;

use crate::agent_setup;
use crate::agent_setup_helpers::build_model_router;
use crate::hub_broadcaster::HubBroadcaster;
use crate::hub_frontend::HubFrontend;
use crate::session_handlers_factory::build_session_handlers_with_emitter;
use crate::session_hub::{SessionHub, SharedSession};
use crate::session_spawn::{parse_start_params, spawn_agent_and_bridges};
use crate::session_start_prompt::push_start_prompt;
use loopal_provider_api::SharedModelRouter;

pub(crate) struct SessionHandle {
    pub session_id: String,
    pub session: Arc<SharedSession>,
    pub agent_task: tokio::task::JoinHandle<Option<AgentOutput>>,
    pub lifecycle: loopal_runtime::LifecycleMode,
    /// Level-triggered session termination, distinct from per-turn interrupt.
    pub shutdown: CancellationToken,
}

pub(crate) async fn start_session(
    connection: &Arc<Connection<Listening>>,
    request_id: i64,
    params: serde_json::Value,
    hub: &SessionHub,
    is_production: bool,
) -> anyhow::Result<SessionHandle> {
    let session_span = tracing::info_span!("session_start", session.id = tracing::field::Empty);
    async {
        let (start, cwd, lifecycle) = parse_start_params(&params)?;

        let mut config = load_config(&cwd)?;
        crate::params::apply_start_overrides(&mut config.settings, &start);
        let depth = start.depth.unwrap_or(0);
        let hub_client: Option<Arc<dyn loopal_mcp::HubMcpClient>> = Some(Arc::new(
            crate::connection_mcp_client::ConnectionMcpClient::new(connection.clone()),
        ));
        let agent_name = if depth == 0 {
            loopal_protocol::ROOT_AGENT_NAME.to_string()
        } else {
            "sub".to_string()
        };
        let preset_session_id = start
            .resume
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let kernel = if is_production {
            crate::params::build_kernel_from_config(
                &config,
                true,
                depth,
                hub_client,
                Some(connection.clone()),
                cwd.clone(),
                agent_name,
                preset_session_id.clone(),
            )
            .await?
        } else {
            match hub.get_test_provider().await {
                Some(provider) => {
                    crate::params::build_kernel_with_provider(provider, start.model.as_deref())?
                }
                None => {
                    crate::params::build_kernel_from_config(
                        &config,
                        false,
                        depth,
                        None,
                        None,
                        cwd.clone(),
                        "test".to_string(),
                        preset_session_id.clone(),
                    )
                    .await?
                }
            }
        };

        let (input_tx, input_rx) = tokio::sync::mpsc::channel::<AgentInput>(16);
        let interrupt = InterruptSignal::new();
        let (watch_tx, watch_rx) = tokio::sync::watch::channel(0u64);
        let interrupt_tx = Arc::new(watch_tx);
        let shutdown = CancellationToken::new();

        let session_holder: crate::ipc_handlers::SessionRef = Arc::new(tokio::sync::RwLock::new(
            Arc::new(SharedSession::placeholder(
                input_tx.clone(),
                interrupt.clone(),
                interrupt_tx.clone(),
            )),
        ));
        let decision_context =
            loopal_runtime::frontend::DecisionContext::with_cwd(cwd.to_string_lossy().into_owned());
        // The kernel owns the effective settings paired with its provider registry.
        // This differs from `config` for injected test providers and may also differ
        // after secret expansion, so deriving a second route from `config` can select
        // a provider that the kernel does not contain.
        let model_router = SharedModelRouter::new(build_model_router(kernel.settings()));
        // Runtime events and classifier progress share one FIFO delivery worker.
        // This prevents two emitters targeting the same client transport from
        // racing each other at the connection write lock.
        let broadcaster = HubBroadcaster::new(session_holder.clone(), None);
        let (perm_handler, q_handler, decision_cell) = build_session_handlers_with_emitter(
            &config,
            &kernel,
            session_holder.clone(),
            decision_context.clone(),
            model_router.reader(),
            Arc::new(broadcaster.clone()),
        );
        let frontend_placeholder = Arc::new(HubFrontend::new_with_broadcaster(
            session_holder,
            broadcaster,
            input_rx,
            watch_rx,
            shutdown.clone(),
            perm_handler,
            q_handler,
        ));

        let session_dir_override = hub.session_dir_override().await;
        let kernel_for_bridge = kernel.clone();
        let setup =
            agent_setup::build_with_frontend(crate::agent_setup_context::AgentSetupContext::new(
                &cwd,
                &config,
                &start,
                frontend_placeholder.clone(),
                interrupt.clone(),
                interrupt_tx.clone(),
                kernel,
                connection.clone(),
                session_dir_override.as_deref(),
                hub,
                decision_context,
                decision_cell,
                &preset_session_id,
                model_router,
            ))
            .await?;
        let agent_params = setup.params;
        let task_store_for_bridge = setup.task_store;
        let scheduler_for_bridge = setup.scheduler;
        let agent_shared_for_session = setup.agent_shared;

        if let Err(e) = scheduler_for_bridge
            .switch_session(&agent_params.session().id)
            .await
        {
            tracing::warn!(error = %e, "failed to bind scheduler to session");
        }
        // Tick loop activates after switch_session so the first survey sees
        // the loaded task set, not empty in-memory state.
        agent_shared_for_session.scheduler_handle.start();

        let session_id = agent_params.session().id.clone();
        tracing::Span::current().record("session.id", session_id.as_str());

        let session = Arc::new(SharedSession {
            session_id: session_id.clone(),
            clients: Mutex::new(Vec::new()),
            input_tx,
            interrupt: interrupt.clone(),
            interrupt_tx: interrupt_tx.clone(),
            agent_shared: Mutex::new(None),
        });
        session.add_client("stdio".into(), connection.clone()).await;
        session.set_agent_shared(&agent_shared_for_session).await;
        frontend_placeholder.replace_session(session.clone()).await;
        hub.register_session(session.clone()).await;

        let _ = connection
            .respond(request_id, serde_json::json!({"session_id": session_id}))
            .await;
        info!(session.id = %session_id, "session started");

        let spawn_rx = kernel_for_bridge.bg_store().subscribe_spawns();
        let bg_store_for_bridge = kernel_for_bridge.bg_store().clone();

        // Enqueue --prompt BEFORE spawning agent task — pushing after races
        // the ephemeral lifecycle's drain_pending_input, which exits when
        // the queue is empty.
        push_start_prompt(&session, &start).await;

        let agent_task = spawn_agent_and_bridges(
            agent_params,
            task_store_for_bridge,
            scheduler_for_bridge,
            spawn_rx,
            bg_store_for_bridge,
            frontend_placeholder,
        );

        Ok(SessionHandle {
            session_id,
            session,
            agent_task,
            lifecycle,
            shutdown,
        })
    }
    .instrument(session_span)
    .await
}
