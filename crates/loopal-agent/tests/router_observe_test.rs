use loopal_agent::router::MessageRouter;
use loopal_types::envelope::{Envelope, MessageSource};
use loopal_types::event::{AgentEvent, AgentEventPayload};
use tokio::sync::mpsc;

fn make_router() -> (MessageRouter, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel::<AgentEvent>(64);
    (MessageRouter::new(tx), rx)
}

#[tokio::test]
async fn test_route_emits_message_routed_event() {
    let (router, mut obs_rx) = make_router();
    let (tx, _rx) = mpsc::channel::<Envelope>(16);
    router.register("alice", tx).await.unwrap();

    let env = Envelope::new(
        MessageSource::Agent("bob".to_string()),
        "alice",
        "hello alice",
    );
    router.route(env).await.unwrap();

    let event = obs_rx.recv().await.unwrap();
    match event.payload {
        AgentEventPayload::MessageRouted {
            source, target, content_preview,
        } => {
            assert_eq!(source, "bob");
            assert_eq!(target, "alice");
            assert_eq!(content_preview, "hello alice");
        }
        _ => panic!("expected MessageRouted event"),
    }
}

#[tokio::test]
async fn test_broadcast_emits_events_per_recipient() {
    let (router, mut obs_rx) = make_router();

    let (tx_a, _rx_a) = mpsc::channel::<Envelope>(16);
    let (tx_b, _rx_b) = mpsc::channel::<Envelope>(16);
    router.register("alice", tx_a).await.unwrap();
    router.register("bob", tx_b).await.unwrap();

    let env = Envelope::new(
        MessageSource::Human,
        "",
        "broadcast msg",
    );
    let delivered = router.broadcast(env, None).await.unwrap();
    assert_eq!(delivered.len(), 2);

    // Should get one MessageRouted event per recipient
    let mut events = Vec::new();
    while let Ok(ev) = obs_rx.try_recv() {
        events.push(ev);
    }
    assert_eq!(events.len(), 2);

    for event in &events {
        match &event.payload {
            AgentEventPayload::MessageRouted {
                source, content_preview, ..
            } => {
                assert_eq!(source, "human");
                assert_eq!(content_preview, "broadcast msg");
            }
            _ => panic!("expected MessageRouted event"),
        }
    }
}

#[tokio::test]
async fn test_route_human_source_attribution() {
    let (router, mut obs_rx) = make_router();
    let (tx, _rx) = mpsc::channel::<Envelope>(16);
    router.register("main", tx).await.unwrap();

    let env = Envelope::new(MessageSource::Human, "main", "user input");
    router.route(env).await.unwrap();

    let event = obs_rx.recv().await.unwrap();
    if let AgentEventPayload::MessageRouted { source, target, .. } = event.payload {
        assert_eq!(source, "human");
        assert_eq!(target, "main");
    } else {
        panic!("expected MessageRouted");
    }
}

#[tokio::test]
async fn test_route_channel_source_attribution() {
    let (router, mut obs_rx) = make_router();
    let (tx, _rx) = mpsc::channel::<Envelope>(16);
    router.register("worker", tx).await.unwrap();

    let env = Envelope::new(
        MessageSource::Channel {
            channel: "updates".to_string(),
            from: "notifier".to_string(),
        },
        "worker",
        "new data available",
    );
    router.route(env).await.unwrap();

    let event = obs_rx.recv().await.unwrap();
    if let AgentEventPayload::MessageRouted { source, target, content_preview } = event.payload {
        // Channel source shows the `from` field
        assert_eq!(source, "notifier");
        assert_eq!(target, "worker");
        assert_eq!(content_preview, "new data available");
    } else {
        panic!("expected MessageRouted");
    }
}

#[tokio::test]
async fn test_broadcast_excludes_sender() {
    let (router, mut obs_rx) = make_router();
    let (tx_a, _rx_a) = mpsc::channel::<Envelope>(16);
    let (tx_b, _rx_b) = mpsc::channel::<Envelope>(16);
    let (tx_c, _rx_c) = mpsc::channel::<Envelope>(16);
    router.register("a", tx_a).await.unwrap();
    router.register("b", tx_b).await.unwrap();
    router.register("c", tx_c).await.unwrap();

    let env = Envelope::new(
        MessageSource::Agent("a".to_string()),
        "",
        "hello everyone",
    );
    let delivered = router.broadcast(env, Some("a")).await.unwrap();

    // "a" excluded, "b" and "c" receive
    assert_eq!(delivered.len(), 2);
    assert!(!delivered.contains(&"a".to_string()));

    let mut events = Vec::new();
    while let Ok(ev) = obs_rx.try_recv() {
        events.push(ev);
    }
    assert_eq!(events.len(), 2);
    for ev in &events {
        if let AgentEventPayload::MessageRouted { source, target, .. } = &ev.payload {
            assert_eq!(source, "a");
            assert_ne!(target, "a");
        }
    }
}
