use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};

use super::super::interaction::PendingInteraction;
use crate::hub::Hub;

pub(in crate::pending_relay::cleanup) fn take_for_agent_connection(
    hub: &mut Hub,
    agent_name: &str,
    agent_conn: &Arc<Connection<Listening>>,
) -> Vec<PendingInteraction> {
    let permission_keys: Vec<_> = hub
        .pending_permissions
        .iter()
        .filter(|((agent, _), info)| {
            agent == agent_name && Arc::ptr_eq(&info.agent_conn, agent_conn)
        })
        .map(|(key, _)| key.clone())
        .collect();
    let question_keys: Vec<_> = hub
        .pending_questions
        .iter()
        .filter(|((agent, _), info)| {
            agent == agent_name && Arc::ptr_eq(&info.agent_conn, agent_conn)
        })
        .map(|(key, _)| key.clone())
        .collect();
    let plan_keys: Vec<_> = hub
        .pending_plan_approvals
        .iter()
        .filter(|((agent, _), info)| {
            agent == agent_name && Arc::ptr_eq(&info.agent_conn, agent_conn)
        })
        .map(|(key, _)| key.clone())
        .collect();

    let mut pending =
        Vec::with_capacity(permission_keys.len() + question_keys.len() + plan_keys.len());
    for key in permission_keys {
        if let Some(info) = hub.pending_permissions.remove(&key) {
            pending.push(PendingInteraction::Permission { info });
        }
    }
    for key in question_keys {
        if let Some(info) = hub.pending_questions.remove(&key) {
            pending.push(PendingInteraction::Question { id: key.1, info });
        }
    }
    for key in plan_keys {
        if let Some(info) = hub.pending_plan_approvals.remove(&key) {
            pending.push(PendingInteraction::PlanApproval { info });
        }
    }
    pending
}

pub(in crate::pending_relay::cleanup) fn take_by_request(
    hub: &mut Hub,
    agent_name: &str,
    agent_conn: &Arc<Connection<Listening>>,
    agent_ipc_id: i64,
) -> Option<PendingInteraction> {
    let permission = hub
        .pending_permissions
        .iter()
        .find(|((agent, _), info)| {
            agent == agent_name
                && info.agent_ipc_id == agent_ipc_id
                && Arc::ptr_eq(&info.agent_conn, agent_conn)
        })
        .map(|(key, _)| key.clone());
    if let Some(key) = permission {
        let info = hub.pending_permissions.remove(&key)?;
        return Some(PendingInteraction::Permission { info });
    }
    let question = hub
        .pending_questions
        .iter()
        .find(|((agent, _), info)| {
            agent == agent_name
                && info.agent_ipc_id == agent_ipc_id
                && Arc::ptr_eq(&info.agent_conn, agent_conn)
        })
        .map(|(key, _)| key.clone());
    if let Some(key) = question {
        let info = hub.pending_questions.remove(&key)?;
        return Some(PendingInteraction::Question { id: key.1, info });
    }
    let plan = hub
        .pending_plan_approvals
        .iter()
        .find(|((agent, _), info)| {
            agent == agent_name
                && info.agent_ipc_id == agent_ipc_id
                && Arc::ptr_eq(&info.agent_conn, agent_conn)
        })
        .map(|(key, _)| key.clone());
    if let Some(key) = plan {
        let info = hub.pending_plan_approvals.remove(&key)?;
        return Some(PendingInteraction::PlanApproval { info });
    }
    None
}
