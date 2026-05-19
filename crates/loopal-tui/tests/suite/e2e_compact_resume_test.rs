use loopal_context::ContextBudget;
use loopal_message::Message;
use loopal_protocol::AgentEventPayload;
use loopal_test_support::{HarnessBuilder, chunks};

fn tiny_budget() -> ContextBudget {
    ContextBudget {
        context_window: 500,
        system_tokens: 0,
        tool_tokens: 0,
        output_reserve: 50,
        safety_margin: 25,
        message_budget: 425,
        max_output_tokens: 50,
    }
}

fn padded_user_msg(label: &str) -> Message {
    Message::user(&format!("{label}: {}", "x".repeat(100)))
}

#[tokio::test]
async fn compact_boundary_survives_resume_via_session_manager() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("compact-summary-body")])
        .build()
        .await;

    h.runner.params.store.update_budget(tiny_budget());
    h.runner.params.store.clear();
    for i in 0..15 {
        let mut msg = padded_user_msg(&format!("msg-{i}"));
        h.runner
            .params
            .deps
            .session_manager
            .save_message(&h.runner.params.session.id, &mut msg)
            .unwrap();
        h.runner.params.store.push_user(msg);
    }
    let session_id = h.runner.params.session.id.clone();

    h.runner.force_compact(None).await.unwrap();

    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;
    let summary_msg_id = evts
        .iter()
        .find_map(|e| match e {
            AgentEventPayload::Compacted(s) => s.summary_msg_id.clone(),
            _ => None,
        })
        .expect("Compacted event must carry summary_msg_id");

    let mgr = h.fixture.session_manager();
    let messages = mgr.load_messages(&session_id).unwrap();

    assert!(
        !messages.is_empty(),
        "resume must produce at least the summary message"
    );
    let first = &messages[0];
    assert_eq!(
        first.id.as_deref(),
        Some(summary_msg_id.as_str()),
        "first message after resume must be the summary anchor",
    );
    assert!(
        messages.len() < 15,
        "boundary marker must have dropped original prefix (got {} messages)",
        messages.len()
    );
}

#[tokio::test]
async fn bare_summary_boundary_also_survives_resume() {
    let mut h = HarnessBuilder::new()
        .calls(vec![vec![chunks::non_retryable_error("forced fallback")]])
        .build()
        .await;

    h.runner.params.store.update_budget(tiny_budget());
    h.runner.params.store.clear();
    for i in 0..10 {
        let mut msg = padded_user_msg(&format!("msg-{i}"));
        h.runner
            .params
            .deps
            .session_manager
            .save_message(&h.runner.params.session.id, &mut msg)
            .unwrap();
        h.runner.params.store.push_user(msg);
    }
    let session_id = h.runner.params.session.id.clone();

    h.runner.force_compact(None).await.unwrap();

    let evts = loopal_test_support::events::drain_pending(&mut h.event_rx).await;
    let summary_msg_id = evts
        .iter()
        .find_map(|e| match e {
            AgentEventPayload::Compacted(s) => s.summary_msg_id.clone(),
            _ => None,
        })
        .expect("bare_summary path must still write boundary marker");

    let mgr = h.fixture.session_manager();
    let messages = mgr.load_messages(&session_id).unwrap();

    let first = messages.first().expect("at least summary must remain");
    assert_eq!(
        first.id.as_deref(),
        Some(summary_msg_id.as_str()),
        "bare_summary anchor must be the first message after resume",
    );
    assert!(messages.len() < 10);
}
