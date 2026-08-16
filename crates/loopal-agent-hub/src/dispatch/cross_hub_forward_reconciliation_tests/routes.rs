#[tokio::test]
async fn cached_completion_covers_absent_and_successful_parent_routes() {
    let (events, _event_rx) = mpsc::channel::<AgentEvent>(16);
    let hub = Arc::new(Mutex::new(Hub::new(events)));
    let (hub_meta, _meta_peer) = loopal_ipc::duplex_pair();
    let (uplink_connection, _uplink_rx) = Connection::new(hub_meta).into_listening();
    let uplink = Arc::new(HubUplink::new(uplink_connection, "origin".into()));
    hub.lock().await.uplink = Some(uplink.clone());

    let remote_generation = {
        let mut hub = hub.lock().await;
        hub.registry
            .register_shadow_with_parent_policy_execution(
                "remote-parent",
                QualifiedAddress::remote(["other"], "parent"),
                true,
            )
            .unwrap()
            .connection_generation
    };
    drain_cached_completion(
        &hub,
        "remote-parent",
        remote_generation,
        &uplink,
        cached("remote-parent", "missing"),
    )
    .await;

    let (parent_peer, parent_transport) = loopal_ipc::duplex_pair();
    let (parent, mut parent_rx) = Connection::new(parent_peer).into_listening();
    let (parent_connection, _parent_incoming) = Connection::new(parent_transport).into_listening();
    let local_generation = {
        let mut hub = hub.lock().await;
        hub.registry
            .register_connection("main", parent_connection)
            .unwrap();
        hub.registry
            .register_shadow_with_parent_policy_execution(
                "local-parent",
                QualifiedAddress::local("main"),
                true,
            )
            .unwrap()
            .connection_generation
    };
    let responder = tokio::spawn(async move {
        let Incoming::Request { id, .. } = parent_rx.recv().await.unwrap() else {
            panic!("expected parent delivery");
        };
        parent.respond(id, serde_json::json!({})).await.unwrap();
    });
    drain_cached_completion(
        &hub,
        "local-parent",
        local_generation,
        &uplink,
        cached("local-parent", "main"),
    )
    .await;
    responder.await.unwrap();
}
