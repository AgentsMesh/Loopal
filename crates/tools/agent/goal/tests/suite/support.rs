use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use loopal_error::{ProcessHandle, ToolIoError};
use loopal_protocol::{ThreadGoal, ThreadGoalStatus};
use loopal_tool_api::backend_types::{
    EditResult, ExecResult, FetchResult, FileInfo, GlobOptions, GlobSearchResult, GrepOptions,
    GrepSearchResult, LsResult, ReadResult, WriteResult,
};
use loopal_tool_api::{Backend, ExecOutcome, GoalSession, GoalSessionError, ToolContext};

pub struct PanicBackend;

#[async_trait]
impl Backend for PanicBackend {
    async fn read(&self, _: &str, _: usize, _: usize) -> Result<ReadResult, ToolIoError> {
        unimplemented!("backend not used in goal tool tests")
    }
    async fn write(&self, _: &str, _: &str) -> Result<WriteResult, ToolIoError> {
        unimplemented!()
    }
    async fn edit(&self, _: &str, _: &str, _: &str, _: bool) -> Result<EditResult, ToolIoError> {
        unimplemented!()
    }
    async fn remove(&self, _: &str) -> Result<(), ToolIoError> {
        unimplemented!()
    }
    async fn create_dir_all(&self, _: &str) -> Result<(), ToolIoError> {
        unimplemented!()
    }
    async fn copy(&self, _: &str, _: &str) -> Result<(), ToolIoError> {
        unimplemented!()
    }
    async fn rename(&self, _: &str, _: &str) -> Result<(), ToolIoError> {
        unimplemented!()
    }
    async fn file_info(&self, _: &str) -> Result<FileInfo, ToolIoError> {
        unimplemented!()
    }
    async fn ls(&self, _: &str) -> Result<LsResult, ToolIoError> {
        unimplemented!()
    }
    async fn glob(&self, _: &GlobOptions) -> Result<GlobSearchResult, ToolIoError> {
        unimplemented!()
    }
    async fn grep(&self, _: &GrepOptions) -> Result<GrepSearchResult, ToolIoError> {
        unimplemented!()
    }
    fn resolve_path(&self, _: &str, _: bool) -> Result<PathBuf, ToolIoError> {
        unimplemented!()
    }
    async fn read_raw(&self, _: &str) -> Result<String, ToolIoError> {
        unimplemented!()
    }
    fn cwd(&self) -> &Path {
        Path::new("/tmp")
    }
    async fn exec(&self, _: &str, _: Duration) -> Result<ExecResult, ToolIoError> {
        unimplemented!()
    }
    async fn exec_streaming(
        &self,
        _: &str,
        _: Duration,
        _: Arc<loopal_tool_api::output_tail::OutputTail>,
    ) -> Result<ExecOutcome, ToolIoError> {
        unimplemented!()
    }
    async fn exec_background(&self, _: &str) -> Result<ProcessHandle, ToolIoError> {
        unimplemented!()
    }
    async fn fetch(&self, _: &str) -> Result<FetchResult, ToolIoError> {
        unimplemented!()
    }
}

#[derive(Default)]
pub struct FakeGoalSession {
    pub goal: Mutex<Option<ThreadGoal>>,
}

impl FakeGoalSession {
    pub fn empty() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn with_active(objective: &str, budget: Option<u64>) -> Arc<Self> {
        let mut g = ThreadGoal::new("test", objective);
        g.token_budget = budget;
        Arc::new(Self {
            goal: Mutex::new(Some(g)),
        })
    }
}

#[async_trait]
impl GoalSession for FakeGoalSession {
    async fn snapshot(&self) -> Result<Option<ThreadGoal>, GoalSessionError> {
        Ok(self.goal.lock().unwrap().clone())
    }

    async fn create(
        &self,
        objective: String,
        token_budget: Option<u64>,
    ) -> Result<ThreadGoal, GoalSessionError> {
        let mut slot = self.goal.lock().unwrap();
        if slot.is_some() {
            return Err(GoalSessionError::AlreadyExists);
        }
        if matches!(token_budget, Some(0)) {
            return Err(GoalSessionError::InvalidBudget);
        }
        let mut g = ThreadGoal::new("test", objective);
        g.token_budget = token_budget;
        *slot = Some(g.clone());
        Ok(g)
    }

    async fn complete_by_model(&self) -> Result<ThreadGoal, GoalSessionError> {
        let mut slot = self.goal.lock().unwrap();
        match slot.as_mut() {
            Some(g) if g.status == ThreadGoalStatus::Active => {
                g.status = ThreadGoalStatus::Complete;
                Ok(g.clone())
            }
            Some(_) => Err(GoalSessionError::ModelStatusForbidden),
            None => Err(GoalSessionError::NotFound),
        }
    }
}

pub fn ctx_with_goal_session(session: Arc<dyn GoalSession>) -> ToolContext {
    ToolContext::new(Arc::new(PanicBackend), "test").with_goal_session(session)
}

pub fn ctx_without_goal_session() -> ToolContext {
    ToolContext::new(Arc::new(PanicBackend), "test")
}
