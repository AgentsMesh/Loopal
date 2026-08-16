use loopal_ipc::Connection;
use loopal_protocol::{UiCapabilities, UiCapability};

use crate::UiDispatcher;

fn connection() -> std::sync::Arc<Connection<loopal_ipc::Listening>> {
    let (_peer, transport) = loopal_ipc::duplex_pair();
    Connection::new(transport).into_listening().0
}

#[tokio::test]
async fn default_dispatcher_tracks_resync_and_capability_union() {
    let mut dispatcher = UiDispatcher::default();
    dispatcher.request_resync();
    assert_eq!(dispatcher.capability_snapshot().generation, 0);
    assert!(!dispatcher.has_capability(UiCapability::Permission));
    assert!(!dispatcher.client_has_capability("missing", UiCapability::Question));

    let mut resync = dispatcher.subscribe_resync();
    dispatcher.request_resync();
    resync.recv().await.unwrap();

    dispatcher.register_client_with_lease(
        "permission",
        "desktop",
        connection(),
        UiCapabilities {
            permission: true,
            question: false,
            plan_approval: false,
        },
    );
    dispatcher.register_client_with_lease(
        "questions",
        "terminal",
        connection(),
        UiCapabilities {
            permission: false,
            question: true,
            plan_approval: true,
        },
    );
    assert_eq!(
        dispatcher.capability_snapshot().capabilities,
        UiCapabilities::ALL
    );
    assert!(dispatcher.has_client_name("desktop"));
    assert!(dispatcher.has_capability(UiCapability::PlanApproval));
    assert!(!dispatcher.client_has_capability("permission", UiCapability::Question));

    let generation = dispatcher.capability_snapshot().generation;
    dispatcher.unregister_client("missing");
    assert_eq!(dispatcher.capability_snapshot().generation, generation);
    dispatcher.unregister_client("questions");
    assert_eq!(dispatcher.capability_snapshot().generation, generation + 1);
    assert!(!dispatcher.has_capability(UiCapability::Question));
}
