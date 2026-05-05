use crate::app::App;

/// Reload session + sub-agent history into ViewClients after the agent
/// confirms a `/resume`. Storage failures are swallowed silently.
pub(crate) fn load_resumed_display(app: &mut App, session_id: &str) {
    let Ok(sm) = loopal_runtime::SessionManager::new() else {
        return;
    };
    let Ok((session, messages)) = sm.resume_session(session_id) else {
        return;
    };
    let projected = loopal_protocol::project_messages(&messages);
    app.load_display_history(projected);

    for sub in &session.sub_agents {
        let Ok(sub_msgs) = sm.load_messages(&sub.session_id) else {
            continue;
        };
        if sub_msgs.is_empty() {
            continue;
        }
        app.load_sub_agent_history(
            &sub.name,
            &sub.session_id,
            sub.parent.as_deref(),
            sub.model.as_deref(),
            loopal_protocol::project_messages(&sub_msgs),
        );
    }
}

pub(crate) fn push_session_warning(app: &mut App, warning: &str) {
    app.push_system_message(format!("Session resume warning: {warning}"));
}
