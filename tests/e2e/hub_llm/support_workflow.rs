use std::time::Duration;

use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_protocol::{
    AgentEvent, AgentEventPayload, Envelope, MessageSource, ROOT_AGENT_NAME, UserContent,
    WorkflowReduceOutcome, WorkflowRunId, WorkflowRunSnapshot, WorkflowRunSummary,
    reduce_workflow_event,
};
use loopal_storage::{SessionStore, WorkflowJournal, WorkflowJournalReplay};
use serde_json::{Value, json};

use super::hub::{HubEnv, HubHarness, TIMEOUT};

#[derive(Default, Debug)]
pub struct WorkflowTurnOutcome {
    pub summaries: Vec<WorkflowRunSummary>,
    pub error: Option<String>,
    pub events: Vec<String>,
}

impl HubHarness {
    pub async fn start_with_workflow(scenario: Value) -> Self {
        Self::start_with_workflow_env(HubEnv::new(), scenario).await
    }

    pub async fn start_with_workflow_env(env: HubEnv, scenario: Value) -> Self {
        write_settings(env.home.path());
        Self::launch(env, scenario, false).await
    }

    pub fn workflow_replay(&self, run_id: &WorkflowRunId) -> WorkflowJournalReplay {
        let sessions = SessionStore::with_base_dir(self._home.path().join(".loopal"));
        WorkflowJournal::from_session_store(&sessions, &self.session_id, run_id.clone())
            .expect("open workflow journal")
            .replay()
            .expect("replay workflow journal")
    }

    pub async fn workflow_turn(&mut self, text: &str) -> WorkflowTurnOutcome {
        let envelope = Envelope::new(
            MessageSource::Human,
            ROOT_AGENT_NAME,
            UserContent::text_only(text),
        );
        self.conn
            .send_request(
                methods::HUB_ROUTE.name,
                serde_json::to_value(&envelope).unwrap(),
            )
            .await
            .expect("hub/route workflow goal");

        let mut outcome = WorkflowTurnOutcome::default();
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_secs(10), self.rx.recv()).await {
                Ok(Some(Incoming::Notification { method, params }))
                    if method == methods::AGENT_EVENT.name =>
                {
                    let Ok(event) = serde_json::from_value::<AgentEvent>(params) else {
                        continue;
                    };
                    outcome.events.push(format!("{:?}", event.payload));
                    match event.payload {
                        AgentEventPayload::WorkflowRunChanged(summary) => {
                            let terminal = summary.state.is_terminal();
                            outcome.summaries.push(summary);
                            if terminal {
                                return outcome;
                            }
                        }
                        AgentEventPayload::Error { message }
                            if event.agent_name.as_ref().is_none_or(|agent| {
                                agent.is_local() && agent.agent == ROOT_AGENT_NAME
                            }) =>
                        {
                            outcome.error = Some(message);
                            return outcome;
                        }
                        _ => {}
                    }
                }
                Ok(Some(_)) | Err(_) => {}
                Ok(None) => return outcome,
            }
        }
        outcome
    }
}

pub fn replay_workflow(replay: WorkflowJournalReplay) -> WorkflowRunSnapshot {
    assert!(replay.torn_tail.is_none());
    let init = replay.init.expect("workflow journal init");
    let mut run = init.snapshot;
    for event in init
        .events
        .into_iter()
        .chain(replay.commits.into_iter().flat_map(|commit| commit.events))
    {
        run = match reduce_workflow_event(&run, &event, &TextOnly).unwrap() {
            WorkflowReduceOutcome::Applied(next) => *next,
            WorkflowReduceOutcome::IgnoredStale { .. } => panic!("journal contains stale event"),
        };
    }
    run
}

pub fn workflow_events(replay: &WorkflowJournalReplay) -> Vec<&loopal_protocol::WorkflowEvent> {
    replay
        .init
        .iter()
        .flat_map(|init| init.events.iter())
        .chain(
            replay
                .commits
                .iter()
                .flat_map(|commit| commit.events.iter()),
        )
        .collect()
}

struct TextOnly;

impl loopal_protocol::WorkflowJsonValidator for TextOnly {
    type Error = &'static str;

    fn validate(
        &self,
        _schema: &serde_json::Value,
        _value: &serde_json::Value,
    ) -> Result<(), Self::Error> {
        Err("JSON output is not used by this text workflow")
    }
}

fn write_settings(home: &std::path::Path) {
    let dir = home.join(".loopal");
    std::fs::create_dir_all(&dir).unwrap();
    let settings = json!({
        "workflow": {
            "policy": "proactive",
            "execution_enabled": true,
            "limits": {
                "max_nodes": 8,
                "max_parallel": 2,
                "max_attempts": 8,
                "max_output_bytes": 4096
            },
            "timing": {
                "run_deadline_secs": 60,
                "attempt_timeout_secs": 30,
                "cancel_grace_secs": 5,
                "recovery_grace_secs": 5
            }
        }
    });
    std::fs::write(
        dir.join("settings.json"),
        serde_json::to_vec_pretty(&settings).unwrap(),
    )
    .unwrap();
}
