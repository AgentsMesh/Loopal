use std::sync::{Arc, Weak};
use std::time::Duration;

use loopal_protocol::UserQuestionResponse;
use serde_json::{Value, json};
use tokio::sync::Mutex;
use tracing::warn;

use crate::pending_relay::{self, OriginRemoteQuestionCancel, PendingRemoteQuestionInfo};
use crate::{Hub, HubUplink};

pub(crate) fn cancel_destination_detached(cancel: OriginRemoteQuestionCancel) {
    let payload = json!({
        "target_hub": cancel.target_hub,
        "operation": "question_cancel",
        "origin_hub": cancel.uplink.hub_name(),
        "agent_name": cancel.origin_agent,
        "interaction_id": cancel.interaction_id,
    });
    tokio::spawn(retry_relay(cancel.uplink, payload));
}

pub(crate) async fn cancel_remote_origins(
    hub: &Arc<Mutex<Hub>>,
    records: Vec<PendingRemoteQuestionInfo>,
) {
    cancel_remote_origins_because(hub, records, "UI disconnect").await;
}

async fn cancel_remote_origins_because(
    hub: &Arc<Mutex<Hub>>,
    records: Vec<PendingRemoteQuestionInfo>,
    reason: &str,
) {
    for record in records {
        if !deliver_resolved(hub, &record, reason).await {
            break;
        }
        let response = UserQuestionResponse::cancelled(&record.interaction_id);
        let payload = json!({
            "target_hub": record.origin_hub,
            "operation": "question_response",
            "payload": {
                "agent_name": record.origin_agent,
                "question_id": record.interaction_id,
                "response": response,
            },
        });
        tokio::spawn(retry_relay(record.uplink.clone(), payload));
    }
}

pub(super) fn schedule_remote_timeout(
    hub: &Arc<Mutex<Hub>>,
    key: (String, String),
    uplink: Arc<HubUplink>,
    deadline: tokio::time::Instant,
) {
    let hub = Arc::downgrade(hub);
    tokio::spawn(async move {
        tokio::time::sleep_until(deadline).await;
        expire_remote_question(hub, key, uplink, deadline).await;
    });
}

async fn expire_remote_question(
    hub: Weak<Mutex<Hub>>,
    key: (String, String),
    uplink: Arc<HubUplink>,
    deadline: tokio::time::Instant,
) {
    let Some(hub) = hub.upgrade() else {
        return;
    };
    let record = {
        let mut h = hub.lock().await;
        h.pending_remote_questions
            .get(&key)
            .is_some_and(|record| {
                Arc::ptr_eq(&record.uplink, &uplink) && record.deadline == deadline
            })
            .then(|| h.pending_remote_questions.remove(&key))
            .flatten()
    };
    if let Some(record) = record {
        cancel_remote_origins_because(&hub, vec![record], "deadline expired").await;
    }
}

pub(crate) async fn resolve_remote_records(
    hub: &Arc<Mutex<Hub>>,
    records: Vec<PendingRemoteQuestionInfo>,
) {
    for record in records {
        if !deliver_resolved(hub, &record, "uplink cleanup").await {
            break;
        }
    }
}

async fn deliver_resolved(
    hub: &Arc<Mutex<Hub>>,
    record: &PendingRemoteQuestionInfo,
    reason: &str,
) -> bool {
    match pending_relay::deliver_terminal_event(hub, super::remote_resolved_event(record)).await {
        Ok(()) => true,
        Err(error) => {
            warn!(
                agent = %record.qualified_agent,
                interaction_id = %record.interaction_id,
                %error,
                %reason,
                "remote question terminal event delivery failed"
            );
            false
        }
    }
}

async fn retry_relay(uplink: Arc<HubUplink>, payload: Value) {
    let mut delay = Duration::from_millis(200);
    for _ in 0..6 {
        if !uplink.connection().is_connected() {
            return;
        }
        if tokio::time::timeout(Duration::from_secs(2), uplink.relay_remote(payload.clone()))
            .await
            .is_ok_and(|result| result.is_ok())
        {
            return;
        }
        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(Duration::from_secs(5));
    }
    warn!("remote interaction cleanup relay exhausted retries");
}

#[cfg(test)]
#[path = "cleanup/tests.rs"]
mod tests;
