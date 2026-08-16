use super::*;

#[tokio::test]
async fn unrelated_notification_is_ignored() {
    let transport = TestTransport::recording();
    let (incoming, io) = start_with_incoming(DispatcherBuilder::new().build(), transport.clone());

    incoming
        .send(Incoming::Notification {
            method: "ui/unrelated".into(),
            params: json!({}),
        })
        .await
        .unwrap();
    incoming
        .send(Incoming::Request {
            id: 1,
            method: "ui/not-allowed".into(),
            params: json!({}),
        })
        .await
        .unwrap();

    transport.wait_for_frame().await;
    assert_eq!(transport.frame(0)["error"]["code"], -32600);
    drop(incoming);
    io.await.unwrap();
}

#[tokio::test]
async fn duplicate_request_id_is_rejected_while_original_runs() {
    let started = Arc::new(Semaphore::new(0));
    let signal = started.clone();
    let dispatcher = DispatcherBuilder::new()
        .register_fn(methods::WORKSPACE_SEARCH.name, move |_params, _ctx| {
            let signal = signal.clone();
            Box::pin(async move {
                signal.add_permits(1);
                pending::<()>().await;
                Ok(json!(null))
            })
        })
        .build();
    let transport = TestTransport::recording();
    let (incoming, io) = start_with_incoming(dispatcher, transport.clone());

    incoming
        .send(Incoming::Request {
            id: 7,
            method: methods::WORKSPACE_SEARCH.name.into(),
            params: json!({}),
        })
        .await
        .unwrap();
    started.acquire().await.unwrap().forget();
    incoming
        .send(Incoming::Request {
            id: 7,
            method: methods::WORKSPACE_READ_FILE.name.into(),
            params: json!({}),
        })
        .await
        .unwrap();

    transport.wait_for_frame().await;
    let response = transport.frame(0);
    assert_eq!(response["id"], 7);
    assert_eq!(response["error"]["code"], -32600);
    assert_eq!(
        response["error"]["message"],
        "duplicate in-flight JSON-RPC request id"
    );

    incoming
        .send(Incoming::Notification {
            method: methods::REQUEST_CANCEL.name.into(),
            params: json!({"id": 7, "method": methods::WORKSPACE_SEARCH.name}),
        })
        .await
        .unwrap();
    drop(incoming);
    io.await.unwrap();
}

#[tokio::test]
async fn duplicate_response_failure_stops_loop_and_aborts_original() {
    let started = Arc::new(Semaphore::new(0));
    let signal = started.clone();
    let dispatcher = DispatcherBuilder::new()
        .register_fn(methods::WORKSPACE_SEARCH.name, move |_params, _ctx| {
            let signal = signal.clone();
            Box::pin(async move {
                signal.add_permits(1);
                pending::<()>().await;
                Ok(json!(null))
            })
        })
        .build();
    let transport = TestTransport::failing();
    let (incoming, io) = start_with_incoming(dispatcher, transport.clone());

    for method in [
        methods::WORKSPACE_SEARCH.name,
        methods::WORKSPACE_READ_FILE.name,
    ] {
        incoming
            .send(Incoming::Request {
                id: 7,
                method: method.into(),
                params: json!({}),
            })
            .await
            .unwrap();
        if method == methods::WORKSPACE_SEARCH.name {
            started.acquire().await.unwrap().forget();
        }
    }

    tokio::time::timeout(Duration::from_secs(1), io)
        .await
        .expect("failed duplicate response must stop the loop")
        .unwrap();
    assert!(transport.closed.load(Ordering::Acquire));
}

#[tokio::test]
async fn busy_response_failure_stops_loop_and_aborts_handlers() {
    let started = Arc::new(Semaphore::new(0));
    let signal = started.clone();
    let dispatcher = DispatcherBuilder::new()
        .register_fn(methods::WORKSPACE_SEARCH.name, move |_params, _ctx| {
            let signal = signal.clone();
            Box::pin(async move {
                signal.add_permits(1);
                pending::<()>().await;
                Ok(json!(null))
            })
        })
        .build();
    let transport = TestTransport::failing();
    let (incoming, io) = start_with_incoming(dispatcher, transport.clone());

    for id in 0..UI_DATA_REQUEST_LIMIT as i64 {
        incoming
            .send(Incoming::Request {
                id,
                method: methods::WORKSPACE_SEARCH.name.into(),
                params: json!({}),
            })
            .await
            .unwrap();
    }
    started
        .acquire_many(UI_DATA_REQUEST_LIMIT as u32)
        .await
        .unwrap()
        .forget();
    incoming
        .send(Incoming::Request {
            id: UI_DATA_REQUEST_LIMIT as i64,
            method: methods::WORKSPACE_SEARCH.name.into(),
            params: json!({}),
        })
        .await
        .unwrap();

    tokio::time::timeout(Duration::from_secs(1), io)
        .await
        .expect("failed busy response must stop the loop")
        .unwrap();
    assert!(transport.closed.load(Ordering::Acquire));
}

#[tokio::test]
async fn handler_response_failure_closes_loop_transport() {
    let transport = TestTransport::failing();
    let (incoming, io) = start_with_incoming(DispatcherBuilder::new().build(), transport.clone());

    incoming
        .send(Incoming::Request {
            id: 23,
            method: "ui/not-allowed".into(),
            params: json!({}),
        })
        .await
        .unwrap();

    tokio::time::timeout(Duration::from_secs(1), io)
        .await
        .expect("failed handler response must stop the loop")
        .unwrap();
    assert!(transport.closed.load(Ordering::Acquire));
}
