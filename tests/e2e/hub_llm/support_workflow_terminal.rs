use std::collections::VecDeque;

use loopal_protocol::{AgentEventPayload, WorkflowRunId, WorkflowTerminalDeliveryId};
use loopal_storage::{SessionStore, TurnEventStore};
use loopal_turn::TurnTrigger;

use super::hub::{HubHarness, TIMEOUT};
use super::workflow_lifecycle::is_root;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersistedWorkflowResult {
    pub session_id: String,
    pub run_id: String,
    pub terminal_revision: u64,
    pub payload_digest: String,
    pub state: String,
    pub content: String,
}

impl HubHarness {
    pub async fn wait_for_root_stream(&mut self, canary: &str) {
        let mut recent_root_events = VecDeque::with_capacity(16);
        let result = tokio::time::timeout(TIMEOUT, async {
            loop {
                let event = self.next_agent_event().await;
                if is_root(&event) {
                    if recent_root_events.len() == 16 {
                        recent_root_events.pop_front();
                    }
                    let encoded = serde_json::to_string(&event.payload)
                        .unwrap_or_else(|_| format!("{:?}", event.payload));
                    recent_root_events.push_back(encoded.chars().take(320).collect::<String>());
                    if matches!(
                        event.payload,
                        AgentEventPayload::Stream { ref text } if text.contains(canary)
                    ) {
                        return;
                    }
                }
            }
        })
        .await;
        if result.is_err() {
            let journal = self.journal().await;
            let labels = journal
                .as_array()
                .into_iter()
                .flatten()
                .map(|call| {
                    call.get("callLabel")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("<unmatched>")
                })
                .collect::<Vec<_>>();
            panic!(
                "root stream canary '{canary}' timed out; mock calls: {labels:?}; recent root events: {recent_root_events:?}"
            );
        }
    }

    pub async fn wait_for_terminal_root_response(&mut self, canary: &str) -> String {
        tokio::time::timeout(TIMEOUT, async {
            let mut text = String::new();
            let mut observed = false;
            loop {
                let event = self.next_agent_event().await;
                if !is_root(&event) {
                    continue;
                }
                match event.payload {
                    AgentEventPayload::Stream { text: chunk } => {
                        text.push_str(&chunk);
                        observed |= text.contains(canary);
                    }
                    AgentEventPayload::Finished | AgentEventPayload::AwaitingInput if observed => {
                        return text;
                    }
                    AgentEventPayload::Error { message } => {
                        panic!("root failed while consuming workflow result: {message}")
                    }
                    _ => {}
                }
            }
        })
        .await
        .expect("workflow result root turn timed out")
    }

    pub fn persisted_workflow_results(
        &self,
        run_id: &WorkflowRunId,
    ) -> Vec<PersistedWorkflowResult> {
        TurnEventStore::with_base_dir(self._home.path().join(".loopal"))
            .load_turns(&self.session_id)
            .expect("load root turns")
            .into_iter()
            .filter_map(|turn| match turn.trigger {
                TurnTrigger::WorkflowResult {
                    session_id,
                    run_id: observed_run,
                    terminal_revision,
                    payload_digest,
                    state,
                    content,
                } if observed_run == run_id.as_str() => Some(PersistedWorkflowResult {
                    session_id,
                    run_id: observed_run,
                    terminal_revision,
                    payload_digest,
                    state,
                    content,
                }),
                _ => None,
            })
            .collect()
    }

    pub async fn wait_for_delivery_ack(
        &self,
        run_id: &WorkflowRunId,
    ) -> WorkflowTerminalDeliveryId {
        tokio::time::timeout(TIMEOUT, async {
            loop {
                if let Some(delivery) = self
                    .workflow_replay(run_id)
                    .delivery_acks
                    .into_iter()
                    .find(|delivery| &delivery.run_id == run_id)
                {
                    return delivery;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("workflow delivery acknowledgement timed out")
    }

    pub async fn wait_for_mock_calls(&self, expected: usize) -> serde_json::Value {
        let result = tokio::time::timeout(TIMEOUT, async {
            loop {
                let journal = self.journal().await;
                if journal
                    .as_array()
                    .is_some_and(|calls| calls.len() >= expected)
                {
                    return journal;
                }
                tokio::task::yield_now().await;
            }
        })
        .await;
        match result {
            Ok(journal) => journal,
            Err(_) => {
                let journal = self.journal().await;
                let summary = journal
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|call| {
                        let label = call
                            .get("callLabel")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("<unmatched>");
                        let matched = call
                            .get("matched")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(false);
                        format!("{label}:{matched}")
                    })
                    .collect::<Vec<_>>();
                panic!(
                    "mock LLM calls timed out waiting for {expected}; observed calls: {summary:?}"
                );
            }
        }
    }

    pub fn remove_delivery_ack(&self, run_id: &WorkflowRunId) {
        let sessions = SessionStore::with_base_dir(self._home.path().join(".loopal"));
        let path = sessions
            .workflow_journal_path(&self.session_id, run_id.as_str())
            .expect("workflow journal path");
        let contents = std::fs::read_to_string(&path).expect("read workflow journal");
        let retained = contents
            .lines()
            .filter(|line| !line.contains(r#""kind":"delivery_ack""#))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(path, format!("{retained}\n")).expect("stage missing delivery ACK");
    }
}
