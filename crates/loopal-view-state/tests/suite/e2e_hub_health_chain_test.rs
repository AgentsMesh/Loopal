use loopal_protocol::AgentEventPayload;
use loopal_view_state::ViewStateReducer;

#[test]
fn hub_degraded_sets_since_ms_on_view_state() {
    let mut r = ViewStateReducer::new("root");
    assert_eq!(r.state().hub_degraded_since_ms, None);

    r.apply(AgentEventPayload::HubDegraded {
        since_unix_ms: 1_700_000_000_000,
    });

    assert_eq!(r.state().hub_degraded_since_ms, Some(1_700_000_000_000));
}

#[test]
fn hub_recovered_clears_since_ms() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::HubDegraded {
        since_unix_ms: 1_700_000_000_000,
    });
    assert!(r.state().hub_degraded_since_ms.is_some());

    r.apply(AgentEventPayload::HubRecovered { duration_ms: 5_000 });

    assert_eq!(r.state().hub_degraded_since_ms, None);
}

#[test]
fn duplicate_degraded_emit_is_idempotent() {
    let mut r = ViewStateReducer::new("root");
    let first_ms = 1_700_000_000_000;
    r.apply(AgentEventPayload::HubDegraded {
        since_unix_ms: first_ms,
    });
    r.apply(AgentEventPayload::HubDegraded {
        since_unix_ms: first_ms,
    });
    assert_eq!(r.state().hub_degraded_since_ms, Some(first_ms));
}

#[test]
fn different_since_ms_overwrites_field() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::HubDegraded {
        since_unix_ms: 1_700_000_000_000,
    });
    r.apply(AgentEventPayload::HubDegraded {
        since_unix_ms: 1_700_000_010_000,
    });
    assert_eq!(r.state().hub_degraded_since_ms, Some(1_700_000_010_000));
}

#[test]
fn recovered_when_not_degraded_is_noop_safe() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::HubRecovered { duration_ms: 0 });
    assert_eq!(r.state().hub_degraded_since_ms, None);
}

#[test]
fn session_resumed_clears_stale_degraded_marker() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::HubDegraded {
        since_unix_ms: 1_700_000_000_000,
    });
    assert!(r.state().hub_degraded_since_ms.is_some());

    r.apply(AgentEventPayload::SessionResumed {
        session_id: "new-session-id".into(),
        message_count: 0,
    });

    assert_eq!(r.state().hub_degraded_since_ms, None);
}

#[test]
fn full_chain_degraded_then_recovered_with_age() {
    let mut r = ViewStateReducer::new("root");
    let since_ms: u64 = 1_700_000_000_000;
    r.apply(AgentEventPayload::HubDegraded {
        since_unix_ms: since_ms,
    });

    // Same since_ms during the streak must not bump the timestamp — TUI's
    // age = now - since_ms would jitter if every emit reset it.
    r.apply(AgentEventPayload::HubDegraded {
        since_unix_ms: since_ms,
    });
    assert_eq!(r.state().hub_degraded_since_ms, Some(since_ms));

    r.apply(AgentEventPayload::HubRecovered {
        duration_ms: 12_000,
    });
    assert_eq!(r.state().hub_degraded_since_ms, None);
}
