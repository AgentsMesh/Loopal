use loopal_protocol::{AgentEventPayload, PermissionIntent, PermissionIntentRequest};
use loopal_view_state::{PendingPermission, PermissionChoice, ViewStateReducer};

fn permission_intent(id: &str) -> PermissionIntent {
    let request = PermissionIntentRequest::create(
        format!("logical-{id}"),
        "Bash",
        serde_json::json!({"command": "ls"}),
        serde_json::json!({"command": "ls"}),
        serde_json::json!({"type": "object"}),
        None,
    )
    .unwrap();
    PermissionIntent::bind(request.intent_seed, 7, 9, id).unwrap()
}

fn request(id: &str, intent: Option<PermissionIntent>) -> AgentEventPayload {
    AgentEventPayload::ToolPermissionRequest {
        id: id.into(),
        name: "Bash".into(),
        input: serde_json::json!({"command": "ls"}),
        permission_intent: intent.map(Box::new),
    }
}

#[test]
fn permission_request_flushes_stream_and_stores_intent_digest() {
    let mut reducer = ViewStateReducer::new("main");
    let intent = permission_intent("permission-1");
    let digest = intent.intent_digest();
    reducer.apply(AgentEventPayload::Stream {
        text: "partial answer".into(),
    });
    reducer.apply(request("permission-1", Some(intent)));

    let conversation = &reducer.state().agent.conversation;
    assert!(conversation.streaming_text.is_empty());
    assert_eq!(conversation.messages[0].content, "partial answer");
    let pending = conversation.pending_permission.as_ref().unwrap();
    assert_eq!(pending.id, "permission-1");
    assert_eq!(pending.intent_digest, Some(digest));
    assert_eq!(pending.cursor, PermissionChoice::Allow);
}

#[test]
fn permission_resolve_requires_matching_id() {
    let mut reducer = ViewStateReducer::new("main");
    reducer.apply(request("permission-1", None));
    let request_rev = reducer.rev();
    let pending = reducer
        .state()
        .agent
        .conversation
        .pending_permission
        .as_ref()
        .unwrap();
    assert_eq!(pending.intent_digest, None);

    assert_eq!(
        reducer.apply(AgentEventPayload::ToolPermissionResolved { id: "other".into() }),
        None
    );
    assert_eq!(reducer.rev(), request_rev);
    assert_eq!(
        reducer
            .state()
            .agent
            .conversation
            .pending_permission
            .as_ref()
            .unwrap()
            .id,
        "permission-1"
    );

    assert_eq!(
        reducer.apply(AgentEventPayload::ToolPermissionResolved {
            id: "permission-1".into(),
        }),
        Some(request_rev + 1)
    );
    assert!(
        reducer
            .state()
            .agent
            .conversation
            .pending_permission
            .is_none()
    );
}

#[test]
fn pending_permission_defaults_and_omits_absent_digest() {
    let pending: PendingPermission = serde_json::from_value(serde_json::json!({
        "id": "legacy",
        "name": "Read",
        "input": {"file_path": "/tmp/input"}
    }))
    .unwrap();
    assert_eq!(pending.intent_digest, None);
    assert_eq!(pending.cursor, PermissionChoice::Allow);
    assert_eq!(PermissionChoice::Allow.toggle(), PermissionChoice::Deny);
    assert_eq!(PermissionChoice::Deny.toggle(), PermissionChoice::Allow);

    let serialized = serde_json::to_value(pending).unwrap();
    assert!(serialized.get("intent_digest").is_none());
}

#[test]
fn permission_settings_project_to_observable_state() {
    let mut reducer = ViewStateReducer::new("main");
    reducer.apply(AgentEventPayload::PermissionModeChanged {
        mode: "ask_any_write".into(),
    });
    reducer.apply(AgentEventPayload::DecisionModeChanged {
        mode: "classifier".into(),
    });
    reducer.apply(AgentEventPayload::SandboxPolicyChanged {
        policy: "workspace_write".into(),
    });

    let observable = &reducer.state().agent.observable;
    assert_eq!(observable.permission_mode, "ask_any_write");
    assert_eq!(observable.decision_mode, "classifier");
    assert_eq!(observable.sandbox_policy, "workspace_write");
}
