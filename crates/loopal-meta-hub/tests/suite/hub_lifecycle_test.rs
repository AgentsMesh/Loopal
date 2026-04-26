//! Tests: hub lifecycle (disconnect cleanup) at the MetaHub level.

use std::sync::Arc;

use tokio::sync::Mutex;

use loopal_ipc::connection::Connection;

use loopal_meta_hub::MetaHub;

#[tokio::test]
async fn hub_disconnect_cleans_up_registry() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (_, meta_transport) = loopal_ipc::duplex_pair();
    let meta_conn = Arc::new(Connection::new(meta_transport));
    let _rx = meta_conn.start();
    {
        let mut mh = meta_hub.lock().await;
        mh.registry
            .register("dying-hub", meta_conn, vec![])
            .unwrap();
        mh.remove_hub("dying-hub");
        assert_eq!(mh.registry.len(), 0);
    }
}
