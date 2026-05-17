use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;

use loopal_backend::{KillOutcome, kill_process_group};
use tokio::process::Child;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::AbortHandle;

use crate::control::{ControlSignal, StopOutcome, TaskStatus};

#[derive(Debug, Clone, Copy)]
pub struct MonitorTiming {
    pub drainers_grace: Duration,
    pub sigterm_grace: Duration,
}

type AckSlot = Option<(oneshot::Sender<StopOutcome>, StopOutcome)>;

pub async fn run_process_monitor(
    mut child: Child,
    pgid: Option<i32>,
    output_drainers: Vec<AbortHandle>,
    exit_code: Arc<AtomicI32>,
    status_tx: watch::Sender<TaskStatus>,
    mut control_rx: mpsc::Receiver<ControlSignal>,
    timing: MonitorTiming,
) {
    let mut ack_slot: AckSlot = None;

    let (final_status, code) = tokio::select! {
        biased;
        recv = control_rx.recv() => handle_control_recv(recv, pgid, &mut child, &mut ack_slot, timing).await,
        res = child.wait() => natural_exit(res),
    };

    tokio::time::sleep(timing.drainers_grace).await;
    for h in output_drainers {
        h.abort();
    }
    if let Some(c) = code {
        exit_code.store(c, Ordering::Release);
    }

    // reason: status_tx.send MUST precede ack.send so observers can't see
    // a stale Running state after a "killed" ack returns.
    let _ = status_tx.send(final_status);

    if let Some((ack, payload)) = ack_slot {
        let _ = ack.send(payload);
    }
}

async fn handle_control_recv(
    recv: Option<ControlSignal>,
    pgid: Option<i32>,
    child: &mut Child,
    ack_slot: &mut AckSlot,
    timing: MonitorTiming,
) -> (TaskStatus, Option<i32>) {
    match recv {
        Some(ControlSignal::Stop { ack }) => {
            let (kill, code) = kill_and_collect(pgid, child, timing.sigterm_grace).await;
            let payload = stop_outcome_from_kill(kill, code);
            *ack_slot = Some((ack, payload));
            (TaskStatus::Killed, code)
        }
        // reason: orchestrator dropped control_tx while child still alive —
        // force-kill to avoid orphan grandchildren.
        None => {
            let (_, code) = kill_and_collect(pgid, child, timing.sigterm_grace).await;
            (TaskStatus::Killed, code)
        }
    }
}

async fn kill_and_collect(
    pgid: Option<i32>,
    child: &mut Child,
    sigterm_grace: Duration,
) -> (KillOutcome, Option<i32>) {
    let outcome = kill_process_group(pgid, child, sigterm_grace).await;
    let code = child.wait().await.ok().and_then(|s| s.code());
    (outcome, code)
}

fn stop_outcome_from_kill(kill: KillOutcome, code: Option<i32>) -> StopOutcome {
    match kill {
        KillOutcome::Terminated | KillOutcome::Killed | KillOutcome::FallbackChild => {
            StopOutcome::Killed { exit_code: code }
        }
        KillOutcome::KillFailed(e) => StopOutcome::KillFailed(e),
    }
}

fn natural_exit(res: std::io::Result<std::process::ExitStatus>) -> (TaskStatus, Option<i32>) {
    let code = res.as_ref().ok().and_then(|s| s.code());
    let status = match code {
        Some(0) => TaskStatus::Completed,
        _ => TaskStatus::Failed,
    };
    (status, code)
}
