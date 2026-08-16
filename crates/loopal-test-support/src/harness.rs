use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use loopal_config::HookConfig;
use loopal_error::LoopalError;
use loopal_kernel::Kernel;
use loopal_provider_api::Message;
use loopal_provider_api::{StreamChunk, ThinkingConfig};
use loopal_runtime::AgentMode;
use loopal_runtime::goal::GoalRuntimeSession;
use loopal_tool_api::{OutstandingTasksDigest, PermissionMode};

use crate::harness_types::{IntegrationHarness, SpawnedHarness};

pub struct HarnessBuilder {
    pub(crate) calls: Vec<Vec<Result<StreamChunk, LoopalError>>>,
    pub(crate) model: String,
    pub(crate) summarization_model: Option<String>,
    pub(crate) permission_mode: PermissionMode,
    pub(crate) messages: Vec<Message>,
    pub(crate) messages_pending: bool,
    pub(crate) mode: AgentMode,
    pub(crate) lifecycle: loopal_runtime::LifecycleMode,
    pub(crate) system_prompt: String,
    pub(crate) thinking_config: ThinkingConfig,
    pub(crate) tool_filter: Option<HashSet<String>>,
    pub(crate) hooks: Vec<HookConfig>,
    pub(crate) cwd: Option<PathBuf>,
    #[allow(clippy::type_complexity)]
    pub(crate) kernel_setup: Option<Box<dyn FnOnce(&mut Kernel)>>,
    pub(crate) scheduler: Option<Arc<loopal_scheduler::CronScheduler>>,
    pub(crate) goal_session: Option<Arc<GoalRuntimeSession>>,
    pub(crate) outstanding_tasks: Option<Arc<dyn OutstandingTasksDigest>>,
    pub(crate) llm_chunk_delay: Option<std::time::Duration>,
}

impl Default for HarnessBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl HarnessBuilder {
    pub fn new() -> Self {
        Self {
            calls: Vec::new(),
            model: "claude-sonnet-4-20250514".into(),
            summarization_model: None,
            permission_mode: PermissionMode::Bypass,
            messages: vec![Message::user("hello")],
            messages_pending: true,
            mode: AgentMode::Act,
            lifecycle: loopal_runtime::LifecycleMode::Ephemeral,
            system_prompt: "test".into(),
            thinking_config: ThinkingConfig::Auto,
            tool_filter: None,
            hooks: Vec::new(),
            cwd: None,
            kernel_setup: None,
            scheduler: None,
            goal_session: None,
            outstanding_tasks: None,
            llm_chunk_delay: None,
        }
    }

    pub fn outstanding_tasks(mut self, p: Arc<dyn OutstandingTasksDigest>) -> Self {
        self.outstanding_tasks = Some(p);
        self
    }

    pub fn calls(mut self, c: Vec<Vec<Result<StreamChunk, LoopalError>>>) -> Self {
        self.calls = c;
        self
    }
    pub fn model(mut self, m: impl Into<String>) -> Self {
        self.model = m.into();
        self
    }
    pub fn summarization_model(mut self, m: impl Into<String>) -> Self {
        self.summarization_model = Some(m.into());
        self
    }
    pub fn permission_mode(mut self, m: PermissionMode) -> Self {
        self.permission_mode = m;
        self
    }
    pub fn messages(mut self, m: Vec<Message>) -> Self {
        self.messages = m;
        self.messages_pending = false;
        self
    }
    pub fn prompt(mut self, m: Vec<Message>) -> Self {
        self.messages = m;
        self.messages_pending = true;
        self
    }
    pub fn mode(mut self, m: AgentMode) -> Self {
        self.mode = m;
        self
    }
    pub fn system_prompt(mut self, s: impl Into<String>) -> Self {
        self.system_prompt = s.into();
        self
    }
    pub fn thinking_config(mut self, c: ThinkingConfig) -> Self {
        self.thinking_config = c;
        self
    }
    pub fn tool_filter(mut self, f: HashSet<String>) -> Self {
        self.tool_filter = Some(f);
        self
    }
    pub fn hooks(mut self, h: Vec<HookConfig>) -> Self {
        self.hooks = h;
        self
    }
    pub fn cwd(mut self, path: impl Into<PathBuf>) -> Self {
        self.cwd = Some(path.into());
        self
    }
    pub fn kernel_setup(mut self, f: impl FnOnce(&mut Kernel) + 'static) -> Self {
        self.kernel_setup = Some(Box::new(f));
        self
    }
    pub fn scheduler(mut self, s: Arc<loopal_scheduler::CronScheduler>) -> Self {
        self.scheduler = Some(s);
        self
    }
    pub fn goal_session(mut self, g: Arc<GoalRuntimeSession>) -> Self {
        self.goal_session = Some(g);
        self
    }
    pub fn lifecycle(mut self, l: loopal_runtime::LifecycleMode) -> Self {
        self.lifecycle = l;
        self
    }
    /// Insert a per-chunk sleep into the mock LLM stream so that the turn
    /// stays in-flight long enough for a test to inject a control / signal
    /// an interrupt while the runner is streaming. Pair this with
    /// `wait_for_stream_event` (event-driven sync) rather than
    /// `tokio::time::sleep` for race-free test pacing.
    pub fn llm_chunk_delay(mut self, d: std::time::Duration) -> Self {
        self.llm_chunk_delay = Some(d);
        self
    }

    pub async fn build(self) -> IntegrationHarness {
        let (harness, runner) = self.into_wired().await;
        IntegrationHarness::from_parts(harness, runner)
    }

    pub async fn build_spawned(mut self) -> SpawnedHarness {
        self.lifecycle = loopal_runtime::LifecycleMode::Persistent;
        let (harness, runner) = self.into_wired().await;
        tokio::spawn(async move {
            let mut runner = runner;
            let _ = runner.run().await;
        });
        harness
    }

    async fn into_wired(self) -> (SpawnedHarness, loopal_runtime::agent_loop::AgentLoopRunner) {
        crate::wiring::wire(self).await
    }
}
