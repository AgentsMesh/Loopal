use loopagent_agent::router::MessageRouter;
use loopagent_types::event::AgentEvent;
use tokio::sync::mpsc;

fn make_router() -> (MessageRouter, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel::<AgentEvent>(64);
    (MessageRouter::new(tx), rx)
}

#[tokio::test]
async fn test_subscribe_and_publish() {
    let (router, _obs_rx) = make_router();

    router.subscribe("general", "alice").await;
    router.subscribe("general", "bob").await;

    let recipients = router.publish("general", "alice", "hello channel").await;
    // alice is excluded (sender), bob receives
    assert_eq!(recipients, vec!["bob".to_string()]);
}

#[tokio::test]
async fn test_unsubscribe_stops_delivery() {
    let (router, _obs_rx) = make_router();

    router.subscribe("general", "alice").await;
    router.subscribe("general", "bob").await;
    router.unsubscribe("general", "bob").await;

    let recipients = router.publish("general", "alice", "hello").await;
    assert!(recipients.is_empty());
}

#[tokio::test]
async fn test_read_channel_history() {
    let (router, _obs_rx) = make_router();

    router.subscribe("logs", "alice").await;
    router.publish("logs", "alice", "msg1").await;
    router.publish("logs", "bob", "msg2").await;
    router.publish("logs", "alice", "msg3").await;

    // Read all from start
    let all = router.read_channel("logs", 0).await;
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].content, "msg1");
    assert_eq!(all[2].content, "msg3");

    // Read after index 1 (skip first message)
    let partial = router.read_channel("logs", 1).await;
    assert_eq!(partial.len(), 2);
    assert_eq!(partial[0].content, "msg2");

    // Read after all messages
    let empty = router.read_channel("logs", 3).await;
    assert!(empty.is_empty());
}

#[tokio::test]
async fn test_list_channels() {
    let (router, _obs_rx) = make_router();

    router.subscribe("general", "alice").await;
    router.subscribe("logs", "bob").await;

    let mut channels = router.list_channels().await;
    channels.sort();
    assert_eq!(channels, vec!["general", "logs"]);
}
