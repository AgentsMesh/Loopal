use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentCompletion, WorkflowPermissionCausation};
use std::future::Future;
use std::time::Duration;
use tokio::sync::oneshot;

use super::super::{AttemptPhase, ProductionWorkflowSpawner};
use crate::types::AgentExecutionRef;
use crate::workflow::scheduler::WorkflowCleanupStatus;
const MAX_CLEANUP_RETRIES: u8 = 3;

pub(in crate::spawn_manager::workflow) fn shutdown(
    spawner: &ProductionWorkflowSpawner,
    execution: &AgentExecutionRef,
    timeout: Duration,
) -> impl Future<Output = WorkflowCleanupStatus> + Send + 'static {
    let done_rx = spawn_cleanup(spawner, execution, timeout);
    async move {
        match tokio::time::timeout(timeout, done_rx).await {
            Ok(Ok(status)) => status,
            Ok(Err(_)) | Err(_) => WorkflowCleanupStatus::TimedOut,
        }
    }
}
#[cfg(test)]
pub(in crate::spawn_manager::workflow) fn shutdown_supervisor_for_test(
    spawner: &ProductionWorkflowSpawner,
    execution: &AgentExecutionRef,
    timeout: Duration,
) -> oneshot::Receiver<WorkflowCleanupStatus> {
    spawn_cleanup(spawner, execution, timeout)
}
fn spawn_cleanup(
    spawner: &ProductionWorkflowSpawner,
    execution: &AgentExecutionRef,
    timeout: Duration,
) -> oneshot::Receiver<WorkflowCleanupStatus> {
    let cleanup_spawner = spawner.clone();
    let execution = execution.clone();
    let (done_tx, done_rx) = oneshot::channel();
    tokio::spawn(async move {
        let operation = {
            let mut owners = cleanup_spawner.attempts.lock().await;
            let Some(attempt) = super::exact_mut(&mut owners, &execution) else {
                let _ = done_tx.send(WorkflowCleanupStatus::Confirmed);
                return;
            };
            attempt.phase = AttemptPhase::Stopping;
            attempt.cleanup_registered = true;
            attempt.operation.clone()
        };
        let _operation = match tokio::time::timeout(timeout, operation.lock()).await {
            Ok(operation) => operation,
            Err(_) => {
                let _ = done_tx.send(WorkflowCleanupStatus::TimedOut);
                escalate_cleanup(&cleanup_spawner, &execution);
                return;
            }
        };
        let Some(cleanup) = take_cleanup(&cleanup_spawner, &execution).await else {
            let _ = done_tx.send(WorkflowCleanupStatus::Confirmed);
            return;
        };
        let audit = super::super::lifecycle_audit::append_before_cleanup(
            &cleanup_spawner,
            &cleanup.owner,
            &cleanup.causation,
            Some(&execution),
            super::super::lifecycle_audit::WorkflowAuditPhase::Shutdown,
        );
        let _ = tokio::time::timeout(timeout, audit).await;
        let request = cleanup
            .control
            .connection
            .send_request(methods::AGENT_SHUTDOWN.name, serde_json::Value::Null);
        let _ = tokio::time::timeout(timeout, request).await;
        let mut process_shutdown = cleanup.process_shutdown;
        let mut process_confirmed = false;
        let mut detach_confirmed = false;
        let mut done_tx = Some(done_tx);
        let mut retries = 0;
        loop {
            if !process_confirmed {
                match wait_process_shutdown(&mut process_shutdown, timeout).await {
                    ProcessShutdownStatus::Confirmed => process_confirmed = true,
                    ProcessShutdownStatus::Pending => {}
                    ProcessShutdownStatus::Lost => {
                        escalate_cleanup(&cleanup_spawner, &execution);
                        if let Some(done_tx) = done_tx.take() {
                            let _ = done_tx.send(WorkflowCleanupStatus::TimedOut);
                        }
                        return;
                    }
                }
            }
            if !detach_confirmed {
                detach_confirmed = super::finish_exact(
                    &cleanup_spawner,
                    &execution,
                    &cleanup.control,
                    AgentCompletion::new("workflow_stopped", None),
                    timeout,
                )
                .await
                    == WorkflowCleanupStatus::Confirmed;
            }
            if process_confirmed && detach_confirmed {
                cleanup_spawner.finish_owner(&execution).await;
                if let Some(done_tx) = done_tx.take() {
                    let _ = done_tx.send(WorkflowCleanupStatus::Confirmed);
                }
                return;
            }
            if let Some(done_tx) = done_tx.take() {
                let _ = done_tx.send(WorkflowCleanupStatus::TimedOut);
            }
            if retries == MAX_CLEANUP_RETRIES {
                escalate_cleanup(&cleanup_spawner, &execution);
                return;
            }
            retries += 1;
            retry_pause(timeout).await;
        }
    });
    done_rx
}
async fn retry_pause(timeout: Duration) {
    let delay = timeout
        .max(Duration::from_millis(10))
        .min(Duration::from_secs(1));
    tokio::time::sleep(delay).await;
}

async fn take_cleanup(
    spawner: &ProductionWorkflowSpawner,
    execution: &AgentExecutionRef,
) -> Option<CleanupOwner> {
    let mut owners = spawner.attempts.lock().await;
    let attempt = super::exact_mut(&mut owners, execution)?;
    attempt.phase = AttemptPhase::Stopping;
    let process_shutdown = match &attempt.process_shutdown {
        Some(shutdown) => shutdown.clone(),
        None => {
            let (complete, shutdown) = tokio::sync::watch::channel(false);
            attempt.process_shutdown = Some(shutdown.clone());
            if let Some(process) = attempt.process.take() {
                tokio::spawn(async move {
                    if process.shutdown().await.is_ok() {
                        let _ = complete.send(true);
                    }
                });
            }
            shutdown
        }
    };
    Some(CleanupOwner {
        control: attempt.control.clone(),
        process_shutdown,
        owner: attempt.owner.clone(),
        causation: attempt.causation.clone(),
    })
}

struct CleanupOwner {
    control: std::sync::Arc<crate::spawn_manager::spawn::PreparedControl>,
    process_shutdown: tokio::sync::watch::Receiver<bool>,
    owner: crate::workflow::WorkflowOwner,
    causation: WorkflowPermissionCausation,
}

enum ProcessShutdownStatus {
    Confirmed,
    Pending,
    Lost,
}

async fn wait_process_shutdown(
    shutdown: &mut tokio::sync::watch::Receiver<bool>,
    timeout: Duration,
) -> ProcessShutdownStatus {
    if *shutdown.borrow() {
        return ProcessShutdownStatus::Confirmed;
    }
    match tokio::time::timeout(timeout, shutdown.changed()).await {
        Ok(Ok(())) if *shutdown.borrow() => ProcessShutdownStatus::Confirmed,
        Ok(Ok(())) | Err(_) => ProcessShutdownStatus::Pending,
        Ok(Err(_)) => ProcessShutdownStatus::Lost,
    }
}

fn escalate_cleanup(spawner: &ProductionWorkflowSpawner, execution: &AgentExecutionRef) {
    tracing::error!(
        agent = %execution.address,
        generation = execution.connection_generation,
        "workflow process shutdown containment did not settle; requesting Hub shutdown"
    );
    spawner.shutdown_signal.notify_one();
}
