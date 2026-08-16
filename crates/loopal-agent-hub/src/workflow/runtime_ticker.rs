use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, oneshot};
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

use super::cleanup::clear_exact;
use super::{WorkflowCoordinatorError, WorkflowCoordinatorHandle, WorkflowRuntimeError};
use crate::Hub;
use crate::workflow::{SystemWorkflowClock, WorkflowClock};

const WORKFLOW_TICK_INTERVAL: Duration = Duration::from_millis(250);

pub(super) struct Ticker {
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<(), WorkflowCoordinatorError>>,
}

pub(super) fn start_ticker(hub: Arc<Mutex<Hub>>, handle: WorkflowCoordinatorHandle) -> Ticker {
    let (stop, mut stopped) = oneshot::channel();
    let task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(WORKFLOW_TICK_INTERVAL);
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        interval.tick().await;
        loop {
            tokio::select! {
                _ = &mut stopped => return Ok(()),
                _ = interval.tick() => {
                    let now = SystemWorkflowClock.now_unix_ms();
                    tokio::select! {
                        _ = &mut stopped => return Ok(()),
                        result = handle.tick(now) => {
                            if let Err(error) = result {
                                clear_exact(&hub, &handle).await;
                                hub.lock().await.shutdown_signal.notify_one();
                                return Err(error);
                            }
                        }
                    }
                }
            }
        }
    });
    Ticker {
        stop: Some(stop),
        task,
    }
}

pub(super) fn request_stop(ticker: &mut Option<Ticker>) {
    if let Some(current) = ticker.as_mut()
        && let Some(stop) = current.stop.take()
    {
        let _ = stop.send(());
    }
}

pub(super) async fn join(ticker: &mut Option<Ticker>) -> Result<(), WorkflowRuntimeError> {
    let Some(current) = ticker.as_mut() else {
        return Ok(());
    };
    let result = match (&mut current.task).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(WorkflowRuntimeError::Tick(error)),
        Err(error) => Err(WorkflowRuntimeError::TaskJoin {
            task: "workflow ticker",
            message: error.to_string(),
        }),
    };
    ticker.take();
    result
}

pub(super) fn abort(ticker: &mut Option<Ticker>) {
    if let Some(mut ticker) = ticker.take() {
        if let Some(stop) = ticker.stop.take() {
            let _ = stop.send(());
        }
        ticker.task.abort();
    }
}
