//! Unit tests for `translate::translate_event` — the dispatch table that
//! converts `AgentEventPayload` to `AcpNotification`. Each variant must
//! land in exactly one of: SessionUpdate, Extension, or None.

use loopal_acp::translate::{AcpNotification, translate_event};
use loopal_protocol::AgentEventPayload;

#[test]
fn stream_returns_session_update() {
    let r = translate_event(&AgentEventPayload::Stream { text: "hi".into() }, "s");
    assert!(matches!(r, Some(AcpNotification::SessionUpdate(_))));
}

#[test]
fn thinking_returns_session_update() {
    let r = translate_event(&AgentEventPayload::ThinkingStream { text: "t".into() }, "s");
    assert!(matches!(r, Some(AcpNotification::SessionUpdate(_))));
}

#[test]
fn retry_error_returns_extension() {
    let r = translate_event(
        &AgentEventPayload::RetryError {
            message: "e".into(),
            attempt: 1,
            max_attempts: 3,
        },
        "s",
    );
    assert!(matches!(r, Some(AcpNotification::Extension { .. })));
}

#[test]
fn session_resume_warnings_returns_extension() {
    let r = translate_event(
        &AgentEventPayload::SessionResumeWarnings {
            session_id: "s1".into(),
            warnings: vec!["cron load failed".into()],
        },
        "s1",
    );
    match r {
        Some(AcpNotification::Extension { method, params }) => {
            assert_eq!(method, "_loopal/sessionResumeWarnings");
            assert_eq!(params["sessionId"], "s1");
            assert_eq!(params["data"]["warnings"][0], "cron load failed");
        }
        _ => panic!("expected Extension notification"),
    }
}

#[test]
fn session_resumed_returns_extension() {
    let r = translate_event(
        &AgentEventPayload::SessionResumed {
            session_id: "s2".into(),
            message_count: 7,
        },
        "s2",
    );
    match r {
        Some(AcpNotification::Extension { method, params }) => {
            assert_eq!(method, "_loopal/sessionResumed");
            assert_eq!(params["sessionId"], "s2");
            assert_eq!(params["data"]["messageCount"], 7);
        }
        _ => panic!("expected Extension notification"),
    }
}

#[test]
fn none_events_return_none() {
    let nones = vec![
        AgentEventPayload::AwaitingInput,
        AgentEventPayload::Started,
        AgentEventPayload::Running,
        AgentEventPayload::Finished,
        AgentEventPayload::Interrupted,
        AgentEventPayload::RetryCleared,
        AgentEventPayload::McpStatusReport { servers: vec![] },
    ];
    for ev in &nones {
        assert!(
            translate_event(ev, "s").is_none(),
            "expected None for {ev:?}"
        );
    }
}
