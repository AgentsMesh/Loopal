use std::sync::Arc;
use std::time::Duration;

use loopal_protocol::ControlCommand;
use loopal_scheduler::CronScheduler;
use loopal_test_support::HarnessBuilder;

#[tokio::test]
async fn cron_delete_via_control_removes_job() {
    let scheduler = Arc::new(CronScheduler::new());
    scheduler
        .add("* * * * *", "ping", true, false)
        .await
        .expect("add cron");
    let ids = scheduler
        .list()
        .await
        .into_iter()
        .map(|j| j.id)
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 1);
    let id = ids.into_iter().next().unwrap();

    let harness = HarnessBuilder::new()
        .messages(vec![])
        .scheduler(scheduler.clone())
        .build_spawned()
        .await;

    harness
        .control_tx
        .send(ControlCommand::CronDelete { id: id.clone() })
        .await
        .unwrap();

    // Allow the runtime to drain the control channel.
    for _ in 0..50 {
        if scheduler.list().await.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(
        scheduler.list().await.is_empty(),
        "scheduler should have no jobs after CronDelete"
    );
    drop(harness.control_tx);
}

#[tokio::test]
async fn cron_delete_unknown_id_is_silent_noop() {
    let scheduler = Arc::new(CronScheduler::new());
    let harness = HarnessBuilder::new()
        .messages(vec![])
        .scheduler(scheduler.clone())
        .build_spawned()
        .await;

    harness
        .control_tx
        .send(ControlCommand::CronDelete {
            id: "unknown".into(),
        })
        .await
        .unwrap();

    // No panic, scheduler stays empty.
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert!(scheduler.list().await.is_empty());
    drop(harness.control_tx);
}

#[tokio::test]
async fn bg_task_kill_via_control_against_missing_id_is_noop() {
    // The default harness creates a fresh BackgroundTaskStore with no
    // running tasks, so BgTaskKill targets a missing id and exercises the
    // "not found" path in bg_stop without setting up a real subprocess.
    let scheduler = Arc::new(CronScheduler::new());
    let harness = HarnessBuilder::new()
        .messages(vec![])
        .scheduler(scheduler)
        .build_spawned()
        .await;

    harness
        .control_tx
        .send(ControlCommand::BgTaskKill {
            id: "bg_does_not_exist".into(),
        })
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(80)).await;
    drop(harness.control_tx);
}
