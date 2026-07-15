use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use loopal_ipc::connection::{Connection, Listening};
use loopal_ipc::protocol::methods;
use loopal_ipc::{Dispatcher, DispatcherBuilder, RpcError};
use serde_json::json;
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinHandle;

use super::{UI_DATA_REQUEST_LIMIT, ui_client_io_loop};
use crate::Hub;

#[tokio::test]
async fn slow_request_does_not_block_fast_request_and_disconnect_aborts_it() {
    let started = Arc::new(Semaphore::new(0));
    let signal = started.clone();
    let dispatcher = DispatcherBuilder::new()
        .register_fn(methods::WORKSPACE_SEARCH.name, move |_params, _ctx| {
            let signal = signal.clone();
            Box::pin(async move {
                signal.add_permits(1);
                std::future::pending::<()>().await;
                Ok(json!(null))
            })
        })
        .register_fn(methods::WORKSPACE_READ_FILE.name, |_params, _ctx| {
            Box::pin(async { Ok(json!({"content": "fast"})) })
        })
        .build();
    let (client, io) = start(dispatcher);
    let request_client = client.clone();
    let slow = tokio::spawn(async move {
        request_client
            .send_request(methods::WORKSPACE_SEARCH.name, json!({}))
            .await
    });
    started.acquire().await.unwrap().forget();
    let fast = tokio::time::timeout(
        Duration::from_millis(500),
        client.send_request(methods::WORKSPACE_READ_FILE.name, json!({})),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(fast["content"], "fast");
    client.close().await;
    tokio::time::timeout(Duration::from_secs(1), io)
        .await
        .unwrap()
        .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(1), slow)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
}

#[tokio::test]
async fn concurrent_requests_never_exceed_limit() {
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let dispatcher = DispatcherBuilder::new()
        .register_fn(methods::WORKSPACE_SEARCH.name, {
            let active = active.clone();
            let maximum = maximum.clone();
            let started = started.clone();
            let release = release.clone();
            move |_params, _ctx| {
                let active = active.clone();
                let maximum = maximum.clone();
                let started = started.clone();
                let release = release.clone();
                Box::pin(async move {
                    let now = active.fetch_add(1, Ordering::AcqRel) + 1;
                    maximum.fetch_max(now, Ordering::AcqRel);
                    started.add_permits(1);
                    release.acquire().await.unwrap().forget();
                    active.fetch_sub(1, Ordering::AcqRel);
                    Ok(json!({"ok": true}))
                })
            }
        })
        .register_fn(methods::HUB_INTERRUPT.name, |_params, _ctx| {
            Box::pin(async { Ok(json!({"control": true})) })
        })
        .build();
    let (client, io) = start(dispatcher);
    let mut tasks = Vec::new();
    for _ in 0..=UI_DATA_REQUEST_LIMIT {
        let client = client.clone();
        tasks.push(tokio::spawn(async move {
            client
                .send_request(methods::WORKSPACE_SEARCH.name, json!({}))
                .await
        }));
    }
    started
        .acquire_many(UI_DATA_REQUEST_LIMIT as u32)
        .await
        .unwrap()
        .forget();
    let control = tokio::time::timeout(
        Duration::from_millis(500),
        client.send_request(methods::HUB_INTERRUPT.name, json!({})),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(control["control"], true);
    release.add_permits(UI_DATA_REQUEST_LIMIT);
    let mut busy = 0;
    for task in tasks {
        if matches!(
            task.await.unwrap(),
            Err(RpcError::Remote { code: -32000, .. })
        ) {
            busy += 1;
        }
    }
    assert_eq!(busy, 1);
    assert_eq!(maximum.load(Ordering::Acquire), UI_DATA_REQUEST_LIMIT);
    client.close().await;
    io.await.unwrap();
}

fn start(dispatcher: Dispatcher) -> (Arc<Connection<Listening>>, JoinHandle<()>) {
    let (client_transport, server_transport) = loopal_ipc::duplex_pair();
    let (client, _client_rx) = Connection::new(client_transport).into_listening();
    let (server, server_rx) = Connection::new(server_transport).into_listening();
    let hub = Arc::new(Mutex::new(Hub::noop()));
    let task = tokio::spawn(ui_client_io_loop(
        hub,
        Arc::new(dispatcher),
        server,
        server_rx,
        "ui-test".into(),
    ));
    (client, task)
}
