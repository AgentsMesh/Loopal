use std::time::Duration;

use loopal_ipc::connection::Connection;
use loopal_protocol::{AgentCompletion, Envelope, MessageSource, QualifiedAddress};
use tokio::sync::{Mutex, mpsc};

use super::*;

#[tokio::test]
async fn parent_reconnect_between_completion_commit_and_route_cannot_receive_old_result() {
    let (event_tx, _event_rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let (_old_peer_transport, old_hub_transport) = loopal_ipc::duplex_pair();
    let (old_parent, _old_parent_rx) = Connection::new(old_hub_transport).into_listening();
    {
        let mut h = hub.lock().await;
        h.registry
            .register_connection("parent", old_parent)
            .unwrap();
        h.registry
            .register_shadow(
                "remote-child",
                loopal_protocol::QualifiedAddress::local("parent"),
            )
            .unwrap();
    }
    let envelope = Envelope::new(
        MessageSource::AgentResult {
            child: QualifiedAddress::local("remote-child"),
        },
        QualifiedAddress::local("parent"),
        "old result",
    );

    let route = record_scoped(
        &hub,
        "remote-child",
        AgentCompletion::goal(Some("old result".into())),
        None,
        None,
        None,
    )
    .await;
    let expected_parent_generation = route
        .local_parent_generation()
        .expect("old parent route generation");

    let (new_peer_transport, new_hub_transport) = loopal_ipc::duplex_pair();
    let (_new_peer, mut new_parent_rx) = Connection::new(new_peer_transport).into_listening();
    let (new_parent, _new_hub_rx) = Connection::new(new_hub_transport).into_listening();
    {
        let mut h = hub.lock().await;
        h.registry.unregister_connection("parent");
        h.registry
            .register_connection("parent", new_parent)
            .unwrap();
    }

    assert!(
        !crate::uplink::reverse_route::deliver_for_generation(
            &hub,
            &envelope,
            expected_parent_generation,
        )
        .await
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(20), new_parent_rx.recv())
            .await
            .is_err()
    );
}
