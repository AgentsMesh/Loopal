use std::time::{Duration, SystemTime};

use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_protocol::{AgentEventPayload, CompactPhase};
use loopal_test_support::{HarnessBuilder, chunks};

async fn drain_events(
    rx: &mut tokio::sync::mpsc::Receiver<loopal_protocol::AgentEvent>,
) -> Vec<AgentEventPayload> {
    tokio::task::yield_now().await;
    let mut out = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        out.push(ev.payload);
    }
    out
}

const CLEARED_MARKER: &str = "[Old tool result content cleared after idle timeout]";

fn tool_use(id: &str, name: &str) -> Message {
    Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![ContentBlock::ToolUse {
            id: id.into(),
            name: name.into(),
            input: serde_json::json!({}),
        }],
        origin: None,
    }
}

fn tool_result(id: &str, body: &str) -> Message {
    Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: id.into(),
            content: body.into(),
            is_error: false,
            metadata: None,
        }],
        origin: None,
    }
}

/// Drive `check_and_microcompact` through the runtime: when the
/// configured idle threshold has elapsed since the last assistant
/// activity, ToolResult bodies for scrubbable tools must be replaced by
/// the cleared marker, and a `CompactProgress { phase: Microcompact }`
/// event must fire.
#[tokio::test]
async fn microcompact_scrubs_idle_tool_results_e2e() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;

    h.runner.params.config.microcompact_idle = Duration::from_secs(60);

    h.runner.params.store.clear();
    h.runner.params.store.push_assistant(tool_use("u1", "Read"));
    h.runner
        .params
        .store
        .push_tool_results(tool_result("u1", "file contents A"));
    h.runner.params.store.push_assistant(tool_use("u2", "Bash"));
    h.runner
        .params
        .store
        .push_tool_results(tool_result("u2", "shell output B"));

    let stale = SystemTime::now() - Duration::from_secs(120);
    h.runner.params.store.record_assistant_activity(stale);

    h.runner.check_and_microcompact().await.unwrap();

    let scrubbed: Vec<&str> = h
        .runner
        .params
        .store
        .messages()
        .iter()
        .flat_map(|m| m.content.iter())
        .filter_map(|b| match b {
            ContentBlock::ToolResult { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(scrubbed.len(), 2);
    assert!(
        scrubbed.iter().all(|c| *c == CLEARED_MARKER),
        "all tool results should be scrubbed, got: {scrubbed:?}",
    );

    let evts = drain_events(&mut h.event_rx).await;
    let saw_microcompact = evts.iter().any(|e| {
        matches!(
            e,
            AgentEventPayload::CompactProgress {
                phase: CompactPhase::Microcompact,
                ..
            }
        )
    });
    assert!(
        saw_microcompact,
        "expected CompactProgress(Microcompact) emit, got: {evts:?}",
    );
}

/// Within the idle window: nothing scrubs, no event emits.
#[tokio::test]
async fn microcompact_noop_when_recent_activity_e2e() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;

    h.runner.params.config.microcompact_idle = Duration::from_secs(60);

    h.runner.params.store.clear();
    h.runner.params.store.push_assistant(tool_use("u1", "Read"));
    h.runner
        .params
        .store
        .push_tool_results(tool_result("u1", "stays as-is"));

    h.runner
        .params
        .store
        .record_assistant_activity(SystemTime::now());

    h.runner.check_and_microcompact().await.unwrap();

    let preserved: Vec<&str> = h
        .runner
        .params
        .store
        .messages()
        .iter()
        .flat_map(|m| m.content.iter())
        .filter_map(|b| match b {
            ContentBlock::ToolResult { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(preserved, vec!["stays as-is"]);

    let evts = drain_events(&mut h.event_rx).await;
    let saw_microcompact = evts.iter().any(|e| {
        matches!(
            e,
            AgentEventPayload::CompactProgress {
                phase: CompactPhase::Microcompact,
                ..
            }
        )
    });
    assert!(
        !saw_microcompact,
        "no event should fire inside idle window, got: {evts:?}",
    );
}

/// Non-scrubbable tools (e.g. `Plan`, `AskUser`) keep their bodies even
/// after the idle threshold — they carry low-volume, high-value state.
#[tokio::test]
async fn microcompact_preserves_non_scrubbable_tools_e2e() {
    let mut h = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("noop")])
        .build()
        .await;

    h.runner.params.config.microcompact_idle = Duration::from_secs(60);

    h.runner.params.store.clear();
    h.runner.params.store.push_assistant(tool_use("u1", "Plan"));
    h.runner
        .params
        .store
        .push_tool_results(tool_result("u1", "deep deliberation"));

    let stale = SystemTime::now() - Duration::from_secs(120);
    h.runner.params.store.record_assistant_activity(stale);

    h.runner.check_and_microcompact().await.unwrap();

    let preserved: Vec<&str> = h
        .runner
        .params
        .store
        .messages()
        .iter()
        .flat_map(|m| m.content.iter())
        .filter_map(|b| match b {
            ContentBlock::ToolResult { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(preserved, vec!["deep deliberation"]);
}
