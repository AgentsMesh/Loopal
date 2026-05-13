use loopal_protocol::{ProjectedMessage, ProjectedToolCall};
use loopal_session::into_session_message;
use loopal_view_state::{
    CancelCause, FailureKind, InvocationState, Outcome, StaleReason, ToolResultMetadata,
};

fn projected_tool(id: &str, result: Option<&str>, is_error: bool) -> ProjectedToolCall {
    ProjectedToolCall {
        id: id.to_string(),
        name: "Bash".to_string(),
        summary: "Bash(ls)".to_string(),
        result: result.map(String::from),
        is_error,
        input: Some(serde_json::json!({"command": "ls"})),
        metadata: None,
    }
}

fn projected_msg(tool_calls: Vec<ProjectedToolCall>) -> ProjectedMessage {
    ProjectedMessage {
        role: "assistant".to_string(),
        content: "hi".to_string(),
        tool_calls,
        image_count: 0,
    }
}

#[test]
fn success_result_maps_to_done_success() {
    let p = projected_msg(vec![projected_tool("a", Some("output"), false)]);
    let s = into_session_message(p);
    assert_eq!(s.tool_calls.len(), 1);
    let tc = &s.tool_calls[0];
    let InvocationState::Done { outcome, .. } = &tc.state else {
        panic!("expected Done")
    };
    let Outcome::Success { content, .. } = outcome else {
        panic!("expected Success")
    };
    assert_eq!(content, "output");
}

#[test]
fn error_result_maps_to_done_failure() {
    let p = projected_msg(vec![projected_tool("b", Some("boom"), true)]);
    let s = into_session_message(p);
    let tc = &s.tool_calls[0];
    let InvocationState::Done { outcome, .. } = &tc.state else {
        panic!("expected Done")
    };
    let Outcome::Failure { error, kind } = outcome else {
        panic!("expected Failure")
    };
    assert_eq!(error, "boom");
    assert_eq!(*kind, FailureKind::ToolError);
}

#[test]
fn no_result_maps_to_stale_connection_lost() {
    let p = projected_msg(vec![projected_tool("c", None, false)]);
    let s = into_session_message(p);
    let tc = &s.tool_calls[0];
    let InvocationState::Stale { reason, .. } = &tc.state else {
        panic!("expected Stale, got {:?}", tc.state.variant_name())
    };
    assert_eq!(*reason, StaleReason::ConnectionLost);
}

#[test]
fn empty_id_invocation_filtered_out() {
    let p = projected_msg(vec![projected_tool("", Some("ignored"), false)]);
    let s = into_session_message(p);
    assert!(s.tool_calls.is_empty());
}

#[test]
fn preserves_role_and_content() {
    let p = projected_msg(vec![]);
    let s = into_session_message(p);
    assert_eq!(s.role, "assistant");
    assert_eq!(s.content, "hi");
}

#[test]
fn preserves_metadata_when_present() {
    let mut tc = projected_tool("d", Some("x"), false);
    tc.metadata = Some(ToolResultMetadata::bytes_written(42));
    let p = projected_msg(vec![tc]);
    let s = into_session_message(p);
    assert_eq!(
        s.tool_calls[0].metadata.as_ref(),
        Some(&ToolResultMetadata::bytes_written(42))
    );
}

#[test]
fn preserves_input_for_rendering() {
    let p = projected_msg(vec![projected_tool("e", Some("x"), false)]);
    let s = into_session_message(p);
    let input = s.tool_calls[0].input.as_ref().expect("input preserved");
    assert_eq!(input["command"], "ls");
}

#[test]
fn multiple_tool_calls_in_order() {
    let p = projected_msg(vec![
        projected_tool("a", Some("ok"), false),
        projected_tool("b", Some("bad"), true),
        projected_tool("c", None, false),
    ]);
    let s = into_session_message(p);
    assert_eq!(s.tool_calls.len(), 3);
    assert_eq!(s.tool_calls[0].id.as_str(), "a");
    assert_eq!(s.tool_calls[1].id.as_str(), "b");
    assert_eq!(s.tool_calls[2].id.as_str(), "c");
}

#[test]
fn ui_local_flag_is_false_by_default() {
    let p = projected_msg(vec![]);
    let s = into_session_message(p);
    assert!(!s.ui_local);
}

#[test]
fn watchdog_stale_metadata_restored_as_stale() {
    let mut tc = projected_tool("a", Some("timed out"), true);
    tc.metadata = Some(ToolResultMetadata::stale(StaleReason::WatchdogTimeout));
    let s = into_session_message(projected_msg(vec![tc]));
    let InvocationState::Stale { reason, .. } = &s.tool_calls[0].state else {
        panic!("expected Stale on restore")
    };
    assert_eq!(*reason, StaleReason::WatchdogTimeout);
}

#[test]
fn turn_ended_metadata_restored_as_stale() {
    let mut tc = projected_tool("b", Some("x"), true);
    tc.metadata = Some(ToolResultMetadata::stale(StaleReason::TurnEnded));
    let s = into_session_message(projected_msg(vec![tc]));
    assert!(matches!(
        s.tool_calls[0].state,
        InvocationState::Stale { .. }
    ));
}

#[test]
fn user_interrupt_metadata_restored_as_cancelled() {
    let mut tc = projected_tool("ci", Some("Interrupted by user"), true);
    tc.metadata = Some(ToolResultMetadata::cancelled(CancelCause::UserInterrupt));
    let s = into_session_message(projected_msg(vec![tc]));
    let InvocationState::Cancelled { cause, .. } = &s.tool_calls[0].state else {
        panic!("expected Cancelled on restore");
    };
    assert_eq!(*cause, CancelCause::UserInterrupt);
}

#[test]
fn parent_cancelled_metadata_restored_as_cancelled() {
    let mut tc = projected_tool("cp", Some("x"), true);
    tc.metadata = Some(ToolResultMetadata::cancelled(CancelCause::ParentCancelled));
    let s = into_session_message(projected_msg(vec![tc]));
    assert!(matches!(
        s.tool_calls[0].state,
        InvocationState::Cancelled { .. }
    ));
}

#[test]
fn bytes_written_metadata_falls_back_to_done() {
    let mut tc = projected_tool("c", Some("ok"), false);
    tc.metadata = Some(ToolResultMetadata::bytes_written(100));
    let s = into_session_message(projected_msg(vec![tc]));
    assert!(matches!(
        s.tool_calls[0].state,
        InvocationState::Done { .. }
    ));
}
