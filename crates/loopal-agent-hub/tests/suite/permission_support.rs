use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::Hub;
use loopal_protocol::{
    PermissionIntentDigest, PermissionIntentRequest, WorkflowPermissionCausation,
};
use tokio::sync::Mutex;

#[derive(Clone)]
pub(crate) struct PermissionInteraction {
    pub(crate) id: String,
    pub(crate) digest: PermissionIntentDigest,
}

pub(crate) fn permission_request(
    id: &str,
    tool_name: &str,
    input: serde_json::Value,
) -> serde_json::Value {
    permission_request_with(
        id,
        tool_name,
        input,
        serde_json::json!({"type": "object"}),
        None,
    )
}

pub(crate) fn permission_request_with(
    id: &str,
    tool_name: &str,
    input: serde_json::Value,
    schema: serde_json::Value,
    workflow: Option<WorkflowPermissionCausation>,
) -> serde_json::Value {
    serde_json::to_value(
        PermissionIntentRequest::create(id, tool_name, input.clone(), input, schema, workflow)
            .unwrap(),
    )
    .unwrap()
}

pub(crate) fn hub_with_noop_audit(
    events: tokio::sync::mpsc::Sender<loopal_protocol::AgentEvent>,
) -> Arc<Mutex<Hub>> {
    let mut hub = Hub::new(events);
    hub.set_protected_audit(Arc::new(loopal_vault_api::NoopAuditSink));
    Arc::new(Mutex::new(hub))
}

pub(crate) async fn permission_interaction(
    hub: &Arc<Mutex<Hub>>,
    agent: &str,
    logical_id: &str,
) -> PermissionInteraction {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(interaction) = hub
                .lock()
                .await
                .pending_permissions
                .get(&(agent.to_string(), logical_id.to_string()))
                .map(|info| PermissionInteraction {
                    id: info.interaction_id.clone(),
                    digest: info.permission_intent.intent_digest(),
                })
            {
                return interaction;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("permission interaction should become pending")
}

pub(crate) async fn permission_interaction_id(
    hub: &Arc<Mutex<Hub>>,
    agent: &str,
    logical_id: &str,
) -> String {
    permission_interaction(hub, agent, logical_id).await.id
}
