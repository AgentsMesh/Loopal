use loopal_protocol::{AgentEventPayload, AgentStatus, CompactPhase, CompactionSummary};
use loopal_view_state::ViewStateReducer;

fn progress(phase: CompactPhase, detail: Option<&str>) -> AgentEventPayload {
    AgentEventPayload::CompactProgress {
        phase,
        detail: detail.map(String::from),
    }
}

fn banner_of(r: &ViewStateReducer) -> Option<String> {
    r.state().agent.conversation.compact_banner.clone()
}

#[test]
fn microcompact_phase_sets_banner_text() {
    let mut r = ViewStateReducer::new("root");
    r.apply(progress(CompactPhase::Microcompact, None));
    let b = banner_of(&r).expect("banner must be set");
    assert!(
        b.contains("microcompact"),
        "banner missing phase label: {b:?}",
    );
}

#[test]
fn summarize_phase_sets_banner_text() {
    let mut r = ViewStateReducer::new("root");
    r.apply(progress(CompactPhase::Summarize, None));
    let b = banner_of(&r).expect("banner must be set");
    assert!(b.contains("summariz"), "banner missing phase label: {b:?}");
}

#[test]
fn rehydrate_phase_sets_banner_text() {
    let mut r = ViewStateReducer::new("root");
    r.apply(progress(CompactPhase::Rehydrate, None));
    let b = banner_of(&r).expect("banner must be set");
    assert!(b.contains("rehydrat"), "banner missing phase label: {b:?}");
}

#[test]
fn done_phase_clears_banner() {
    let mut r = ViewStateReducer::new("root");
    r.apply(progress(CompactPhase::Summarize, None));
    assert!(banner_of(&r).is_some());
    r.apply(progress(CompactPhase::Done, None));
    assert_eq!(banner_of(&r), None, "Done must clear banner");
}

#[test]
fn done_phase_clears_nested_retry_banner() {
    let mut r = ViewStateReducer::new("root");
    r.apply(progress(CompactPhase::Summarize, None));
    r.apply(AgentEventPayload::RetryError {
        message: "gateway unavailable".into(),
        attempt: 1,
        max_attempts: 3,
    });
    assert!(r.state().agent.conversation.retry_banner.is_some());

    r.apply(progress(CompactPhase::Done, None));

    assert_eq!(r.state().agent.conversation.retry_banner, None);
}

#[test]
fn completed_microcompact_lifecycle_does_not_leave_a_banner() {
    let mut r = ViewStateReducer::new("root");
    r.apply(progress(
        CompactPhase::Microcompact,
        Some("scrubbed 46 stale tool results"),
    ));
    r.apply(progress(CompactPhase::Done, None));

    assert_eq!(
        banner_of(&r),
        None,
        "a completed microcompact lifecycle must not mask subsequent working status",
    );
}

#[test]
fn normal_turn_activity_repairs_a_lost_compact_terminal_event() {
    let mut r = ViewStateReducer::new("root");
    r.apply(progress(
        CompactPhase::Microcompact,
        Some("scrubbed 46 stale tool results"),
    ));
    r.apply(AgentEventPayload::RetryError {
        message: "gateway unavailable".into(),
        attempt: 1,
        max_attempts: 3,
    });
    assert!(banner_of(&r).is_some());
    assert!(r.state().agent.conversation.retry_banner.is_some());

    r.apply(AgentEventPayload::Stream {
        text: "normal model output".to_string(),
    });

    assert_eq!(
        banner_of(&r),
        None,
        "normal turn activity must supersede a stale compact lifecycle",
    );
    assert_eq!(r.state().agent.conversation.retry_banner, None);
}

#[test]
fn new_running_phase_repairs_a_stale_compact_snapshot() {
    let mut r = ViewStateReducer::new("root");
    r.apply(progress(CompactPhase::Summarize, None));
    r.apply(AgentEventPayload::RetryError {
        message: "gateway unavailable".into(),
        attempt: 1,
        max_attempts: 3,
    });
    assert!(banner_of(&r).is_some());

    r.apply(AgentEventPayload::Running);

    assert_eq!(
        banner_of(&r),
        None,
        "a new running phase must not inherit an older compact banner",
    );
    assert_eq!(r.state().agent.conversation.retry_banner, None);
}

#[test]
fn detail_is_appended_to_banner() {
    let mut r = ViewStateReducer::new("root");
    r.apply(progress(CompactPhase::Rehydrate, Some("3 files, 4.2K")));
    let b = banner_of(&r).expect("banner must be set");
    assert!(b.contains("3 files, 4.2K"), "banner missing detail: {b:?}");
}

#[test]
fn empty_detail_does_not_append_separator() {
    let mut r = ViewStateReducer::new("root");
    r.apply(progress(CompactPhase::Summarize, Some("")));
    let b = banner_of(&r).expect("banner must be set");
    assert!(!b.contains("—"), "empty detail must skip dash: {b:?}");
}

#[test]
fn compacted_event_clears_banner() {
    let mut r = ViewStateReducer::new("root");
    r.apply(progress(CompactPhase::Summarize, None));
    r.apply(AgentEventPayload::RetryError {
        message: "gateway unavailable".into(),
        attempt: 1,
        max_attempts: 3,
    });
    assert!(banner_of(&r).is_some());
    assert!(r.state().agent.conversation.retry_banner.is_some());
    r.apply(AgentEventPayload::Compacted(CompactionSummary {
        kept: 5,
        summarized: 100,
        tokens_before: 50_000,
        tokens_after: 5_000,
        strategy: "smart".into(),
        summary_msg_id: None,
        files_rehydrated: 0,
    }));
    assert_eq!(
        banner_of(&r),
        None,
        "Compacted event must clear stale banner",
    );
    assert_eq!(r.state().agent.conversation.retry_banner, None);
}

#[test]
fn awaiting_input_clears_stale_banner() {
    let mut r = ViewStateReducer::new("root");
    r.apply(progress(CompactPhase::Summarize, None));
    assert!(banner_of(&r).is_some());
    r.apply(AgentEventPayload::AwaitingInput);
    assert_eq!(banner_of(&r), None, "idle transition must clear banner");
}

#[test]
fn error_clears_stale_banner() {
    let mut r = ViewStateReducer::new("root");
    r.apply(progress(CompactPhase::Rehydrate, None));
    r.apply(AgentEventPayload::Error {
        message: "boom".into(),
    });
    assert_eq!(banner_of(&r), None, "error transition must clear banner");
}

#[test]
fn phase_transitions_replace_previous_banner() {
    let mut r = ViewStateReducer::new("root");
    r.apply(progress(CompactPhase::Microcompact, None));
    let first = banner_of(&r).unwrap();
    r.apply(progress(CompactPhase::Summarize, None));
    let second = banner_of(&r).unwrap();
    assert_ne!(first, second, "transition must replace banner text");
}

#[test]
fn compacted_event_refreshes_ctx() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::Compacted(CompactionSummary {
        kept: 9,
        summarized: 491,
        tokens_before: 259_392,
        tokens_after: 6_453,
        strategy: "manual".into(),
        summary_msg_id: None,
        files_rehydrated: 5,
    }));
    assert_eq!(
        r.state().agent.conversation.token_count(),
        6_453,
        "Compacted event must refresh ctx token count to tokens_after",
    );
}

#[test]
fn compact_progress_does_not_touch_status() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::AwaitingInput);
    let before = r.state().agent.observable.status;
    assert_eq!(before, AgentStatus::WaitingForInput);

    r.apply(progress(CompactPhase::Summarize, None));
    r.apply(progress(CompactPhase::Rehydrate, Some("5 files")));
    r.apply(progress(CompactPhase::Done, None));

    assert_eq!(
        r.state().agent.observable.status,
        AgentStatus::WaitingForInput,
        "compaction progress must not mutate backend-authoritative status",
    );
}

#[test]
fn idle_compaction_retry_does_not_turn_waiting_session_into_running() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::AwaitingInput);
    r.apply(progress(CompactPhase::Summarize, None));
    r.apply(AgentEventPayload::RetryError {
        message: "HTTP 502. Retrying".into(),
        attempt: 1,
        max_attempts: 3,
    });
    r.apply(AgentEventPayload::RetryCleared);
    r.apply(progress(CompactPhase::Done, None));

    assert_eq!(
        r.state().agent.observable.status,
        AgentStatus::WaitingForInput,
        "retry lifecycle must preserve the idle compaction owner status",
    );
    assert_eq!(r.state().agent.conversation.retry_banner, None);
    assert_eq!(r.state().agent.conversation.compact_banner, None);
}

#[test]
fn main_model_retry_preserves_running_status() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::Running);
    r.apply(AgentEventPayload::RetryError {
        message: "HTTP 502. Retrying".into(),
        attempt: 1,
        max_attempts: 6,
    });
    r.apply(AgentEventPayload::RetryCleared);

    assert_eq!(r.state().agent.observable.status, AgentStatus::Running);
    assert_eq!(r.state().agent.conversation.retry_banner, None);
}
