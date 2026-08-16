use std::collections::HashMap;

use loopal_protocol::{
    WorkflowTerminalDeliveryId, WorkflowTerminalDisposition, WorkflowTerminalNotification,
};

use crate::shared_session::SharedSession;

const MAX_PENDING_WORKFLOW_TERMINALS: usize = 64;

pub(crate) struct WorkflowTerminalPending {
    entries: tokio::sync::Mutex<HashMap<WorkflowTerminalDeliveryId, PendingEntry>>,
}

struct PendingEntry {
    payload_digest: String,
    disposition: tokio::sync::watch::Receiver<Option<WorkflowTerminalDisposition>>,
}

pub(crate) enum WorkflowTerminalClaim {
    New,
    Pending,
    Completed(WorkflowTerminalDisposition),
    Conflict,
    Full,
}

impl WorkflowTerminalPending {
    pub(crate) fn new() -> Self {
        Self {
            entries: tokio::sync::Mutex::new(HashMap::new()),
        }
    }
}

impl SharedSession {
    pub(crate) async fn claim_workflow_terminal(
        &self,
        notification: &WorkflowTerminalNotification,
        disposition: tokio::sync::watch::Receiver<Option<WorkflowTerminalDisposition>>,
    ) -> WorkflowTerminalClaim {
        let id = &notification.delivery_id;
        let digest = notification.payload_digest();
        let mut entries = self.pending_workflow_terminals.entries.lock().await;
        if let Some(entry) = entries.get(id) {
            if entry.payload_digest != digest {
                return WorkflowTerminalClaim::Conflict;
            }
            let completed = entry.disposition.borrow().clone();
            let still_live = entry.disposition.has_changed().is_ok();
            match completed {
                Some(
                    WorkflowTerminalDisposition::Queued
                    | WorkflowTerminalDisposition::Retryable { .. },
                ) => {
                    entries.remove(id);
                }
                Some(completed) => return WorkflowTerminalClaim::Completed(completed),
                None if still_live => return WorkflowTerminalClaim::Pending,
                None => {
                    entries.remove(id);
                }
            }
        }
        if entries.len() >= MAX_PENDING_WORKFLOW_TERMINALS {
            // Capacity pressure may evict terminal tombstones, never live input.
            entries.retain(|_, entry| {
                entry.disposition.borrow().is_none() && entry.disposition.has_changed().is_ok()
            });
        }
        if entries.len() >= MAX_PENDING_WORKFLOW_TERMINALS {
            return WorkflowTerminalClaim::Full;
        }
        entries.insert(
            id.clone(),
            PendingEntry {
                payload_digest: digest,
                disposition,
            },
        );
        WorkflowTerminalClaim::New
    }

    pub(crate) async fn discard_workflow_terminal(
        &self,
        id: &WorkflowTerminalDeliveryId,
        payload_digest: &str,
        disposition: &tokio::sync::watch::Receiver<Option<WorkflowTerminalDisposition>>,
    ) {
        let mut entries = self.pending_workflow_terminals.entries.lock().await;
        if entries.get(id).is_some_and(|entry| {
            entry.payload_digest == payload_digest && entry.disposition.same_channel(disposition)
        }) {
            entries.remove(id);
        }
    }
}

#[cfg(test)]
#[path = "workflow_terminal_pending_tests.rs"]
mod tests;
