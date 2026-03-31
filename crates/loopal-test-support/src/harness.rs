//! Configurable integration test harness with correct channel wiring.
//!
//! Mirrors the production wiring in `bootstrap.rs` — SessionController holds
//! TX ends while UnifiedFrontend holds RX ends of the same channels.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::mpsc;

use loopal_config::HookConfig;
use loopal_error::LoopalError;
use loopal_kernel::Kernel;
use loopal_message::Message;
use loopal_protocol::{AgentEvent, ControlCommand, Envelope};
use loopal_provider_api::{StreamChunk, ThinkingConfig};
use loopal_runtime::AgentMode;
use loopal_runtime::agent_loop::AgentLoopRunner;
use loopal_session::SessionController;
use loopal_tool_api::PermissionMode;

use crate::fixture::TestFixture;

// ── Builder ────────────────────────────────────────────────────────

/// Configurable builder for integration test harnesses.
pub struct HarnessBuilder {
    pub(crate) calls: Vec<Vec<Result<StreamChunk, LoopalError>>>,
    pub(crate) model: String,
    pub(crate) summarization_model: Option<String>,
    pub(crate) permission_mode: PermissionMode,
    pub(crate) messages: Vec<Message>,
    pub(crate) max_turns: u32,
    pub(crate) mode: AgentMode,
    pub(crate) system_prompt: String,
    pub(crate) thinking_config: ThinkingConfig,
    pub(crate) tool_filter: Option<HashSet<String>>,
    pub(crate) hooks: Vec<HookConfig>,
    pub(crate) cwd: Option<PathBuf>,
    #[allow(clippy::type_complexity)]
    pub(crate) kernel_setup: Option<Box<dyn FnOnce(&mut Kernel)>>,
    pub(crate) scheduler: Option<Arc<loopal_scheduler::CronScheduler>>,
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
            max_turns: 10,
            mode: AgentMode::Act,
            system_prompt: "test".into(),
            thinking_config: ThinkingConfig::Auto,
            tool_filter: None,
            hooks: Vec::new(),
            cwd: None,
            kernel_setup: None,
            scheduler: None,
        }
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
        self
    }
    pub fn max_turns(mut self, t: u32) -> Self {
        self.max_turns = t;
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

    /// Build harness without spawning — caller drives `runner.run()`.
    pub async fn build(self) -> IntegrationHarness {
        let (harness, runner) = self.into_wired().await;
        IntegrationHarness::from_parts(harness, runner)
    }

    /// Build and spawn `agent_loop` in a background task.
    pub async fn build_spawned(self) -> SpawnedHarness {
        let (harness, runner) = self.into_wired().await;
        tokio::spawn(async move {
            let mut runner = runner;
            let _ = runner.run().await;
        });
        harness
    }

    async fn into_wired(self) -> (SpawnedHarness, AgentLoopRunner) {
        crate::wiring::wire(self).await
    }
}

// ── Output types ───────────────────────────────────────────────────

/// Harness with an unstarted `AgentLoopRunner`.
pub struct IntegrationHarness {
    pub runner: AgentLoopRunner,
    pub event_rx: mpsc::Receiver<AgentEvent>,
    pub mailbox_tx: mpsc::Sender<Envelope>,
    pub control_tx: mpsc::Sender<ControlCommand>,
    pub session_ctrl: SessionController,
    pub fixture: TestFixture,
}

impl IntegrationHarness {
    fn from_parts(h: SpawnedHarness, runner: AgentLoopRunner) -> Self {
        Self {
            runner,
            event_rx: h.event_rx,
            mailbox_tx: h.mailbox_tx,
            control_tx: h.control_tx,
            session_ctrl: h.session_ctrl,
            fixture: h.fixture,
        }
    }
}

/// Harness with `agent_loop` running in a background task.
pub struct SpawnedHarness {
    pub event_rx: mpsc::Receiver<AgentEvent>,
    pub mailbox_tx: mpsc::Sender<Envelope>,
    pub control_tx: mpsc::Sender<ControlCommand>,
    pub session_ctrl: SessionController,
    pub fixture: TestFixture,
}
