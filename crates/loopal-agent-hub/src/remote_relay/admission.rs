use std::sync::Arc;
use std::time::Duration;

use loopal_protocol::{AgentEvent, AgentEventPayload, QualifiedAddress, UiCapability};
use serde_json::{Value, json};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::authoritative_events::PreparedAuthoritativeEvent;
use crate::pending_relay::PendingRemoteQuestionInfo;
use crate::{Hub, HubUplink};

pub(super) async fn emit_question(
    hub: &Arc<Mutex<Hub>>,
    params: &Value,
    active_uplink: Arc<HubUplink>,
) -> Result<Value, String> {
    let origin_hub = super::required(params, "origin_hub")?;
    let agent = super::required(params, "agent_name")?;
    let payload: AgentEventPayload = serde_json::from_value(params["payload"].clone())
        .map_err(|error| format!("invalid remote question payload: {error}"))?;
    let (interaction_id, logical_id) = question_ids(&payload)?;
    Uuid::parse_str(&interaction_id)
        .map_err(|_| "remote question interaction id is not an opaque UUID".to_string())?;
    let qualified_agent = format!("{origin_hub}/{agent}");
    let requested_timeout = params["timeout_ms"]
        .as_u64()
        .filter(|value| *value > 0)
        .map(Duration::from_millis);
    let event = AgentEvent::named(QualifiedAddress::local(&qualified_agent), payload.clone());
    let admission = {
        let mut h = hub.lock().await;
        if !h
            .uplink
            .as_ref()
            .is_some_and(|uplink| Arc::ptr_eq(uplink, &active_uplink))
        {
            return Err("remote question arrived on a stale uplink generation".into());
        }
        let timeout = requested_timeout
            .unwrap_or_else(|| h.pending_interaction_timeout())
            .min(h.pending_interaction_timeout());
        let deadline = tokio::time::Instant::now() + timeout;
        if !h.ui.has_capability(UiCapability::Question) {
            AdmissionOutcome::Rejected
        } else {
            admit(
                &mut h,
                Admission {
                    origin_hub,
                    agent,
                    qualified_agent,
                    interaction_id,
                    logical_id,
                    payload,
                    event,
                    uplink: active_uplink,
                    deadline,
                },
            )
        }
    };
    match admission {
        AdmissionOutcome::Rejected => Ok(json!({"emitted": false})),
        AdmissionOutcome::Retry => Ok(json!({"emitted": true})),
        AdmissionOutcome::Admitted(admitted) => {
            let mut admitted = *admitted;
            let delivery_hub = hub.clone();
            let cleanup_key = admitted.key.clone();
            let cleanup_uplink = admitted.uplink.clone();
            let coordinator_key = cleanup_key.clone();
            let coordinator_uplink = cleanup_uplink.clone();
            let coordinator = tokio::spawn(async move {
                match admitted.delivery.deliver().await {
                    Ok(()) => {
                        super::cleanup::schedule_remote_timeout(
                            &delivery_hub,
                            admitted.key,
                            admitted.uplink,
                            admitted.deadline,
                        );
                        Ok(())
                    }
                    Err(error) => {
                        remove_if_current(&delivery_hub, &coordinator_key, &coordinator_uplink)
                            .await;
                        Err(format!("remote question event admission failed: {error}"))
                    }
                }
            });
            match coordinator.await {
                Ok(Ok(())) => Ok(json!({"emitted": true})),
                Ok(Err(error)) => {
                    remove_if_current(hub, &cleanup_key, &cleanup_uplink).await;
                    Err(error)
                }
                Err(error) => {
                    hub.lock().await.shutdown_signal.notify_one();
                    remove_if_current(hub, &cleanup_key, &cleanup_uplink).await;
                    Err(format!(
                        "remote question admission coordinator failed: {error}"
                    ))
                }
            }
        }
    }
}

struct Admission<'a> {
    origin_hub: &'a str,
    agent: &'a str,
    qualified_agent: String,
    interaction_id: String,
    logical_id: String,
    payload: AgentEventPayload,
    event: AgentEvent,
    uplink: Arc<HubUplink>,
    deadline: tokio::time::Instant,
}

struct AdmittedRemoteQuestion {
    key: (String, String),
    uplink: Arc<HubUplink>,
    deadline: tokio::time::Instant,
    delivery: PreparedAuthoritativeEvent,
}

enum AdmissionOutcome {
    Rejected,
    Retry,
    Admitted(Box<AdmittedRemoteQuestion>),
}

fn admit(hub: &mut Hub, admission: Admission<'_>) -> AdmissionOutcome {
    let key = (
        admission.qualified_agent.clone(),
        admission.interaction_id.clone(),
    );
    if let Some(current) = hub.pending_remote_questions.get(&key) {
        let is_retry = current.origin_hub == admission.origin_hub
            && current.origin_agent == admission.agent
            && current.logical_id == admission.logical_id
            && Arc::ptr_eq(&current.uplink, &admission.uplink);
        return if is_retry {
            AdmissionOutcome::Retry
        } else {
            AdmissionOutcome::Rejected
        };
    }
    if hub
        .pending_remote_questions
        .keys()
        .any(|(name, _)| name == &admission.qualified_agent)
    {
        return AdmissionOutcome::Rejected;
    }
    hub.remote_views
        .entry(admission.qualified_agent.clone())
        .or_insert_with(|| {
            Arc::new(Mutex::new(loopal_view_state::ViewStateReducer::new(
                &admission.qualified_agent,
            )))
        });
    hub.pending_remote_questions.insert(
        key.clone(),
        PendingRemoteQuestionInfo {
            origin_hub: admission.origin_hub.to_string(),
            origin_agent: admission.agent.to_string(),
            qualified_agent: admission.qualified_agent,
            interaction_id: admission.interaction_id,
            logical_id: admission.logical_id,
            request: admission.payload,
            uplink: admission.uplink.clone(),
            deadline: admission.deadline,
            forwarding: false,
        },
    );
    AdmissionOutcome::Admitted(Box::new(AdmittedRemoteQuestion {
        key,
        uplink: admission.uplink,
        deadline: admission.deadline,
        delivery: PreparedAuthoritativeEvent::from_hub(hub, admission.event),
    }))
}

async fn remove_if_current(
    hub: &Arc<Mutex<Hub>>,
    key: &(String, String),
    uplink: &Arc<HubUplink>,
) -> bool {
    let mut hub = hub.lock().await;
    if !hub
        .pending_remote_questions
        .get(key)
        .is_some_and(|record| Arc::ptr_eq(&record.uplink, uplink))
    {
        return false;
    }
    hub.pending_remote_questions.remove(key).is_some()
}

fn question_ids(payload: &AgentEventPayload) -> Result<(String, String), String> {
    match payload {
        AgentEventPayload::UserQuestionRequest { id, logical_id, .. }
            if !id.is_empty() && !logical_id.is_empty() =>
        {
            Ok((id.clone(), logical_id.clone()))
        }
        AgentEventPayload::UserQuestionRequest { .. } => {
            Err("remote question relay missing interaction or logical id".into())
        }
        _ => Err("remote question relay received a non-question payload".into()),
    }
}

#[cfg(test)]
#[path = "admission/tests.rs"]
mod tests;
