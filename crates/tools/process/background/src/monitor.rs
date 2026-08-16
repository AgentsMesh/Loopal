use std::future::Future;
use std::io;
use std::pin::Pin;
use std::process::ExitStatus;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;

use loopal_backend::{
    KillOutcome, ProcessCaptureState, ProcessCaptureTask, ProcessCompletion, SpawnedChild,
    Termination, process_capture_task,
};
use tokio::sync::{mpsc, oneshot, watch};

use crate::control::{ControlSignal, StopOutcome, TaskStatus};

const TERMINATION_RETRY_DELAY: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Copy)]
pub struct MonitorTiming {
    pub drainers_grace: Duration,
    pub sigterm_grace: Duration,
}

enum Trigger {
    Control(Option<ControlSignal>),
    Exited(io::Result<ExitStatus>),
    CaptureFailed,
    Retry,
}

struct FinalState {
    status: TaskStatus,
    exit_code: Option<i32>,
    stop_ack: Option<(oneshot::Sender<StopOutcome>, StopOutcome)>,
}

trait MonitoredProcess: Send {
    fn wait(&mut self) -> Pin<Box<dyn Future<Output = io::Result<ExitStatus>> + Send + '_>>;
    fn terminate(
        &mut self,
        grace: Duration,
    ) -> Pin<Box<dyn Future<Output = Termination> + Send + '_>>;
}

impl MonitoredProcess for SpawnedChild {
    fn wait(&mut self) -> Pin<Box<dyn Future<Output = io::Result<ExitStatus>> + Send + '_>> {
        Box::pin(SpawnedChild::wait(self))
    }

    fn terminate(
        &mut self,
        grace: Duration,
    ) -> Pin<Box<dyn Future<Output = Termination> + Send + '_>> {
        Box::pin(SpawnedChild::terminate(self, grace))
    }
}

pub async fn run_process_monitor(
    mut spawned: SpawnedChild,
    capture_task: ProcessCaptureTask,
    capture_state: Arc<ProcessCaptureState>,
    exit_code: Arc<AtomicI32>,
    status_tx: watch::Sender<TaskStatus>,
    mut control_rx: mpsc::Receiver<ControlSignal>,
    timing: MonitorTiming,
) {
    let mut final_state = monitor_process(
        &mut spawned,
        &mut control_rx,
        timing,
        capture_state.wait_for_capture_failure(),
    )
    .await;
    let capture = process_capture_task::join_bounded(capture_task, timing.drainers_grace).await;
    if capture.is_err() && final_state.status == TaskStatus::Completed {
        final_state.status = TaskStatus::Failed;
    }
    if let Some(code) = final_state.exit_code {
        exit_code.store(code, Ordering::Release);
    }
    capture_state.finalize(completion(final_state.status), final_state.exit_code);
    let _ = status_tx.send(final_state.status);
    if let Some((sender, outcome)) = final_state.stop_ack {
        let _ = sender.send(outcome);
    }
}

async fn monitor_process<P, F>(
    process: &mut P,
    control_rx: &mut mpsc::Receiver<ControlSignal>,
    timing: MonitorTiming,
    capture_failure: F,
) -> FinalState
where
    P: MonitoredProcess,
    F: Future<Output = ()>,
{
    tokio::pin!(capture_failure);
    let mut capture_observed = false;
    let mut control_open = true;
    let mut wait_enabled = true;
    let mut forced_status = None;
    let mut retry = false;
    loop {
        let trigger = tokio::select! {
            biased;
            control = control_rx.recv(), if control_open => Trigger::Control(control),
            _ = &mut capture_failure, if !capture_observed => Trigger::CaptureFailed,
            _ = tokio::time::sleep(TERMINATION_RETRY_DELAY), if retry => Trigger::Retry,
            result = process.wait(), if wait_enabled => Trigger::Exited(result),
        };
        match trigger {
            Trigger::Exited(Ok(status)) => {
                let (status, exit_code) = natural_exit(status);
                return FinalState {
                    status: forced_status.unwrap_or(status),
                    exit_code,
                    stop_ack: None,
                };
            }
            Trigger::Exited(Err(_)) => {
                wait_enabled = false;
                forced_status = Some(TaskStatus::Failed);
            }
            Trigger::CaptureFailed => {
                capture_observed = true;
                forced_status = Some(TaskStatus::Failed);
            }
            Trigger::Control(None) => {
                control_open = false;
                forced_status.get_or_insert(TaskStatus::Killed);
            }
            Trigger::Control(Some(ControlSignal::Stop { ack })) => {
                let termination = process.terminate(timing.sigterm_grace).await;
                let outcome = stop_outcome(&termination);
                if termination_succeeded(&termination) {
                    return FinalState {
                        status: forced_status.unwrap_or(TaskStatus::Killed),
                        exit_code: termination.exit_code,
                        stop_ack: Some((ack, outcome)),
                    };
                }
                let _ = ack.send(outcome);
                retry = forced_status.is_some();
                continue;
            }
            Trigger::Retry => {}
        }
        let termination = process.terminate(timing.sigterm_grace).await;
        if termination_succeeded(&termination) {
            return FinalState {
                status: forced_status.unwrap_or(TaskStatus::Killed),
                exit_code: termination.exit_code,
                stop_ack: None,
            };
        }
        retry = true;
    }
}

fn termination_succeeded(termination: &Termination) -> bool {
    !matches!(termination.outcome, KillOutcome::KillFailed(_))
}

fn stop_outcome(termination: &Termination) -> StopOutcome {
    match &termination.outcome {
        KillOutcome::Terminated | KillOutcome::Killed => StopOutcome::Killed {
            exit_code: termination.exit_code,
        },
        KillOutcome::KillFailed(error) => StopOutcome::KillFailed(error.clone()),
    }
}

fn completion(status: TaskStatus) -> ProcessCompletion {
    match status {
        TaskStatus::Completed => ProcessCompletion::Completed,
        TaskStatus::Failed => ProcessCompletion::Failed,
        TaskStatus::Killed | TaskStatus::Running => ProcessCompletion::Killed,
    }
}

fn natural_exit(status: ExitStatus) -> (TaskStatus, Option<i32>) {
    let code = status.code();
    let status = if code == Some(0) {
        TaskStatus::Completed
    } else {
        TaskStatus::Failed
    };
    (status, code)
}

#[cfg(test)]
mod tests;
