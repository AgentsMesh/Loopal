use std::time::Instant;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use loopal_protocol::{
    AgentStateSnapshot, BgTaskDetail, BgTaskSnapshot, BgTaskStatus, CronJobSnapshot,
    McpServerSnapshot, ObservableAgentState, TaskSnapshot,
};

use crate::conversation::AgentConversation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionViewState {
    pub agent: AgentView,
    pub tasks: Vec<TaskSnapshot>,
    pub crons: Vec<CronJobSnapshot>,
    pub bg_tasks: IndexMap<String, BgTaskView>,
    pub mcp_status: Option<Vec<McpServerSnapshot>>,
}

impl SessionViewState {
    pub fn empty(agent_name: impl Into<String>) -> Self {
        Self {
            agent: AgentView::new(agent_name),
            tasks: Vec::new(),
            crons: Vec::new(),
            bg_tasks: IndexMap::new(),
            mcp_status: None,
        }
    }

    pub fn from_snapshot(agent_name: impl Into<String>, snapshot: AgentStateSnapshot) -> Self {
        let bg_tasks = snapshot
            .bg_tasks
            .into_iter()
            .map(|s| (s.id.clone(), BgTaskView::from_snapshot(s)))
            .collect();
        Self {
            agent: AgentView::new(agent_name),
            tasks: snapshot.tasks,
            crons: snapshot.crons,
            bg_tasks,
            mcp_status: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentView {
    pub name: String,
    pub session_id: Option<String>,
    pub observable: ObservableAgentState,
    pub children: Vec<String>,
    pub parent: Option<String>,
    #[serde(skip)]
    pub started_at: Option<Instant>,
    pub conversation: AgentConversation,
}

impl AgentView {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            session_id: None,
            observable: ObservableAgentState::default(),
            children: Vec::new(),
            parent: None,
            started_at: None,
            conversation: AgentConversation::default(),
        }
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.started_at
            .map_or(std::time::Duration::ZERO, |t| t.elapsed())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BgTaskView {
    pub id: String,
    pub description: String,
    pub status: BgTaskStatus,
    pub exit_code: Option<i32>,
    pub output: String,
}

impl BgTaskView {
    pub fn from_snapshot(s: BgTaskSnapshot) -> Self {
        Self {
            id: s.id,
            description: s.description,
            status: s.status,
            exit_code: s.exit_code,
            output: String::new(),
        }
    }

    pub fn from_detail(d: BgTaskDetail) -> Self {
        Self {
            id: d.id,
            description: d.description,
            status: d.status,
            exit_code: d.exit_code,
            output: d.output,
        }
    }
}
