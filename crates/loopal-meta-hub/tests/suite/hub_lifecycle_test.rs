//! Tests: hub lifecycle (disconnect cleanup) at the MetaHub level.

use std::sync::Arc;

use tokio::sync::Mutex;

use loopal_ipc::connection::Connection;

use loopal_meta_hub::MetaHub;

#[tokio::test]
async fn hub_disconnect_cleans_up_registry() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (_, meta_transport) = loopal_ipc::duplex_pair();
    let (meta_conn, _rx) = Connection::new(meta_transport).into_listening();
    {
        let mut mh = meta_hub.lock().await;
        mh.registry
            .register("dying-hub", meta_conn, vec![])
            .unwrap();
        mh.remove_hub("dying-hub");
        assert_eq!(mh.registry.len(), 0);
    }
}

#[tokio::test]
async fn stale_connection_cleanup_cannot_remove_its_replacement() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (old_transport, _) = loopal_ipc::duplex_pair();
    let (old, _old_rx) = loopal_ipc::connection::Connection::new(old_transport).into_listening();
    meta_hub
        .lock()
        .await
        .registry
        .register("desktop", old.clone(), vec![])
        .unwrap();
    old.close().await;
    let (new_transport, _) = loopal_ipc::duplex_pair();
    let (new, _new_rx) = loopal_ipc::connection::Connection::new(new_transport).into_listening();
    meta_hub
        .lock()
        .await
        .registry
        .register("desktop", new.clone(), vec![])
        .unwrap();
    assert!(
        meta_hub
            .lock()
            .await
            .registry
            .unregister_connection("desktop", &old)
            .is_none()
    );
    assert!(Arc::ptr_eq(
        &meta_hub
            .lock()
            .await
            .registry
            .connection("desktop")
            .unwrap(),
        &new,
    ));
}
