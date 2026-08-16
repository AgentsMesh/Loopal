use std::time::Duration;

use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use serde_json::{Value, json};

use super::event_waits::TurnOutcome;
use super::harness::CliHarness;
use super::process::TIMEOUT;

impl CliHarness {
    pub async fn begin_persistent(&mut self) -> String {
        self.start_persistent_with(json!({})).await.0
    }

    pub async fn begin_persistent_with(&mut self, extra: Value) -> String {
        self.start_persistent_with(extra).await.0
    }

    pub async fn resume_persistent(&mut self, session_id: &str) -> (String, Vec<String>) {
        self.start_persistent_with(json!({"resume": session_id}))
            .await
    }

    async fn start_persistent_with(&mut self, extra: Value) -> (String, Vec<String>) {
        let mut params = json!({
            "model": self.provider.model(),
            "cwd": self.cwd().to_string_lossy(),
            "lifecycle": "persistent",
        });
        if let (Some(base), Value::Object(overlay)) = (params.as_object_mut(), extra) {
            base.extend(overlay);
        }
        let response = self
            .conn
            .send_request(methods::AGENT_START.name, params)
            .await
            .expect("agent_start persistent");
        let session_id = response["session_id"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let mut events = Vec::new();
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_secs(8), self.rx.recv()).await {
                Ok(Some(Incoming::Notification { method, params }))
                    if method == methods::AGENT_EVENT.name =>
                {
                    if let Ok(event) = serde_json::from_value::<AgentEvent>(params) {
                        events.push(format!("{:?}", event.payload));
                        if matches!(event.payload, AgentEventPayload::AwaitingInput) {
                            break;
                        }
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => break,
            }
        }
        (session_id, events)
    }

    pub async fn turn_via_message(&mut self, text: &str) -> TurnOutcome {
        self.message_fire(text).await;
        self.collect_persistent().await
    }

    pub async fn message_fire(&self, text: &str) {
        let envelope = loopal_protocol::Envelope {
            id: uuid::Uuid::new_v4(),
            source: loopal_protocol::MessageSource::Human,
            target: "main".into(),
            content: loopal_protocol::UserContent::text_only(text),
            timestamp: chrono::Utc::now(),
            summary: None,
            agent_completion: None,
        };
        self.conn
            .send_request(
                methods::AGENT_MESSAGE.name,
                serde_json::to_value(&envelope).unwrap(),
            )
            .await
            .expect("agent_message");
    }

    pub async fn control(&mut self, command: Value) -> TurnOutcome {
        self.conn
            .send_request(methods::AGENT_CONTROL.name, command)
            .await
            .expect("agent_control");
        self.collect_persistent().await
    }

    pub async fn control_fire(&self, command: Value) {
        self.conn
            .send_request(methods::AGENT_CONTROL.name, command)
            .await
            .expect("agent_control");
    }

    pub async fn collect_persistent(&mut self) -> TurnOutcome {
        let mut out = TurnOutcome::default();
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_secs(8), self.rx.recv()).await {
                Ok(Some(Incoming::Notification { method, params }))
                    if method == methods::AGENT_EVENT.name =>
                {
                    if let Ok(event) = serde_json::from_value::<AgentEvent>(params) {
                        out.events.push(format!("{:?}", event.payload));
                        match event.payload {
                            AgentEventPayload::Stream { text } => out.text.push_str(&text),
                            AgentEventPayload::ThinkingStream { text } => {
                                out.thinking.push_str(&text)
                            }
                            AgentEventPayload::Error { message } => out.error = Some(message),
                            AgentEventPayload::Finished | AgentEventPayload::AwaitingInput => {
                                out.finished = out.error.is_none();
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => break,
            }
        }
        out
    }
}
