use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::{
    AgentEvent, AgentEventPayload, QualifiedAddress, Question, UiCapability, UserQuestionResponse,
};
use tokio::sync::Mutex;
use tracing::warn;
use uuid::Uuid;

use super::cleanup::{InteractionKind, remove_if_current, schedule_timeout};
use super::completion::complete_detached;
use super::types::{FastPath, InteractionAudience, PendingQuestionInfo};
use crate::authoritative_events::PreparedAuthoritativeEvent;
use crate::hub::Hub;

pub async fn handle_agent_question(
    hub: &Arc<Mutex<Hub>>,
    agent_conn: Arc<Connection<Listening>>,
    agent_ipc_id: i64,
    params: serde_json::Value,
    agent_name: &str,
) {
    let questions: Vec<Question> =
        match serde_json::from_value(params.get("questions").cloned().unwrap_or_default()) {
            Ok(q) => q,
            Err(e) => {
                warn!(agent = %agent_name, error = %e, "agent/question malformed, denying");
                complete_detached(
                    agent_conn,
                    agent_ipc_id,
                    serde_json::json!({"answers": ["(parse error)"]}),
                    None,
                );
                return;
            }
        };
    let question_id = params
        .get("question_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let classifier_running = params
        .get("classifier_running")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let interaction_id = Uuid::new_v4().to_string();
    let event = AgentEvent::named(
        QualifiedAddress::local(agent_name),
        AgentEventPayload::UserQuestionRequest {
            id: interaction_id.clone(),
            logical_id: question_id.clone(),
            questions,
            classifier_running,
        },
    );
    if super::remote::relay_question_if_remote(
        hub,
        agent_conn.clone(),
        agent_ipc_id,
        agent_name,
        &question_id,
        &interaction_id,
        event.payload.clone(),
    )
    .await
    {
        return;
    }
    let key = (agent_name.to_string(), question_id.clone());
    let (outcome, timeout) = {
        let mut h = hub.lock().await;
        if !h.ui.has_capability(UiCapability::Question) {
            (FastPath::DenyNoUi, h.pending_interaction_timeout())
        } else if h
            .pending_questions
            .keys()
            .any(|(agent, _)| agent == agent_name)
        {
            (FastPath::RejectDuplicate, h.pending_interaction_timeout())
        } else {
            h.pending_questions.insert(
                key,
                PendingQuestionInfo {
                    agent_conn: agent_conn.clone(),
                    agent_ipc_id,
                    agent_name: agent_name.to_string(),
                    interaction_id: interaction_id.clone(),
                    logical_id: question_id.clone(),
                    audience: InteractionAudience::LocalUi,
                },
            );
            let event = h.registry.prepare_generation_event(agent_name, event);
            let outcome =
                FastPath::Pending(Box::new(PreparedAuthoritativeEvent::from_hub(&h, event)));
            (outcome, h.pending_interaction_timeout())
        }
    };

    match outcome {
        FastPath::DenyNoUi => {
            warn!(agent = %agent_name, "no question-capable UI, cancelling question");
            let response = serde_json::to_value(UserQuestionResponse::unsupported(
                &question_id,
                "no question-capable UI",
            ))
            .unwrap_or(serde_json::Value::Null);
            complete_detached(agent_conn, agent_ipc_id, response, None);
        }
        FastPath::RejectDuplicate => {
            warn!(agent = %agent_name, question_id, "concurrent question request rejected");
            let response = serde_json::to_value(UserQuestionResponse::cancelled(question_id))
                .unwrap_or(serde_json::Value::Null);
            complete_detached(agent_conn, agent_ipc_id, response, None);
        }
        FastPath::Pending(mut delivery) => {
            let delivery_hub = hub.clone();
            let delivery_conn = agent_conn.clone();
            let delivery_agent = agent_name.to_string();
            let delivery_logical_id = question_id.clone();
            let delivery_interaction_id = interaction_id.clone();
            let coordinator = tokio::spawn(async move {
                match delivery.deliver().await {
                    Ok(()) => schedule_timeout(
                        &delivery_hub,
                        InteractionKind::Question,
                        delivery_agent,
                        delivery_logical_id,
                        delivery_interaction_id,
                        timeout,
                    ),
                    Err(error) => {
                        warn!(
                            agent = %delivery_agent,
                            question_id = %delivery_logical_id,
                            %error,
                            "question event admission failed; cancelling request"
                        );
                        if remove_if_current(
                            &delivery_hub,
                            InteractionKind::Question,
                            &delivery_agent,
                            &delivery_logical_id,
                            &delivery_interaction_id,
                        )
                        .await
                        {
                            let response = serde_json::to_value(UserQuestionResponse::unsupported(
                                &delivery_logical_id,
                                "Hub event router unavailable",
                            ))
                            .unwrap_or(serde_json::Value::Null);
                            complete_detached(delivery_conn, agent_ipc_id, response, None);
                        }
                    }
                }
            });
            if let Err(error) = coordinator.await {
                tracing::error!(agent = %agent_name, %error, "question admission coordinator failed");
                hub.lock().await.shutdown_signal.notify_one();
                if remove_if_current(
                    hub,
                    InteractionKind::Question,
                    agent_name,
                    &question_id,
                    &interaction_id,
                )
                .await
                {
                    let response = serde_json::to_value(UserQuestionResponse::unsupported(
                        &question_id,
                        "Hub event admission failed",
                    ))
                    .unwrap_or(serde_json::Value::Null);
                    complete_detached(agent_conn, agent_ipc_id, response, None);
                }
            }
        }
    }
}
