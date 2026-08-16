use super::super::ProductionWorkflowSpawner;
use crate::types::AgentExecutionRef;

pub(super) struct PreparedRegistrationGuard {
    spawner: ProductionWorkflowSpawner,
    execution: AgentExecutionRef,
    armed: bool,
}

impl PreparedRegistrationGuard {
    pub(super) fn new(spawner: &ProductionWorkflowSpawner, execution: AgentExecutionRef) -> Self {
        Self {
            spawner: spawner.clone(),
            execution,
            armed: true,
        }
    }

    pub(super) fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for PreparedRegistrationGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let panicking = std::thread::panicking();
        if panicking {
            self.spawner.shutdown_signal.notify_one();
        }
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let spawner = self.spawner.clone();
        let execution = self.execution.clone();
        runtime.spawn(async move {
            let (connection, mcp, removed) = {
                let mut hub = spawner.hub.lock().await;
                let connection = hub.registry.exact_connection(&execution);
                hub.clear_permission_grants(&execution);
                hub.spawn_registry.unregister_exact(&execution);
                let removed = hub.registry.unregister_exact(&execution);
                (connection, hub.mcp_service.clone(), removed)
            };
            if removed || connection.is_some() {
                mcp.on_agent_detach(&execution).await;
            }
            if let Some(connection) = connection {
                let _ = tokio::time::timeout(std::time::Duration::from_secs(2), connection.close())
                    .await;
            }
            spawner.changed.notify_waiters();
        });
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use loopal_protocol::AgentEvent;
    use tokio::sync::{Mutex, mpsc};

    use super::*;
    use crate::Hub;

    #[tokio::test]
    async fn dropped_guard_removes_unpublished_exact_registration() {
        let (events, _event_rx) = mpsc::channel::<AgentEvent>(2);
        let hub = Arc::new(Mutex::new(Hub::new(events)));
        let shutdown_signal = hub.lock().await.shutdown_signal.clone();
        let spawner = ProductionWorkflowSpawner::new(hub.clone(), shutdown_signal);
        let (transport, _peer) = loopal_ipc::duplex_pair();
        let connection = loopal_ipc::Connection::new(transport).into_listening().0;
        let execution = hub
            .lock()
            .await
            .registry
            .register_connection_with_parent_execution(
                "unpublished-workflow-worker",
                connection,
                None,
                None,
                None,
            )
            .unwrap();

        drop(PreparedRegistrationGuard::new(&spawner, execution));

        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if hub
                    .lock()
                    .await
                    .registry
                    .current_execution("unpublished-workflow-worker")
                    .is_none()
                {
                    return;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn panicking_guard_signals_shutdown_without_runtime_or_hub_lock() {
        let (events, _event_rx) = mpsc::channel::<AgentEvent>(2);
        let hub = Arc::new(Mutex::new(Hub::new(events)));
        let shutdown_signal = hub.lock().await.shutdown_signal.clone();
        let spawner = ProductionWorkflowSpawner::new(hub.clone(), shutdown_signal.clone());
        let (transport, _peer) = loopal_ipc::duplex_pair();
        let connection = loopal_ipc::Connection::new(transport).into_listening().0;
        let execution = hub
            .lock()
            .await
            .registry
            .register_connection_with_parent_execution(
                "panic-workflow-worker",
                connection,
                None,
                None,
                None,
            )
            .unwrap();
        let shutdown = shutdown_signal.notified();
        tokio::pin!(shutdown);
        shutdown.as_mut().enable();
        let hub_guard = hub.lock().await;
        let guard = PreparedRegistrationGuard::new(&spawner, execution);

        let panic = std::thread::spawn(move || {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _guard = guard;
                panic!("exercise detached preparation panic containment");
            }))
        })
        .join()
        .unwrap();

        assert!(panic.is_err());
        shutdown.await;
        drop(hub_guard);
    }
}
