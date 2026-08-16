use std::time::Duration;

use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload};

use super::harness::CliHarness;

#[derive(Default, Debug)]
pub struct TurnOutcome {
    pub text: String,
    pub thinking: String,
    pub finished: bool,
    pub cancelled: bool,
    pub error: Option<String>,
    pub events: Vec<String>,
}

impl TurnOutcome {
    pub fn tool_result_count(&self) -> usize {
        self.events
            .iter()
            .filter(|event| event.starts_with("ToolResult"))
            .count()
    }
}

impl CliHarness {
    pub async fn await_event(&mut self, needle: &str, budget: Duration) -> bool {
        let deadline = tokio::time::Instant::now() + budget;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(250), self.rx.recv()).await {
                Ok(Some(Incoming::Notification { method, params }))
                    if method == methods::AGENT_EVENT.name =>
                {
                    if let Ok(event) = serde_json::from_value::<AgentEvent>(params)
                        && format!("{:?}", event.payload).contains(needle)
                    {
                        return true;
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) => return false,
                Err(_) => {}
            }
        }
        false
    }

    pub async fn await_settled(&mut self, budget: Duration) -> TurnOutcome {
        let mut out = TurnOutcome::default();
        let deadline = tokio::time::Instant::now() + budget;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_secs(1), self.rx.recv()).await {
                Ok(Some(Incoming::Notification { method, params }))
                    if method == methods::AGENT_EVENT.name =>
                {
                    if let Ok(event) = serde_json::from_value::<AgentEvent>(params) {
                        out.events.push(format!("{:?}", event.payload));
                        match event.payload {
                            AgentEventPayload::TurnCancelled { .. }
                            | AgentEventPayload::Interrupted => {
                                out.cancelled = true;
                                break;
                            }
                            AgentEventPayload::AwaitingInput => {
                                out.cancelled = !out.finished;
                                break;
                            }
                            AgentEventPayload::Finished => {
                                out.finished = true;
                                break;
                            }
                            AgentEventPayload::Error { message } => {
                                out.error = Some(message);
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(_) => {}
            }
        }
        out
    }
}
