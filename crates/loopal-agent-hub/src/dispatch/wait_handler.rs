//! Hub wait_agent handler — waits for a spawned agent to finish.

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::{Mutex, watch};
use tracing::info;

use loopal_protocol::{
    AgentCompletion, TRANSPORT_ERROR_REASON, WAIT_AGENT_TYPED_RESPONSE_V1, WaitAgentResponse,
};

use crate::hub::Hub;

const WAIT_AGENT_TIMEOUT: Duration = Duration::from_secs(600);

pub async fn handle_wait_agent(hub: &Arc<Mutex<Hub>>, params: Value) -> Result<Value, String> {
    handle_wait_agent_with_timeout(hub, params, WAIT_AGENT_TIMEOUT).await
}

async fn handle_wait_agent_with_timeout(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
    timeout: Duration,
) -> Result<Value, String> {
    let name = params["name"].as_str().ok_or("missing 'name'")?.to_string();
    let typed_response_requested =
        params["response_format"].as_str() == Some(WAIT_AGENT_TYPED_RESPONSE_V1);
    info!(agent = %name, "handle_wait_agent start");

    let rx = {
        let mut h = hub.lock().await;

        if let Some(completion) = h.registry.completion(&name).cloned() {
            info!(agent = %name, reason = %completion.reason, "handle_wait_agent: found cached completion");
            return response_value(
                WaitAgentResponse::from_completion(completion),
                typed_response_requested,
            );
        }

        if !h.registry.agents.contains_key(&name) {
            info!(agent = %name, "handle_wait_agent: not found");
            return response_value(WaitAgentResponse::not_found(), typed_response_requested);
        }
        h.registry.watch_completion(&name)
    }; // Lock released.

    info!(agent = %name, "handle_wait_agent: waiting");
    let response = match wait_for_completion(rx, timeout).await {
        CompletionWait::Completed(completion) => {
            let response = WaitAgentResponse::from_completion(completion);
            info!(agent = %name, status = ?response.status, reason = %response.reason, "handle_wait_agent: completed");
            response
        }
        CompletionWait::ChannelClosed => {
            info!(agent = %name, "handle_wait_agent: completion channel closed");
            WaitAgentResponse::from_completion(AgentCompletion::new(
                TRANSPORT_ERROR_REASON,
                Some("agent completion channel closed before agent/completed".into()),
            ))
        }
        CompletionWait::TimedOut => {
            info!(agent = %name, timeout_secs = timeout.as_secs_f64(), "handle_wait_agent: timed out");
            WaitAgentResponse::timed_out()
        }
    };
    response_value(response, typed_response_requested)
}

enum CompletionWait {
    Completed(AgentCompletion),
    ChannelClosed,
    TimedOut,
}

async fn wait_for_completion(
    mut rx: watch::Receiver<Option<AgentCompletion>>,
    timeout: Duration,
) -> CompletionWait {
    let wait = async {
        loop {
            if let Some(completion) = rx.borrow().clone() {
                return CompletionWait::Completed(completion);
            }
            if rx.changed().await.is_err() {
                return rx
                    .borrow()
                    .clone()
                    .map(CompletionWait::Completed)
                    .unwrap_or(CompletionWait::ChannelClosed);
            }
        }
    };
    tokio::time::timeout(timeout, wait)
        .await
        .unwrap_or(CompletionWait::TimedOut)
}

fn response_value(
    mut response: WaitAgentResponse,
    typed_response_requested: bool,
) -> Result<Value, String> {
    if !typed_response_requested && response.status != loopal_protocol::WaitAgentStatus::Completed {
        response.output = response.legacy_safe_output();
    }
    serde_json::to_value(response)
        .map_err(|error| format!("wait_agent response encode failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use loopal_protocol::{AgentEvent, WaitAgentStatus};
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn wait_timeout_is_a_typed_terminal_status() {
        let (_tx, rx) = watch::channel::<Option<AgentCompletion>>(None);

        assert!(matches!(
            wait_for_completion(rx, Duration::from_millis(1)).await,
            CompletionWait::TimedOut
        ));
        assert_eq!(
            WaitAgentResponse::timed_out().status,
            WaitAgentStatus::TimedOut
        );
    }

    #[tokio::test]
    async fn closed_channel_without_completion_is_not_success() {
        let (tx, rx) = watch::channel::<Option<AgentCompletion>>(None);
        drop(tx);

        assert!(matches!(
            wait_for_completion(rx, Duration::from_secs(1)).await,
            CompletionWait::ChannelClosed
        ));
    }

    #[tokio::test]
    async fn handler_timeout_uses_the_typed_wire_response() {
        let (event_tx, _event_rx) = mpsc::channel::<AgentEvent>(8);
        let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
        let (_peer_transport, hub_transport) = loopal_ipc::duplex_pair();
        let (connection, _incoming) = loopal_ipc::Connection::new(hub_transport).into_listening();
        hub.lock()
            .await
            .registry
            .register_connection("slow", connection)
            .unwrap();

        let value = handle_wait_agent_with_timeout(
            &hub,
            serde_json::json!({
                "name": "slow",
                "response_format": WAIT_AGENT_TYPED_RESPONSE_V1,
            }),
            Duration::from_millis(1),
        )
        .await
        .unwrap();
        let response: WaitAgentResponse = serde_json::from_value(value).unwrap();
        assert_eq!(response.status, WaitAgentStatus::TimedOut);
        assert_eq!(response.reason, "timeout");
        assert_eq!(response.output, "(agent timed out)");
    }

    #[tokio::test]
    async fn legacy_failure_output_is_marked_but_typed_output_and_cache_stay_raw() {
        let (event_tx, _event_rx) = mpsc::channel::<AgentEvent>(8);
        let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
        {
            let _pending = hub.lock().await.registry.emit_agent_completion(
                "failed",
                AgentCompletion::new("error", Some("partial findings".into())),
            );
        }

        let legacy = handle_wait_agent_with_timeout(
            &hub,
            serde_json::json!({"name": "failed"}),
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        assert_eq!(legacy["status"], "failed");
        assert_eq!(legacy["reason"], "error");
        assert_eq!(
            legacy["output"],
            "[agent completion failed; reason: error]\npartial findings"
        );

        let typed = handle_wait_agent_with_timeout(
            &hub,
            serde_json::json!({
                "name": "failed",
                "response_format": WAIT_AGENT_TYPED_RESPONSE_V1,
            }),
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        assert_eq!(typed["output"], "partial findings");
        assert_eq!(
            hub.lock()
                .await
                .registry
                .completion("failed")
                .and_then(|completion| completion.result.as_deref()),
            Some("partial findings")
        );
    }

    #[test]
    fn legacy_timeout_and_not_found_outputs_are_failure_marked() {
        for response in [
            WaitAgentResponse::timed_out(),
            WaitAgentResponse::not_found(),
        ] {
            let reason = response.reason.clone();
            let value = response_value(response, false).unwrap();
            assert!(value["output"].as_str().is_some_and(|output| {
                output.starts_with(&format!("[agent completion failed; reason: {reason}]"))
            }));
        }
    }
}
