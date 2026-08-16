use std::sync::Arc;

use loopal_protocol::{
    PermissionActionDigest, PermissionSchemaDigest, ProtectedEffectAuditRequest,
};
use loopal_secret_client::SecretClient;
use loopal_tool_api::{
    FetchRefinerPolicy, FileReadTracker, GoalSession, MemoryChannel, OneShotChatService,
    OutputTail, ProtectedEffectAudit,
};

use crate::tool_context_support::{Chat, EffectAudit, Goals, Memory, Policy, Secrets, context};

#[test]
fn new_context_starts_without_optional_services_and_is_redacted_in_debug() {
    let context = context();
    assert_eq!(context.session_id, "session");
    assert!(context.shared.is_none());
    assert!(context.memory_channel.is_none());
    assert!(context.output_tail.is_none());
    assert!(context.one_shot_chat.is_none());
    assert!(context.fetch_refiner_policy.is_none());
    assert!(context.goal_session.is_none());
    assert!(context.protected_effect_audit.is_none());
    assert!(context.secret_client.is_none());
    assert!(context.read_tracker.is_none());
    let debug = format!("{context:?}");
    assert!(debug.contains("session"));
    assert!(debug.contains("shared: false"));
}

#[test]
fn setters_attach_all_services_and_clone_preserves_arcs() {
    let shared: Arc<dyn std::any::Any + Send + Sync> = Arc::new(7_u32);
    let memory: Arc<dyn MemoryChannel> = Arc::new(Memory);
    let tail = Arc::new(OutputTail::new(2));
    let chat: Arc<dyn OneShotChatService> = Arc::new(Chat);
    let policy: Arc<dyn FetchRefinerPolicy> = Arc::new(Policy);
    let goals: Arc<dyn GoalSession> = Arc::new(Goals);
    let audit: Arc<dyn ProtectedEffectAudit> = Arc::new(EffectAudit);
    let secrets: Arc<dyn SecretClient> = Arc::new(Secrets);
    let tracker = Arc::new(FileReadTracker::new());
    let context = context()
        .with_shared(shared.clone())
        .with_memory_channel(memory.clone())
        .with_output_tail(tail.clone())
        .with_one_shot_chat(chat.clone())
        .with_fetch_refiner_policy(policy.clone())
        .with_goal_session(goals.clone())
        .with_protected_effect_audit(audit.clone())
        .with_secret_client(secrets.clone())
        .with_read_tracker(tracker.clone());
    let cloned = context.clone();

    assert!(Arc::ptr_eq(context.shared.as_ref().unwrap(), &shared));
    assert!(Arc::ptr_eq(
        context.memory_channel.as_ref().unwrap(),
        &memory
    ));
    assert!(Arc::ptr_eq(context.output_tail.as_ref().unwrap(), &tail));
    assert!(Arc::ptr_eq(context.one_shot_chat.as_ref().unwrap(), &chat));
    assert!(Arc::ptr_eq(
        context.fetch_refiner_policy.as_ref().unwrap(),
        &policy
    ));
    assert!(Arc::ptr_eq(context.goal_session.as_ref().unwrap(), &goals));
    assert!(Arc::ptr_eq(
        context.protected_effect_audit.as_ref().unwrap(),
        &audit
    ));
    assert!(Arc::ptr_eq(
        context.secret_client.as_ref().unwrap(),
        &secrets
    ));
    assert!(Arc::ptr_eq(
        context.read_tracker.as_ref().unwrap(),
        &tracker
    ));
    assert!(Arc::ptr_eq(&context.backend, &cloned.backend));
    assert!(Arc::ptr_eq(
        context.protected_effect_audit.as_ref().unwrap(),
        cloned.protected_effect_audit.as_ref().unwrap()
    ));
}

#[test]
fn optional_setters_accept_some_and_none() {
    let context = context()
        .with_shared_opt(Some(Arc::new(1_u8)))
        .with_memory_channel_opt(Some(Arc::new(Memory)))
        .with_one_shot_chat_opt(Some(Arc::new(Chat)))
        .with_fetch_refiner_policy_opt(Some(Arc::new(Policy)))
        .with_goal_session_opt(Some(Arc::new(Goals)))
        .with_secret_client_opt(Some(Arc::new(Secrets)));
    assert!(context.shared.is_some());
    assert!(context.memory_channel.is_some());
    assert!(context.one_shot_chat.is_some());
    assert!(context.fetch_refiner_policy.is_some());
    assert!(context.goal_session.is_some());
    assert!(context.secret_client.is_some());

    let context = context
        .with_shared_opt(None)
        .with_memory_channel_opt(None)
        .with_one_shot_chat_opt(None)
        .with_fetch_refiner_policy_opt(None)
        .with_goal_session_opt(None)
        .with_secret_client_opt(None);
    for absent in [
        context.shared.is_none(),
        context.memory_channel.is_none(),
        context.one_shot_chat.is_none(),
        context.fetch_refiner_policy.is_none(),
        context.goal_session.is_none(),
        context.secret_client.is_none(),
    ] {
        assert!(absent);
    }
}

#[tokio::test]
async fn protected_effect_audit_setter_exposes_working_sink() {
    let context = context().with_protected_effect_audit(Arc::new(EffectAudit));
    let request = ProtectedEffectAuditRequest::new(
        "call",
        "tool",
        PermissionActionDigest::from_bytes([1; 32]),
        PermissionSchemaDigest::from_bytes([2; 32]),
    )
    .unwrap();
    context
        .protected_effect_audit
        .unwrap()
        .record(&request)
        .await
        .unwrap();
}
