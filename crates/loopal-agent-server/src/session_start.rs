use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::{Instrument, info};

use loopal_config::load_config;
use loopal_error::AgentOutput;
use loopal_ipc::connection::Connection;
use loopal_protocol::InterruptSignal;
use loopal_runtime::agent_input::AgentInput;

use crate::agent_setup;
use crate::hub_frontend::HubFrontend;
use crate::session_handlers_factory::build_session_handlers;
use crate::session_hub::{SessionHub, SharedSession};
use crate::session_spawn::{parse_start_params, spawn_agent_and_bridges};

pub(crate) struct SessionHandle {
    pub session_id: String,
    pub session: Arc<SharedSession>,
    pub agent_task: tokio::task::JoinHandle<Option<AgentOutput>>,
    pub lifecycle: loopal_runtime::LifecycleMode,
}

pub(crate) async fn start_session(
    connection: &Arc<Connection>,
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
        let kernel = if is_production {
            crate::params::build_kernel_from_config(
                &config,
                true,
                depth,
                hub_client,
                Some(connection.clone()),
                cwd.clone(),
                agent_name,
            )
            .await?
        } else {
            match hub.get_test_provider().await {
                Some(provider) => crate::params::build_kernel_with_provider(provider)?,
                None => {
                    crate::params::build_kernel_from_config(
                        &config,
                        false,
                        depth,
                        None,
                        None,
                        cwd.clone(),
                        "test".to_string(),
                    )
                    .await?
                }
            }
        };

        let (input_tx, input_rx) = tokio::sync::mpsc::channel::<AgentInput>(16);
        let interrupt = InterruptSignal::new();
        let (watch_tx, watch_rx) = tokio::sync::watch::channel(0u64);
        let interrupt_tx = Arc::new(watch_tx);

        let session_holder: crate::ipc_handlers::SessionRef = Arc::new(tokio::sync::RwLock::new(
            Arc::new(SharedSession::placeholder(
                input_tx.clone(),
                interrupt.clone(),
                interrupt_tx.clone(),
            )),
        ));
        let decision_context =
            loopal_runtime::frontend::DecisionContext::with_cwd(cwd.to_string_lossy().into_owned());
        let (perm_handler, q_handler) = build_session_handlers(
            &config,
            &kernel,
            session_holder.clone(),
            decision_context.clone(),
        );
        let frontend_placeholder = Arc::new(HubFrontend::new(
            session_holder,
            input_rx,
            None,
            watch_rx,
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
        // Tick loop activates after switch_session so the first survey
        // sees the loaded task set, not an empty in-memory state.
        // Disk sync is fire-and-forget — failures retry via dirty flag.
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
        })
    }
    .instrument(session_span)
    .await
}
