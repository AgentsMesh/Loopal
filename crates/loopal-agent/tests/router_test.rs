use loopal_agent::router::MessageRouter;
use loopal_types::envelope::{Envelope, MessageSource};
use loopal_types::event::AgentEvent;
use tokio::sync::mpsc;

fn make_router() -> (MessageRouter, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel::<AgentEvent>(64);
    (MessageRouter::new(tx), rx)
}

fn agent_envelope(from: &str, target: &str, content: &str) -> Envelope {
    Envelope::new(
        MessageSource::Agent(from.to_string()),
        target,
        content,
    )
}

#[tokio::test]
async fn test_register_and_route_delivers_envelope() {
    let (router, _obs_rx) = make_router();
    let (tx, mut rx) = mpsc::channel::<Envelope>(16);

    router.register("alice", tx).await.unwrap();

    let env = agent_envelope("bob", "alice", "hello");
    router.route(env).await.unwrap();

    let received = rx.recv().await.unwrap();
    assert_eq!(received.target, "alice");
    assert_eq!(received.content, "hello");
}

#[tokio::test]
async fn test_route_to_unregistered_returns_error() {
    let (router, _obs_rx) = make_router();

    let env = agent_envelope("bob", "unknown", "hi");
    let result = router.route(env).await;

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("no mailbox"));
}

#[tokio::test]
async fn test_unregister_removes_mailbox() {
    let (router, _obs_rx) = make_router();
    let (tx, _rx) = mpsc::channel::<Envelope>(16);

    router.register("alice", tx).await.unwrap();
    router.unregister("alice").await;

    let env = agent_envelope("bob", "alice", "hello");
    let result = router.route(env).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_broadcast_delivers_to_all_except_excluded() {
    let (router, _obs_rx) = make_router();

    let (tx_a, mut rx_a) = mpsc::channel::<Envelope>(16);
    let (tx_b, mut rx_b) = mpsc::channel::<Envelope>(16);
    let (tx_c, mut rx_c) = mpsc::channel::<Envelope>(16);

    router.register("alice", tx_a).await.unwrap();
    router.register("bob", tx_b).await.unwrap();
    router.register("charlie", tx_c).await.unwrap();

    let env = agent_envelope("alice", "", "broadcast msg");
    let delivered = router.broadcast(env, Some("alice")).await.unwrap();

    // alice excluded, bob and charlie should receive
    assert_eq!(delivered.len(), 2);
    assert!(!delivered.contains(&"alice".to_string()));

    // Verify messages arrived
    assert!(rx_b.try_recv().is_ok());
    assert!(rx_c.try_recv().is_ok());
    assert!(rx_a.try_recv().is_err());
}
