//! SNAT helpers for `AgentEventPayload` — split from `event_payload.rs`
//! so the enum fits the 200-line budget.

use crate::event_payload::AgentEventPayload;

impl AgentEventPayload {
    /// SNAT — stamp `self_hub` onto every still-local qualified address inside
    /// this payload. Already-qualified addresses are left untouched. Called by
    /// the event aggregator before relaying upward to MetaHub.
    pub fn prepend_self_hub(&mut self, self_hub: &str) {
        match self {
            Self::MessageRouted { source, target, .. } => {
                source.prepend_hub_if_local(self_hub);
                target.prepend_hub_if_local(self_hub);
            }
            Self::SubAgentSpawned {
                parent: Some(p), ..
            } => {
                p.prepend_hub_if_local(self_hub);
            }
            Self::InboxEnqueued { source, .. } => {
                source.prepend_hub_if_local(self_hub);
            }
            _ => {}
        }
    }
}
