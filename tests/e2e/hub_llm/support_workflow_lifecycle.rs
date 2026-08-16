use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_protocol::{
    AgentEvent, AgentEventPayload, Envelope, MessageSource, ROOT_AGENT_NAME, UserContent,
    WorkflowRunId, WorkflowRunSummary,
};

use super::hub::{HubHarness, TIMEOUT};
use super::workflow::replay_workflow;

impl HubHarness {
    pub async fn wait_for_workflow_terminal(
        &mut self,
        run_id: &WorkflowRunId,
    ) -> WorkflowRunSummary {
        let recovered = replay_workflow(self.workflow_replay(run_id));
        if recovered.state.is_terminal() {
            return WorkflowRunSummary::from(&recovered);
        }
        tokio::time::timeout(TIMEOUT, async {
            loop {
                let event = self.next_agent_event().await;
                let root = is_root(&event);
                match event.payload {
                    AgentEventPayload::WorkflowRunChanged(summary)
                        if &summary.id == run_id && summary.state.is_terminal() =>
                    {
                        return summary;
                    }
                    AgentEventPayload::Error { message } if root => {
                        panic!("root failed while awaiting workflow recovery: {message}")
                    }
                    _ => {}
                }
            }
        })
        .await
        .expect("workflow recovery timed out")
    }

    pub async fn start_workflow_until_stream(
        &mut self,
        text: &str,
        worker_canary: &str,
        successful_tool_id: Option<&str>,
    ) -> WorkflowRunSummary {
        self.route_human(text).await;
        tokio::time::timeout(TIMEOUT, async {
            let mut latest = None;
            let mut tool_succeeded = successful_tool_id.is_none();
            loop {
                let event = self.next_agent_event().await;
                let root = is_root(&event);
                match event.payload {
                    AgentEventPayload::WorkflowRunChanged(summary) => {
                        assert!(
                            !summary.state.is_terminal(),
                            "workflow terminalized before worker stream: {summary:?}"
                        );
                        latest = Some(summary);
                    }
                    AgentEventPayload::ToolResult {
                        id,
                        result,
                        is_error,
                        ..
                    } if successful_tool_id == Some(id.as_str()) => {
                        assert!(!is_error, "workflow setup tool failed: {result}");
                        tool_succeeded = true;
                    }
                    AgentEventPayload::Stream { text }
                        if tool_succeeded && text.contains(worker_canary) =>
                    {
                        return latest.expect("workflow summary before worker stream");
                    }
                    AgentEventPayload::Error { message } if root => {
                        panic!("root failed while starting workflow: {message}")
                    }
                    _ => {}
                }
            }
        })
        .await
        .expect("worker stream timed out")
    }

    pub async fn cancel_workflow_turn(&mut self, run_id: &WorkflowRunId) -> WorkflowRunSummary {
        let envelope_id = self
            .route_human(&format!("Cancel workflow {run_id} now."))
            .await;
        tokio::time::timeout(TIMEOUT, async {
            let mut queued = false;
            let mut tool_completed = false;
            let mut root_settled = false;
            let mut terminal = None;
            loop {
                let event = self.next_agent_event().await;
                let root = is_root(&event);
                match event.payload {
                    AgentEventPayload::UserMessageQueued {
                        envelope_id: observed,
                        ..
                    } if root && observed == envelope_id => queued = true,
                    AgentEventPayload::ToolResult { id, name, .. }
                        if root && id == "cancel-1" && name == "workflow_cancel" =>
                    {
                        tool_completed = true;
                    }
                    AgentEventPayload::WorkflowRunChanged(summary)
                        if summary.id.as_str() == run_id.as_str()
                            && summary.state.is_terminal() =>
                    {
                        terminal = Some(summary);
                    }
                    AgentEventPayload::Finished | AgentEventPayload::AwaitingInput
                        if root && queued && tool_completed =>
                    {
                        root_settled = true;
                    }
                    AgentEventPayload::Error { message } if root && queued => {
                        panic!("root failed while cancelling workflow: {message}")
                    }
                    _ => {}
                }
                if root_settled && let Some(summary) = terminal.take() {
                    return summary;
                }
            }
        })
        .await
        .expect("workflow cancellation timed out")
    }

    pub async fn route_human(&self, text: &str) -> String {
        let envelope = Envelope::new(
            MessageSource::Human,
            ROOT_AGENT_NAME,
            UserContent::text_only(text),
        );
        let envelope_id = envelope.id.to_string();
        self.conn
            .send_request(
                methods::HUB_ROUTE.name,
                serde_json::to_value(&envelope).unwrap(),
            )
            .await
            .expect("hub/route workflow lifecycle message");
        envelope_id
    }

    pub(super) async fn next_agent_event(&mut self) -> AgentEvent {
        loop {
            match self.rx.recv().await.expect("hub event stream closed") {
                Incoming::Notification { method, params }
                    if method == methods::AGENT_EVENT.name =>
                {
                    if let Ok(event) = serde_json::from_value(params) {
                        return event;
                    }
                }
                _ => {}
            }
        }
    }
}

pub(super) fn is_root(event: &AgentEvent) -> bool {
    event
        .agent_name
        .as_ref()
        .is_none_or(|agent| agent.is_local() && agent.agent == ROOT_AGENT_NAME)
}
