use std::sync::Arc;

use tracing::{Instrument, info};

use loopal_config::load_config;
use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::InterruptSignal;
use loopal_provider_api::SharedModelRouter;
use loopal_runtime::agent_input::AgentInput;
use loopal_scheduler::CronScheduler;

use crate::agent_setup;
use crate::agent_setup_helpers::build_model_router;
use crate::hub_broadcaster::HubBroadcaster;
use crate::hub_frontend::HubFrontend;
use crate::session_handlers_factory::build_session_handlers_with_emitter;
use crate::session_hub::{SessionHub, SharedSession};
use crate::session_spawn::{parse_start_params, spawn_agent_and_bridges};
use crate::session_start_prompt::push_start_prompt;

mod kernel;
mod lifecycle;
mod types;
mod workflow_handshake;
pub(crate) use types::SessionHandle;
pub(crate) async fn start_session(
    connection: &Arc<Connection<Listening>>,
    request_id: i64,
    params: serde_json::Value,
    hub: &SessionHub,
    is_production: bool,
) -> anyhow::Result<SessionHandle> {
    let session_span = tracing::info_span!("session_start", session.id = tracing::field::Empty);
    async {
        let (mut start, cwd, mut lifecycle) = parse_start_params(&params)?;

        let mut config = load_config(&cwd)?;
        crate::params::apply_start_overrides(&mut config.settings, &start);
        lifecycle = lifecycle::select_lifecycle(
            lifecycle,
            params.get("lifecycle").is_some(),
            start.prompt.as_deref(),
            start.depth.unwrap_or(0),
            &config.settings.workflow,
        );
        start.lifecycle = lifecycle;
        let preset_session_id = start
            .resume
            .clone()
            .or_else(|| start.session_id.map(|id| id.to_string()))
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let kernel = kernel::build(
            connection,
            &config,
            &start,
            &cwd,
            &preset_session_id,
            hub,
            is_production,
        )
        .await?;
        let redaction_seed = kernel
            .secret_client()
            .and_then(|client| client.final_sink_redaction_seed())
            .unwrap_or_default();

        let (input_tx, input_rx) = tokio::sync::mpsc::channel::<AgentInput>(16);
        let interrupt = InterruptSignal::new();
        let (watch_tx, watch_rx) = tokio::sync::watch::channel(0u64);
        let interrupt_tx = Arc::new(watch_tx);
        let shutdown = tokio_util::sync::CancellationToken::new();

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
        let broadcaster = HubBroadcaster::new_with_redaction_seed(
            session_holder.clone(),
            None,
            redaction_seed.clone(),
        );
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

        // A workflow child must prove its attempt over the already
        // authenticated Hub connection before the start response or worker
        // task can be observed. The Hub derives execution identity from the
        // transport; only opaque workflow proof fields cross this boundary.
        workflow_handshake::send_if_worker(connection, &start).await?;

        bind_scheduler(&scheduler_for_bridge, &agent_params.session().id).await;
        // Tick loop activates after switch_session so the first survey sees
        // the loaded task set, not empty in-memory state.
        agent_shared_for_session.scheduler_handle.start();

        let session_id = agent_params.session().id.clone();
        tracing::Span::current().record("session.id", session_id.as_str());

        let session = Arc::new(SharedSession::new(
            session_id.clone(),
            input_tx,
            interrupt.clone(),
            interrupt_tx.clone(),
        ));
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
            redaction_seed,
            completion_result_limit: start
                .workflow_completion_result_limit
                .map(|limit| limit as usize)
                .unwrap_or(loopal_output_guard::MAX_AGENT_COMPLETION_RESULT_BYTES),
        })
    }
    .instrument(session_span)
    .await
}

async fn bind_scheduler(scheduler: &CronScheduler, session_id: &str) {
    if let Err(error) = scheduler.switch_session(session_id).await {
        tracing::warn!(error = %error, "failed to bind scheduler to session");
    }
}

#[cfg(test)]
mod scheduler_binding_tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use loopal_scheduler::{CronScheduler, PersistError, PersistedTask, SessionScopedCronStorage};

    use super::bind_scheduler;

    struct FailingStorage;

    #[async_trait]
    impl SessionScopedCronStorage for FailingStorage {
        async fn load(&self, _session_id: &str) -> Result<Vec<PersistedTask>, PersistError> {
            Err(PersistError::Io(std::io::Error::other("test load failure")))
        }

        async fn save_all(
            &self,
            _session_id: &str,
            _tasks: &[PersistedTask],
        ) -> Result<(), PersistError> {
            Ok(())
        }
    }

    struct EmptyStorage;

    #[async_trait]
    impl SessionScopedCronStorage for EmptyStorage {
        async fn load(&self, _session_id: &str) -> Result<Vec<PersistedTask>, PersistError> {
            Ok(Vec::new())
        }

        async fn save_all(
            &self,
            _session_id: &str,
            _tasks: &[PersistedTask],
        ) -> Result<(), PersistError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn scheduler_binding_logs_and_returns_on_storage_failure() {
        bind_scheduler(
            &CronScheduler::with_session_storage(Arc::new(EmptyStorage)),
            "ok-session",
        )
        .await;
        bind_scheduler(
            &CronScheduler::with_session_storage(Arc::new(FailingStorage)),
            "failed-session",
        )
        .await;
    }
}
