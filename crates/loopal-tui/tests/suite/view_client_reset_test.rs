use loopal_tui::view_client::ViewClient;
use loopal_view_state::{SessionMessage, SessionViewState, ViewSnapshot};

fn message(role: &str, content: &str, ui_local: bool) -> SessionMessage {
    SessionMessage {
        role: role.into(),
        content: content.into(),
        ui_local,
        ..Default::default()
    }
}

fn snapshot_with_messages(agent: &str, messages: Vec<SessionMessage>, rev: u64) -> ViewSnapshot {
    let mut state = SessionViewState::empty(agent);
    state.agent.conversation.messages = messages;
    ViewSnapshot { rev, state }
}

#[test]
fn reset_to_snapshot_preserves_ui_local_messages() {
    let vc = ViewClient::empty("main");
    vc.with_conversation_mut(|conv| {
        conv.messages = vec![
            message("welcome", "banner", true),
            message("user", "hi", false),
            message("system", "tip", true),
            message("assistant", "hello", false),
        ];
    });

    let hub_msgs = vec![
        message("user", "hi", false),
        message("assistant", "hello", false),
    ];
    let snap = snapshot_with_messages("main", hub_msgs, 5);
    vc.reset_to_snapshot(snap);

    let guard = vc.state();
    let msgs = &guard.conversation().messages;
    assert_eq!(msgs.len(), 4, "ui-local rows must rejoin Hub rows");
    assert_eq!(msgs[0].role, "welcome");
    assert!(msgs[0].ui_local);
    assert_eq!(msgs[1].role, "user");
    assert_eq!(msgs[2].role, "system");
    assert!(msgs[2].ui_local);
    assert_eq!(msgs[3].role, "assistant");
}

#[test]
fn reset_to_snapshot_drops_stale_hub_messages() {
    let vc = ViewClient::empty("main");
    vc.with_conversation_mut(|conv| {
        conv.messages = vec![
            message("user", "old-1", false),
            message("assistant", "old-2", false),
        ];
    });

    let snap = snapshot_with_messages("main", vec![message("user", "fresh", false)], 9);
    vc.reset_to_snapshot(snap);

    let guard = vc.state();
    let msgs = &guard.conversation().messages;
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].content, "fresh");
}

#[test]
fn reset_to_snapshot_updates_rev() {
    let vc = ViewClient::empty("main");
    let snap = snapshot_with_messages("main", vec![], 42);
    vc.reset_to_snapshot(snap);
    assert_eq!(vc.rev(), 42);
}

#[test]
fn reset_to_snapshot_with_only_ui_local_messages_keeps_them() {
    let vc = ViewClient::empty("main");
    vc.with_conversation_mut(|conv| {
        conv.messages = vec![
            message("welcome", "banner", true),
            message("system", "ready", true),
        ];
    });

    let snap = snapshot_with_messages("main", vec![], 1);
    vc.reset_to_snapshot(snap);

    let guard = vc.state();
    let msgs = &guard.conversation().messages;
    assert_eq!(msgs.len(), 2);
    assert!(msgs.iter().all(|m| m.ui_local));
}

#[test]
fn reset_to_snapshot_replaces_observable_state() {
    let vc = ViewClient::empty("main");
    vc.with_view_mut(|view| {
        view.observable.status = loopal_protocol::AgentStatus::Running;
    });

    let mut state = SessionViewState::empty("main");
    state.agent.observable.status = loopal_protocol::AgentStatus::Finished;
    let snap = ViewSnapshot { rev: 7, state };
    vc.reset_to_snapshot(snap);

    let guard = vc.state();
    assert_eq!(
        guard.state().agent.observable.status,
        loopal_protocol::AgentStatus::Finished
    );
    assert_eq!(vc.rev(), 7);
}
