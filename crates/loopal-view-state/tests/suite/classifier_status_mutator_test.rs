use loopal_protocol::{AgentEventPayload, Question, QuestionOption};
use loopal_view_state::ViewStateReducer;
use loopal_view_state::conversation::ClassifierStatus;

fn one_question() -> Question {
    Question {
        question: "Pick".into(),
        options: vec![QuestionOption {
            label: "a".into(),
            description: "".into(),
        }],
        allow_multiple: false,
        header: None,
    }
}

fn open_pending(r: &mut ViewStateReducer, id: &str, auto_running: bool) {
    r.apply(AgentEventPayload::UserQuestionRequest {
        id: id.into(),
        logical_id: id.into(),
        questions: vec![one_question()],
        classifier_running: auto_running,
    });
}

#[test]
fn request_with_auto_running_initializes_status_to_running() {
    let mut r = ViewStateReducer::new("root");
    open_pending(&mut r, "q-1", true);
    let pq = r
        .state()
        .agent
        .conversation
        .pending_question
        .as_ref()
        .expect("pending should exist");
    assert!(pq.classifier_status.is_running());
}

#[test]
fn request_without_auto_running_has_none_status() {
    let mut r = ViewStateReducer::new("root");
    open_pending(&mut r, "q-2", false);
    let pq = r
        .state()
        .agent
        .conversation
        .pending_question
        .as_ref()
        .expect("pending");
    assert!(pq.classifier_status.is_none());
}

#[test]
fn progress_event_updates_elapsed_ms() {
    let mut r = ViewStateReducer::new("root");
    open_pending(&mut r, "q-3", true);
    r.apply(AgentEventPayload::ClassifierProgress {
        id: "q-3".into(),
        elapsed_ms: 1500,
    });
    let pq = r
        .state()
        .agent
        .conversation
        .pending_question
        .as_ref()
        .unwrap();
    match pq.classifier_status {
        ClassifierStatus::Running { elapsed_ms } => assert_eq!(elapsed_ms, 1500),
        ref s => panic!("expected Running, got {s:?}"),
    }
}

#[test]
fn progress_event_for_unknown_id_is_ignored() {
    let mut r = ViewStateReducer::new("root");
    open_pending(&mut r, "q-4", true);
    let bumped = r.apply(AgentEventPayload::ClassifierProgress {
        id: "wrong-id".into(),
        elapsed_ms: 999,
    });
    assert!(bumped.is_none(), "stale id should not bump rev");
}

#[test]
fn failed_event_flips_to_failed_status() {
    let mut r = ViewStateReducer::new("root");
    open_pending(&mut r, "q-5", true);
    r.apply(AgentEventPayload::ClassifierFailed {
        id: "q-5".into(),
        reason: "LLM timeout".into(),
    });
    let pq = r
        .state()
        .agent
        .conversation
        .pending_question
        .as_ref()
        .unwrap();
    match &pq.classifier_status {
        ClassifierStatus::Failed { reason } => assert_eq!(reason, "LLM timeout"),
        s => panic!("expected Failed, got {s:?}"),
    }
}

#[test]
fn progress_after_failed_does_not_revert_status() {
    let mut r = ViewStateReducer::new("root");
    open_pending(&mut r, "q-6", true);
    r.apply(AgentEventPayload::ClassifierFailed {
        id: "q-6".into(),
        reason: "boom".into(),
    });
    // late progress event must NOT overwrite Failed
    let bumped = r.apply(AgentEventPayload::ClassifierProgress {
        id: "q-6".into(),
        elapsed_ms: 9000,
    });
    assert!(bumped.is_none());
    let pq = r
        .state()
        .agent
        .conversation
        .pending_question
        .as_ref()
        .unwrap();
    assert!(pq.classifier_status.is_failed());
}

#[test]
fn completed_event_records_answers() {
    let mut r = ViewStateReducer::new("root");
    open_pending(&mut r, "q-7", true);
    r.apply(AgentEventPayload::ClassifierCompleted {
        id: "q-7".into(),
        answers: vec!["a".into()],
        duration_ms: 800,
    });
    let pq = r
        .state()
        .agent
        .conversation
        .pending_question
        .as_ref()
        .unwrap();
    match &pq.classifier_status {
        ClassifierStatus::Completed { answers } => assert_eq!(answers, &vec!["a".to_string()]),
        s => panic!("expected Completed, got {s:?}"),
    }
}

#[test]
fn none_directly_to_failed_is_allowed() {
    // classifier_running=false → status starts as None. A direct
    // ClassifierFailed event (e.g. classifier provider unavailable at
    // startup) must still flip status to Failed.
    let mut r = ViewStateReducer::new("root");
    open_pending(&mut r, "q-none-fail", false);
    let bumped = r.apply(AgentEventPayload::ClassifierFailed {
        id: "q-none-fail".into(),
        reason: "provider lookup failed".into(),
    });
    assert!(bumped.is_some(), "None → Failed must bump rev");
    let pq = r
        .state()
        .agent
        .conversation
        .pending_question
        .as_ref()
        .unwrap();
    match &pq.classifier_status {
        ClassifierStatus::Failed { reason } => assert!(reason.contains("provider")),
        s => panic!("expected Failed, got {s:?}"),
    }
}

#[test]
fn failed_then_another_failed_keeps_first_reason() {
    // Once Failed, subsequent Failed events are ignored — the first
    // diagnostic is preserved. (Race mode only emits one Failed per ask;
    // this asserts the mutator's contract regardless.)
    let mut r = ViewStateReducer::new("root");
    open_pending(&mut r, "q-double-fail", true);
    r.apply(AgentEventPayload::ClassifierFailed {
        id: "q-double-fail".into(),
        reason: "first diagnostic".into(),
    });
    let bumped = r.apply(AgentEventPayload::ClassifierFailed {
        id: "q-double-fail".into(),
        reason: "second diagnostic".into(),
    });
    assert!(bumped.is_none(), "second Failed must be ignored");
    let pq = r
        .state()
        .agent
        .conversation
        .pending_question
        .as_ref()
        .unwrap();
    match &pq.classifier_status {
        ClassifierStatus::Failed { reason } => {
            assert_eq!(reason, "first diagnostic", "first reason preserved");
        }
        s => panic!("expected Failed, got {s:?}"),
    }
}

#[test]
fn failed_then_completed_keeps_failed() {
    // Terminal status must not be overwritten by an out-of-order event.
    let mut r = ViewStateReducer::new("root");
    open_pending(&mut r, "q-8", true);
    r.apply(AgentEventPayload::ClassifierFailed {
        id: "q-8".into(),
        reason: "timeout".into(),
    });
    let bumped = r.apply(AgentEventPayload::ClassifierCompleted {
        id: "q-8".into(),
        answers: vec!["a".into()],
        duration_ms: 1,
    });
    assert!(
        bumped.is_none(),
        "late Completed after Failed must be rejected"
    );
    assert!(
        r.state()
            .agent
            .conversation
            .pending_question
            .as_ref()
            .unwrap()
            .classifier_status
            .is_failed()
    );
}

#[test]
fn completed_then_failed_keeps_completed() {
    let mut r = ViewStateReducer::new("root");
    open_pending(&mut r, "q-9", true);
    r.apply(AgentEventPayload::ClassifierCompleted {
        id: "q-9".into(),
        answers: vec!["a".into()],
        duration_ms: 1,
    });
    let bumped = r.apply(AgentEventPayload::ClassifierFailed {
        id: "q-9".into(),
        reason: "late failure".into(),
    });
    assert!(bumped.is_none());
    let pq = r
        .state()
        .agent
        .conversation
        .pending_question
        .as_ref()
        .unwrap();
    assert!(matches!(
        pq.classifier_status,
        ClassifierStatus::Completed { .. }
    ));
}
