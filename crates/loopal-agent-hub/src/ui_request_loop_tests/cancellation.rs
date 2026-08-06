use super::*;

#[tokio::test]
async fn cancellation_keeps_request_id_owned_until_handler_completion() {
    let (cancel, cancelled) = tokio::sync::oneshot::channel();
    let mut in_flight = std::collections::HashMap::from([(
        17,
        super::super::InFlightRequest {
            method: methods::WORKSPACE_SEARCH.name.to_string(),
            cancel: Some(cancel),
        },
    )]);

    super::super::cancel_request(
        &mut in_flight,
        &json!({"id": 17, "method": methods::WORKSPACE_SEARCH.name}),
    );

    cancelled.await.expect("matching handler must be cancelled");
    assert!(
        in_flight.contains_key(&17),
        "request id ownership is released only by the completion path"
    );
    assert!(in_flight.get(&17).unwrap().cancel.is_none());
}

#[tokio::test]
async fn aborted_handler_marks_connection_unusable() {
    let (done, mut completed) = tokio::sync::mpsc::unbounded_channel();
    let guard = super::super::CompletionGuard::new(23, done);

    drop(guard);

    assert_eq!(completed.recv().await, Some((23, false)));
}

#[tokio::test]
async fn dropped_client_request_cancels_matching_handler_and_releases_slot() {
    struct DropFlag(Arc<std::sync::atomic::AtomicBool>);
    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }

    let started = Arc::new(Semaphore::new(0));
    let dropped = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let dispatcher = DispatcherBuilder::new()
        .register_fn(methods::WORKSPACE_SEARCH.name, {
            let started = started.clone();
            let dropped = dropped.clone();
            move |_params, _ctx| {
                let started = started.clone();
                let dropped = dropped.clone();
                Box::pin(async move {
                    let _guard = DropFlag(dropped);
                    started.add_permits(1);
                    std::future::pending::<()>().await;
                    Ok(json!(null))
                })
            }
        })
        .register_fn(methods::WORKSPACE_READ_FILE.name, |_params, _ctx| {
            Box::pin(async { Ok(json!({"content": "after-cancel"})) })
        })
        .build();
    let (client, io) = start(dispatcher);
    let pending_client = client.clone();
    let pending = tokio::spawn(async move {
        pending_client
            .send_request(methods::WORKSPACE_SEARCH.name, json!({}))
            .await
    });
    started.acquire().await.unwrap().forget();

    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(1), async {
        while !dropped.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("$/cancelRequest must drop the matching handler future");

    let response = client
        .send_request(methods::WORKSPACE_READ_FILE.name, json!({}))
        .await
        .unwrap();
    assert_eq!(response["content"], "after-cancel");
    client.close().await;
    io.await.unwrap();
}
