use loopagent_tui::event::{AppEvent, EventHandler};
use loopagent_types::event::AgentEvent;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_event_handler_construction() {
    let (_agent_tx, agent_rx) = mpsc::channel::<AgentEvent>(16);
    let _handler = EventHandler::new(agent_rx);
    // Construction should succeed without panic
}

#[tokio::test]
async fn test_try_next_returns_none_when_empty() {
    let (_agent_tx, agent_rx) = mpsc::channel::<AgentEvent>(16);
    let mut handler = EventHandler::new(agent_rx);

    let result = handler.try_next();
    assert!(result.is_none(), "try_next should return None when no events are queued");
}

#[tokio::test]
async fn test_agent_events_come_through() {
    let (agent_tx, agent_rx) = mpsc::channel::<AgentEvent>(16);
    let mut handler = EventHandler::new(agent_rx);

    agent_tx
        .send(AgentEvent::Started)
        .await
        .expect("send should succeed");

    let event = tokio::time::timeout(std::time::Duration::from_secs(2), handler.next())
        .await
        .expect("should receive event within timeout")
        .expect("channel should not be closed");

    match event {
        AppEvent::Agent(AgentEvent::Started) => {} // expected
        other => panic!("expected Agent(Started), got {:?}", other),
    }
}

#[tokio::test]
async fn test_agent_stream_event_forwarded() {
    let (agent_tx, agent_rx) = mpsc::channel::<AgentEvent>(16);
    let mut handler = EventHandler::new(agent_rx);

    agent_tx
        .send(AgentEvent::Stream {
            text: "hello".to_string(),
        })
        .await
        .expect("send should succeed");

    let event = tokio::time::timeout(std::time::Duration::from_secs(2), handler.next())
        .await
        .expect("should receive event within timeout")
        .expect("channel should not be closed");

    match event {
        AppEvent::Agent(AgentEvent::Stream { text }) => {
            assert_eq!(text, "hello");
        }
        other => panic!("expected Agent(Stream), got {:?}", other),
    }
}

#[tokio::test]
async fn test_tick_events_arrive() {
    let (_agent_tx, agent_rx) = mpsc::channel::<AgentEvent>(16);
    let mut handler = EventHandler::new(agent_rx);

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut got_tick = false;

    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(std::time::Duration::from_millis(300), handler.next()).await {
            Ok(Some(AppEvent::Tick)) => {
                got_tick = true;
                break;
            }
            Ok(Some(_)) => continue,
            Ok(None) => break,
            Err(_) => continue,
        }
    }

    assert!(got_tick, "should have received at least one Tick event");
}

#[tokio::test]
async fn test_dropping_sender_closes_agent_forwarding() {
    let (agent_tx, agent_rx) = mpsc::channel::<AgentEvent>(16);
    let mut handler = EventHandler::new(agent_rx);

    agent_tx
        .send(AgentEvent::Finished)
        .await
        .expect("send should succeed");
    drop(agent_tx);

    let mut got_finished = false;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(std::time::Duration::from_millis(300), handler.next()).await {
            Ok(Some(AppEvent::Agent(AgentEvent::Finished))) => {
                got_finished = true;
                break;
            }
            Ok(Some(_)) => continue,
            Ok(None) => break,
            Err(_) => continue,
        }
    }

    assert!(got_finished, "should receive the Finished event before channel closes");
}

#[tokio::test]
async fn test_multiple_agent_events_ordering() {
    let (agent_tx, agent_rx) = mpsc::channel::<AgentEvent>(16);
    let mut handler = EventHandler::new(agent_rx);

    agent_tx.send(AgentEvent::Started).await.unwrap();
    agent_tx
        .send(AgentEvent::Stream {
            text: "first".to_string(),
        })
        .await
        .unwrap();
    agent_tx
        .send(AgentEvent::Stream {
            text: "second".to_string(),
        })
        .await
        .unwrap();

    let mut agent_events = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    while agent_events.len() < 3 && tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(std::time::Duration::from_millis(500), handler.next()).await {
            Ok(Some(AppEvent::Agent(e))) => agent_events.push(e),
            Ok(Some(_)) => continue,
            _ => break,
        }
    }

    assert_eq!(agent_events.len(), 3, "should receive all 3 agent events");
    assert!(matches!(agent_events[0], AgentEvent::Started));
    assert!(matches!(&agent_events[1], AgentEvent::Stream { text } if text == "first"));
    assert!(matches!(&agent_events[2], AgentEvent::Stream { text } if text == "second"));
}
