use loopal_protocol::{AgentEventPayload, Envelope, MessageSource, QualifiedAddress};

use super::make_runner_with_channels;

async fn send_and_drain_event(
    mbox_tx: tokio::sync::mpsc::Sender<Envelope>,
    runner: &mut loopal_runtime::agent_loop::AgentLoopRunner,
    event_rx: &mut tokio::sync::mpsc::Receiver<loopal_protocol::AgentEvent>,
    env: Envelope,
) -> AgentEventPayload {
    mbox_tx.send(env).await.unwrap();
    runner.wait_for_input().await.unwrap();
    loop {
        let event = event_rx.recv().await.unwrap();
        if matches!(event.payload, AgentEventPayload::InboxEnqueued { .. }) {
            return event.payload;
        }
    }
}

#[tokio::test]
async fn test_inbox_enqueued_emitted_for_human_message() {
    let (mut runner, mut event_rx, mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();
    let env = Envelope::new(MessageSource::Human, "main", "hi from user");
    let env_id = env.id.to_string();

    let payload = send_and_drain_event(mbox_tx, &mut runner, &mut event_rx, env).await;

    let AgentEventPayload::InboxEnqueued {
        message_id,
        source,
        content,
        summary,
    } = payload
    else {
        panic!("expected InboxEnqueued");
    };
    assert_eq!(message_id, env_id);
    assert_eq!(source, MessageSource::Human);
    assert_eq!(content, "hi from user");
    assert!(summary.is_none());
}

#[tokio::test]
async fn test_inbox_enqueued_emitted_for_agent_source() {
    let (mut runner, mut event_rx, mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();
    let src = MessageSource::Agent(QualifiedAddress::local("worker"));
    let env = Envelope::new(src.clone(), "main", "ping");

    let payload = send_and_drain_event(mbox_tx, &mut runner, &mut event_rx, env).await;
    let AgentEventPayload::InboxEnqueued { source, .. } = payload else {
        panic!("expected InboxEnqueued");
    };
    assert_eq!(source, src);
}

#[tokio::test]
async fn test_inbox_enqueued_emitted_for_scheduled_source() {
    let (mut runner, mut event_rx, mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();
    let env = Envelope::new(MessageSource::Scheduled, "main", "tick");

    let payload = send_and_drain_event(mbox_tx, &mut runner, &mut event_rx, env).await;
    let AgentEventPayload::InboxEnqueued { source, .. } = payload else {
        panic!("expected InboxEnqueued");
    };
    assert_eq!(source, MessageSource::Scheduled);
}

#[tokio::test]
async fn test_inbox_enqueued_emitted_for_system_source() {
    let (mut runner, mut event_rx, mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();
    let env = Envelope::new(
        MessageSource::System("rewake".into()),
        "main",
        "hook signal",
    );

    let payload = send_and_drain_event(mbox_tx, &mut runner, &mut event_rx, env).await;
    let AgentEventPayload::InboxEnqueued { source, .. } = payload else {
        panic!("expected InboxEnqueued");
    };
    assert_eq!(source, MessageSource::System("rewake".into()));
}

#[tokio::test]
async fn test_inbox_enqueued_emitted_for_channel_source() {
    let (mut runner, mut event_rx, mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();
    let src = MessageSource::Channel {
        channel: "general".into(),
        from: QualifiedAddress::local("bot"),
    };
    let env = Envelope::new(src.clone(), "main", "broadcast");

    let payload = send_and_drain_event(mbox_tx, &mut runner, &mut event_rx, env).await;
    let AgentEventPayload::InboxEnqueued { source, .. } = payload else {
        panic!("expected InboxEnqueued");
    };
    assert_eq!(source, src);
}

#[tokio::test]
async fn test_inbox_enqueued_propagates_summary() {
    let (mut runner, mut event_rx, mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();
    let env = Envelope::new(
        MessageSource::Agent(QualifiedAddress::local("a")),
        "main",
        "the long body",
    )
    .with_summary("ping");

    let payload = send_and_drain_event(mbox_tx, &mut runner, &mut event_rx, env).await;
    let AgentEventPayload::InboxEnqueued { summary, .. } = payload else {
        panic!("expected InboxEnqueued");
    };
    assert_eq!(summary.as_deref(), Some("ping"));
}

#[tokio::test]
async fn test_pending_consumed_ids_record_message_ids_for_each_ingest() {
    let (mut runner, _event_rx, mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();
    let env_a = Envelope::new(MessageSource::Human, "main", "first");
    let env_b = Envelope::new(MessageSource::Human, "main", "second");
    let id_a = env_a.id.to_string();
    let id_b = env_b.id.to_string();

    mbox_tx.send(env_a).await.unwrap();
    runner.wait_for_input().await.unwrap();
    mbox_tx.send(env_b).await.unwrap();
    runner.wait_for_input().await.unwrap();

    assert_eq!(runner.pending_consumed_ids, vec![id_a, id_b]);
}

#[tokio::test]
async fn test_inject_pending_messages_emits_inbox_enqueued_for_mid_turn_arrivals() {
    let (mut runner, mut event_rx, mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();
    let src = MessageSource::Agent(QualifiedAddress::local("worker"));
    let env = Envelope::new(src.clone(), "main", "mid-turn");
    mbox_tx.send(env).await.unwrap();
    drop(mbox_tx);

    runner.inject_pending_messages().await;

    let mut found = false;
    while let Ok(event) = event_rx.try_recv() {
        if let AgentEventPayload::InboxEnqueued {
            source, content, ..
        } = event.payload
        {
            assert_eq!(source, src);
            assert_eq!(content, "mid-turn");
            found = true;
            break;
        }
    }
    assert!(
        found,
        "inject_pending_messages must emit InboxEnqueued so UI sees mid-turn arrivals"
    );
}

#[tokio::test]
async fn test_failed_inbox_enqueued_emit_does_not_leave_orphan_consume_id() {
    let (mut runner, event_rx, mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();
    drop(event_rx);

    let env = Envelope::new(MessageSource::Human, "main", "x");
    mbox_tx.send(env).await.unwrap();
    runner.wait_for_input().await.unwrap();

    assert!(
        runner.pending_consumed_ids.is_empty(),
        "emit failure must not leak a message_id into pending_consumed_ids"
    );
}

#[tokio::test]
async fn test_emit_inbox_consumed_drains_all_pending_ids() {
    let (mut runner, mut event_rx, mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();
    let env_a = Envelope::new(MessageSource::Human, "main", "first");
    let env_b = Envelope::new(MessageSource::Human, "main", "second");
    let id_a = env_a.id.to_string();
    let id_b = env_b.id.to_string();

    mbox_tx.send(env_a).await.unwrap();
    runner.wait_for_input().await.unwrap();
    mbox_tx.send(env_b).await.unwrap();
    runner.wait_for_input().await.unwrap();
    assert_eq!(runner.pending_consumed_ids.len(), 2);

    runner.emit_inbox_consumed().await;
    assert!(runner.pending_consumed_ids.is_empty());

    let mut consumed: Vec<String> = Vec::new();
    while let Ok(event) = event_rx.try_recv() {
        if let AgentEventPayload::InboxConsumed { message_id } = event.payload {
            consumed.push(message_id);
        }
    }
    assert_eq!(consumed, vec![id_a, id_b]);
}

/// Drives the production sequence: ingest → emit_inbox_consumed.
/// Asserts the event stream contains InboxEnqueued *before* InboxConsumed
/// for the same message_id, mirroring observer view during a real turn.
#[tokio::test]
async fn test_inbox_enqueued_precedes_inbox_consumed_in_production_sequence() {
    let (mut runner, mut event_rx, mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();
    let env = Envelope::new(
        MessageSource::Agent(QualifiedAddress::local("peer")),
        "main",
        "ping",
    );
    let env_id = env.id.to_string();
    mbox_tx.send(env).await.unwrap();
    runner.wait_for_input().await.unwrap();
    runner.emit_inbox_consumed().await;

    let mut enq_idx: Option<usize> = None;
    let mut consumed_idx: Option<usize> = None;
    let mut idx = 0;
    while let Ok(event) = event_rx.try_recv() {
        match &event.payload {
            AgentEventPayload::InboxEnqueued { message_id, .. } if message_id == &env_id => {
                enq_idx = Some(idx);
            }
            AgentEventPayload::InboxConsumed { message_id } if message_id == &env_id => {
                consumed_idx = Some(idx);
            }
            _ => {}
        }
        idx += 1;
    }
    let enq = enq_idx.expect("InboxEnqueued must be emitted by ingest_message");
    let consumed = consumed_idx.expect("InboxConsumed must be emitted by emit_inbox_consumed");
    assert!(
        enq < consumed,
        "InboxEnqueued must precede InboxConsumed in observer event stream"
    );
}
