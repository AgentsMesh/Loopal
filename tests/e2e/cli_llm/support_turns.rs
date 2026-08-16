use std::time::Duration;

use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use serde_json::{Value, json};

use super::event_waits::TurnOutcome;
use super::harness::CliHarness;
use super::process::TIMEOUT;

impl CliHarness {
    pub async fn run_turn(&mut self, prompt: &str) -> TurnOutcome {
        self.run_turn_with(prompt, json!({})).await
    }

    pub async fn run_turn_with(&mut self, prompt: &str, extra: Value) -> TurnOutcome {
        let mut params = json!({
            "prompt": prompt,
            "model": self.provider.model(),
            "cwd": self.cwd().to_string_lossy(),
        });
        if let (Some(base), Value::Object(overlay)) = (params.as_object_mut(), extra) {
            base.extend(overlay);
        }
        self.conn
            .send_request(methods::AGENT_START.name, params)
            .await
            .expect("agent_start");

        let mut out = TurnOutcome::default();
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_secs(8), self.rx.recv()).await {
                Ok(Some(Incoming::Notification { method, params })) => {
                    if method == methods::AGENT_EVENT.name
                        && let Ok(event) = serde_json::from_value::<AgentEvent>(params)
                    {
                        out.events.push(format!("{:?}", event.payload));
                        match event.payload {
                            AgentEventPayload::Stream { text } => out.text.push_str(&text),
                            AgentEventPayload::ThinkingStream { text } => {
                                out.thinking.push_str(&text)
                            }
                            AgentEventPayload::Error { message } => out.error = Some(message),
                            AgentEventPayload::Finished => {
                                // reason: Error -> Finished is one failed turn boundary.
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

    pub async fn begin_turn(&self, prompt: &str) {
        self.conn
            .send_request(
                methods::AGENT_START.name,
                json!({
                    "prompt": prompt,
                    "model": self.provider.model(),
                    "cwd": self.cwd().to_string_lossy(),
                }),
            )
            .await
            .expect("agent_start");
    }

    pub async fn interrupt(&self) {
        self.conn
            .send_notification(methods::AGENT_INTERRUPT.name, json!({}))
            .await
            .expect("interrupt");
    }
}
