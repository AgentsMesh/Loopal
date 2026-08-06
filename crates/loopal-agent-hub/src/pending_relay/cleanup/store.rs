use std::collections::HashMap;

use super::InteractionKind;
use super::interaction::PendingInteraction;
use crate::hub::Hub;

mod connection;
mod uplink;

pub(super) use connection::{take_by_request, take_for_agent_connection};
pub(super) use uplink::take_for_uplink;

pub(super) fn take_for_agent(hub: &mut Hub, agent_name: &str) -> Vec<PendingInteraction> {
    let mut pending = Vec::new();
    let permission_keys = keys_for_agent(&hub.pending_permissions, agent_name);
    for key in permission_keys {
        if let Some(info) = hub.pending_permissions.remove(&key) {
            pending.push(PendingInteraction::Permission { info });
        }
    }
    let question_keys = keys_for_agent(&hub.pending_questions, agent_name);
    for key in question_keys {
        if let Some(info) = hub.pending_questions.remove(&key) {
            pending.push(PendingInteraction::Question { id: key.1, info });
        }
    }
    let plan_keys = keys_for_agent(&hub.pending_plan_approvals, agent_name);
    for key in plan_keys {
        if let Some(info) = hub.pending_plan_approvals.remove(&key) {
            pending.push(PendingInteraction::PlanApproval { info });
        }
    }
    pending
}

fn keys_for_agent<T>(
    map: &HashMap<(String, String), T>,
    agent_name: &str,
) -> Vec<(String, String)> {
    map.keys()
        .filter(|(agent, _)| agent == agent_name)
        .cloned()
        .collect()
}

pub(super) fn take_unavailable(
    hub: &mut Hub,
    permission: bool,
    question: bool,
    plan: bool,
) -> Vec<PendingInteraction> {
    let mut pending = Vec::new();
    if permission {
        let keys: Vec<_> = hub.pending_permissions.keys().cloned().collect();
        for key in keys {
            if let Some(info) = hub.pending_permissions.remove(&key) {
                pending.push(PendingInteraction::Permission { info });
            }
        }
    }
    if question {
        let keys: Vec<_> = hub
            .pending_questions
            .iter()
            .filter(|(_, info)| info.audience.is_local())
            .map(|(key, _)| key.clone())
            .collect();
        for key in keys {
            if let Some(info) = hub.pending_questions.remove(&key) {
                pending.push(PendingInteraction::Question { id: key.1, info });
            }
        }
    }
    if plan {
        let keys: Vec<_> = hub.pending_plan_approvals.keys().cloned().collect();
        for key in keys {
            if let Some(info) = hub.pending_plan_approvals.remove(&key) {
                pending.push(PendingInteraction::PlanApproval { info });
            }
        }
    }
    pending
}

pub(super) fn take_if_generation(
    hub: &mut Hub,
    kind: InteractionKind,
    agent_name: &str,
    logical_id: &str,
    interaction_id: &str,
) -> Option<PendingInteraction> {
    let key = (agent_name.to_string(), logical_id.to_string());
    match kind {
        InteractionKind::Permission => hub
            .pending_permissions
            .get(&key)
            .is_some_and(|info| info.interaction_id == interaction_id)
            .then(|| hub.pending_permissions.remove(&key))
            .flatten()
            .map(|info| PendingInteraction::Permission { info }),
        InteractionKind::Question => hub
            .pending_questions
            .get(&key)
            .is_some_and(|info| info.interaction_id == interaction_id)
            .then(|| hub.pending_questions.remove(&key))
            .flatten()
            .map(|info| PendingInteraction::Question {
                id: logical_id.into(),
                info,
            }),
        InteractionKind::PlanApproval => hub
            .pending_plan_approvals
            .get(&key)
            .is_some_and(|info| info.interaction_id == interaction_id)
            .then(|| hub.pending_plan_approvals.remove(&key))
            .flatten()
            .map(|info| PendingInteraction::PlanApproval { info }),
    }
}
