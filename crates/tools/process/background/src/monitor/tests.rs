use std::collections::VecDeque;
use std::future::{Future, pending};
use std::io;
use std::pin::Pin;
use std::process::ExitStatus;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};

use super::{MonitorTiming, MonitoredProcess, monitor_process};
use crate::{ControlSignal, StopOutcome, TaskStatus};
use loopal_backend::{KillOutcome, Termination};

struct FakeProcess {
    terminations: VecDeque<Termination>,
    calls: usize,
}

impl FakeProcess {
    fn new(terminations: Vec<Termination>) -> Self {
        Self {
            terminations: terminations.into(),
            calls: 0,
        }
    }
}

impl MonitoredProcess for FakeProcess {
    fn wait(&mut self) -> Pin<Box<dyn Future<Output = io::Result<ExitStatus>> + Send + '_>> {
        Box::pin(pending())
    }

    fn terminate(
        &mut self,
        _grace: Duration,
    ) -> Pin<Box<dyn Future<Output = Termination> + Send + '_>> {
        self.calls += 1;
        let result = self.terminations.pop_front().expect("termination result");
        Box::pin(async move { result })
    }
}

fn failed() -> Termination {
    Termination {
        outcome: KillOutcome::KillFailed("injected failure".into()),
        exit_code: None,
    }
}

fn killed() -> Termination {
    Termination {
        outcome: KillOutcome::Killed,
        exit_code: Some(9),
    }
}

fn timing() -> MonitorTiming {
    MonitorTiming {
        drainers_grace: Duration::from_millis(10),
        sigterm_grace: Duration::from_millis(10),
    }
}

#[tokio::test]
async fn failed_stop_ack_keeps_monitor_owning_until_retry_succeeds() {
    let mut process = FakeProcess::new(vec![failed(), killed()]);
    let (control_tx, mut control_rx) = mpsc::channel(2);
    let driver = tokio::spawn(async move {
        let (first_tx, first_rx) = oneshot::channel();
        control_tx
            .send(ControlSignal::Stop { ack: first_tx })
            .await
            .unwrap();
        assert!(matches!(
            first_rx.await.unwrap(),
            StopOutcome::KillFailed(_)
        ));
        let (second_tx, second_rx) = oneshot::channel();
        control_tx
            .send(ControlSignal::Stop { ack: second_tx })
            .await
            .unwrap();
        assert!(matches!(
            second_rx.await.unwrap(),
            StopOutcome::Killed { .. }
        ));
    });

    let final_state = monitor_process(&mut process, &mut control_rx, timing(), pending()).await;
    let (ack, outcome) = final_state.stop_ack.expect("successful stop ack");
    ack.send(outcome).unwrap();
    driver.await.unwrap();
    assert_eq!(process.calls, 2);
    assert_eq!(final_state.status, TaskStatus::Killed);
}

#[tokio::test]
async fn capture_failure_retries_termination_without_relinquishing_owner() {
    let mut process = FakeProcess::new(vec![failed(), killed()]);
    let (_control_tx, mut control_rx) = mpsc::channel(1);
    let final_state = monitor_process(&mut process, &mut control_rx, timing(), async {}).await;
    assert_eq!(process.calls, 2);
    assert_eq!(final_state.status, TaskStatus::Failed);
}

#[tokio::test]
async fn control_loss_retries_termination_without_relinquishing_owner() {
    let mut process = FakeProcess::new(vec![failed(), killed()]);
    let (control_tx, mut control_rx) = mpsc::channel(1);
    drop(control_tx);
    let final_state = monitor_process(&mut process, &mut control_rx, timing(), pending()).await;
    assert_eq!(process.calls, 2);
    assert_eq!(final_state.status, TaskStatus::Killed);
}
