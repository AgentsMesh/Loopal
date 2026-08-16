use std::sync::Arc;

use loopal_protocol::{
    AgentEvent, AgentEventPayload, PermissionAuditDecision, PermissionAuditSource,
    PermissionDecisionAuditRequest, PermissionIntentDigest, QualifiedAddress,
};
use tokio::sync::Mutex;
use tracing::{info, warn};

use super::completion::{TerminalEventSink, complete_detached};
use crate::hub::Hub;

pub(crate) async fn resolve_permission(
    hub: &Arc<Mutex<Hub>>,
    agent_name: &str,
    interaction_id: &str,
    allow: bool,
    remember_session: bool,
    intent_digest: Option<PermissionIntentDigest>,
    ui: &crate::request_principal::UiPrincipal,
) -> bool {
    let (info, authorized, terminal_sink) = {
        let mut locked = hub.lock().await;
        let key = locked
            .pending_permissions
            .iter()
            .find(|((agent, _), info)| agent == agent_name && info.interaction_id == interaction_id)
            .map(|(key, _)| key.clone());
        let info = key.and_then(|key| locked.pending_permissions.remove(&key));
        let authorized = info.as_ref().is_some_and(|info| {
            active_agent(&locked, info)
                && ui.is_current_permission_ui(&locked)
                && locked.ui.capability_snapshot().generation
                    == info.permission_intent.ui_generation()
                && intent_digest == Some(info.permission_intent.intent_digest())
        });
        (info, authorized, TerminalEventSink::from_hub(&locked))
    };
    let Some(info) = info else {
        return false;
    };
    let decision = if allow && authorized {
        PermissionAuditDecision::Allow
    } else {
        PermissionAuditDecision::Deny
    };
    let request = PermissionDecisionAuditRequest::from_seed(
        &info.logical_id,
        info.permission_intent.seed(),
        Some(info.permission_intent.intent_digest()),
        decision,
        PermissionAuditSource::Ui,
    );
    let audit_ok = match request {
        Ok(request) if authorized => super::permission_audit::record_for_execution(
            hub,
            &info.execution,
            &info.agent_conn,
            &request,
        )
        .await
        .map(|_| true)
        .unwrap_or_else(|error| {
            warn!(agent = %info.agent_name, %error, "permission decision audit failed");
            false
        }),
        _ => false,
    };
    let (effective_allow, receipt) = if allow && authorized && audit_ok {
        let mut locked = hub.lock().await;
        let still_authorized = active_agent(&locked, &info)
            && ui.is_current_permission_ui(&locked)
            && locked.ui.capability_snapshot().generation == info.permission_intent.ui_generation();
        let receipt = still_authorized
            .then(|| {
                locked
                    .permission_receipts
                    .issue(&info.permission_intent, &info.execution, true)
            })
            .transpose()
            .unwrap_or_else(|error| {
                warn!(agent = %info.agent_name, %error, "permission receipt issuance failed");
                None
            });
        let effective_allow = still_authorized && receipt.is_some();
        if effective_allow && remember_session {
            locked.grant_permission(info.execution.clone(), info.permission_intent.seed());
        }
        (effective_allow, receipt)
    } else {
        (false, None)
    };
    finish(
        info,
        terminal_sink,
        effective_allow,
        allow,
        interaction_id,
        receipt,
    )
}

fn active_agent(hub: &Hub, info: &super::types::PendingPermissionInfo) -> bool {
    hub.registry.owns_active_lease(&info.execution)
        && hub
            .registry
            .exact_connection(&info.execution)
            .is_some_and(|active| Arc::ptr_eq(&active, &info.agent_conn))
}

fn finish(
    info: super::types::PendingPermissionInfo,
    terminal_sink: TerminalEventSink,
    effective_allow: bool,
    requested_allow: bool,
    interaction_id: &str,
    receipt: Option<loopal_protocol::PermissionReceipt>,
) -> bool {
    info!(
        agent = %info.agent_name,
        logical_id = %info.logical_id,
        interaction_id,
        requested_allow,
        allow = effective_allow,
        "permission resolved"
    );
    let resolved = AgentEvent::named(
        QualifiedAddress::local(&info.agent_name),
        AgentEventPayload::ToolPermissionResolved {
            id: info.interaction_id.clone(),
        },
    );
    complete_detached(
        info.agent_conn,
        info.agent_ipc_id,
        serde_json::json!({
            "allow": effective_allow,
            "permission_receipt": receipt,
        }),
        Some((terminal_sink, resolved)),
    );
    true
}
