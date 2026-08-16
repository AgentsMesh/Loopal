use super::*;

fn cancel() -> TurnCancel {
    TurnCancel::new(
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
    )
}

fn success(id: &str) -> PendingToolResult {
    PendingToolResult::new(id, "Read", "ok", false, None)
}

#[tokio::test]
async fn collects_success_and_ignores_panicked_task() {
    let mut tasks = tokio::task::JoinSet::new();
    tasks.spawn(async { (1, success("done")) });
    tasks.spawn(async { panic!("tool panic") });

    let results = collect_results(
        &mut tasks,
        &[("done".into(), "Read".into())],
        &[("done".into(), "Read".into(), serde_json::json!({}))],
        &cancel(),
    )
    .await;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, 1);
    assert!(!results[0].1.is_error());
}

#[tokio::test]
async fn active_cancel_drains_result_that_completed_before_abort() {
    let signal = loopal_protocol::InterruptSignal::new();
    let (watch_tx, _watch_rx) = tokio::sync::watch::channel(0u64);
    let cancel = TurnCancel::new(signal.clone(), std::sync::Arc::new(watch_tx.clone()));
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let mut tasks = tokio::task::JoinSet::new();
    tasks.spawn(async move {
        release_rx.await.unwrap();
        (0, success("done"))
    });
    let trigger = tokio::spawn(async move {
        signal.signal();
        watch_tx.send_modify(|generation| *generation += 1);
        release_tx.send(()).unwrap();
    });

    let results = collect_results(
        &mut tasks,
        &[("done".into(), "Read".into())],
        &[("done".into(), "Read".into(), serde_json::json!({}))],
        &cancel,
    )
    .await;
    trigger.await.unwrap();

    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn cancellation_while_waiting_drains_and_synthesizes_every_tool() {
    let signal = loopal_protocol::InterruptSignal::new();
    let (watch_tx, _watch_rx) = tokio::sync::watch::channel(0u64);
    let cancel = TurnCancel::new(signal.clone(), std::sync::Arc::new(watch_tx.clone()));
    let mut tasks = tokio::task::JoinSet::new();
    for index in 0..2 {
        tasks.spawn(async move {
            std::future::pending::<()>().await;
            (index, success(&format!("slow-{index}")))
        });
    }
    let trigger = tokio::spawn(async move {
        tokio::task::yield_now().await;
        signal.signal();
        watch_tx.send_modify(|generation| *generation += 1);
    });

    let approved = vec![
        ("slow-0".into(), "Slow".into()),
        ("slow-1".into(), "Slow".into()),
    ];
    let tool_uses = vec![
        ("slow-0".into(), "Slow".into(), serde_json::json!({})),
        ("slow-1".into(), "Slow".into(), serde_json::json!({})),
    ];
    let results = collect_results(&mut tasks, &approved, &tool_uses, &cancel).await;
    trigger.await.unwrap();

    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|(_, result)| result.is_error()));
}

#[tokio::test]
async fn pre_cancel_synthesizes_interrupted_results() {
    let signal = loopal_protocol::InterruptSignal::new();
    signal.signal();
    let (watch_tx, _watch_rx) = tokio::sync::watch::channel(1u64);
    let cancel = TurnCancel::new(signal, std::sync::Arc::new(watch_tx));
    let mut tasks = tokio::task::JoinSet::new();
    tasks.spawn(async {
        std::future::pending::<()>().await;
        (0, success("slow"))
    });

    let results = collect_results(
        &mut tasks,
        &[("slow".into(), "Slow".into())],
        &[("slow".into(), "Slow".into(), serde_json::json!({}))],
        &cancel,
    )
    .await;

    assert_eq!(results.len(), 1);
    assert!(results[0].1.is_error());
}
