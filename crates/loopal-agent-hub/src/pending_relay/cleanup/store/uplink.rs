use std::sync::Arc;

use super::super::interaction::PendingInteraction;
use crate::pending_relay::types::{InteractionAudience, PendingRemoteQuestionInfo};
use crate::{Hub, HubUplink};

pub(in crate::pending_relay::cleanup) fn take_for_uplink(
    hub: &mut Hub,
    uplink: &Arc<HubUplink>,
) -> (Vec<PendingInteraction>, Vec<PendingRemoteQuestionInfo>) {
    let origin_keys: Vec<_> = hub
        .pending_questions
        .iter()
        .filter(|(_, info)| {
            matches!(
                &info.audience,
                InteractionAudience::RemoteUi { uplink: owner, .. }
                    if Arc::ptr_eq(owner, uplink)
            )
        })
        .map(|(key, _)| key.clone())
        .collect();
    let mut origin = Vec::with_capacity(origin_keys.len());
    for key in origin_keys {
        if let Some(info) = hub.pending_questions.remove(&key) {
            origin.push(PendingInteraction::Question { id: key.1, info });
        }
    }

    let destination_keys: Vec<_> = hub
        .pending_remote_questions
        .iter()
        .filter(|(_, info)| Arc::ptr_eq(&info.uplink, uplink))
        .map(|(key, _)| key.clone())
        .collect();
    let destination = destination_keys
        .into_iter()
        .filter_map(|key| hub.pending_remote_questions.remove(&key))
        .collect();
    (origin, destination)
}
