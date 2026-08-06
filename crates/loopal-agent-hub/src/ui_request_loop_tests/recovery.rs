use super::*;

#[tokio::test]
async fn saturated_control_lane_does_not_block_interrupt_recovery() {
    let started = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let dispatcher = DispatcherBuilder::new()
        .register_fn(methods::HUB_CONTROL.name, {
            let started = started.clone();
            let release = release.clone();
            move |_params, _ctx| {
                let started = started.clone();
                let release = release.clone();
                Box::pin(async move {
                    started.add_permits(1);
                    release.acquire().await.unwrap().forget();
                    Ok(json!({"control": true}))
                })
            }
        })
        .register_fn(methods::HUB_INTERRUPT.name, |_params, _ctx| {
            Box::pin(async { Ok(json!({"interrupted": true})) })
        })
        .build();
    let (client, io) = start(dispatcher);
    let mut controls = Vec::new();
    for _ in 0..UI_CONTROL_REQUEST_LIMIT {
        let client = client.clone();
        controls.push(tokio::spawn(async move {
            client
                .send_request(methods::HUB_CONTROL.name, json!({}))
                .await
        }));
    }
    started
        .acquire_many(UI_CONTROL_REQUEST_LIMIT as u32)
        .await
        .unwrap()
        .forget();

    let interrupt = tokio::time::timeout(
        Duration::from_millis(500),
        client.send_request(methods::HUB_INTERRUPT.name, json!({})),
    )
    .await
    .expect("interrupt must not wait for the saturated control lane")
    .expect("interrupt must not receive a busy response");
    assert_eq!(interrupt["interrupted"], true);

    release.add_permits(UI_CONTROL_REQUEST_LIMIT);
    for control in controls {
        assert_eq!(control.await.unwrap().unwrap()["control"], true);
    }
    client.close().await;
    io.await.unwrap();
}
