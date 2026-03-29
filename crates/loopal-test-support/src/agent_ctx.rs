//! Lightweight `ToolContext` + `AgentShared` builder for agent tool unit tests.
//!
//! Usage:
//! ```ignore
//! let fixture = TestFixture::new();
//! let (ctx, shared) = agent_tool_context(&fixture);
//! let result = MyCoolTool.execute(input, &ctx).await.unwrap();
//! ```

use std::sync::Arc;

use loopal_agent::shared::{AgentShared, SchedulerHandle};
use loopal_agent::task_store::TaskStore;
use loopal_config::Settings;
use loopal_kernel::Kernel;
use loopal_scheduler::CronScheduler;
use loopal_tool_api::ToolContext;
use tokio_util::sync::CancellationToken;

use crate::fixture::TestFixture;

/// Create a `ToolContext` backed by a real `AgentShared` for testing agent tools.
///
/// The returned `Arc<AgentShared>` can be used for post-execution assertions
/// (e.g., checking the scheduler's task list after a CronCreate call).
pub fn agent_tool_context(fixture: &TestFixture) -> (ToolContext, Arc<AgentShared>) {
    agent_tool_context_inner(fixture, Arc::new(CronScheduler::new()))
}

/// Variant with a custom `CronScheduler` (e.g., one using `ManualClock`).
pub fn agent_tool_context_with_scheduler(
    fixture: &TestFixture,
    scheduler: Arc<CronScheduler>,
) -> (ToolContext, Arc<AgentShared>) {
    agent_tool_context_inner(fixture, scheduler)
}

fn agent_tool_context_inner(
    fixture: &TestFixture,
    scheduler: Arc<CronScheduler>,
) -> (ToolContext, Arc<AgentShared>) {
    let mut kernel = Kernel::new(Settings::default()).unwrap();
    loopal_agent::tools::register_all(&mut kernel);
    let kernel = Arc::new(kernel);

    let cwd = fixture
        .path()
        .canonicalize()
        .unwrap_or_else(|_| fixture.path().to_path_buf());

    let backend = loopal_backend::LocalBackend::new(
        cwd.clone(),
        None,
        loopal_backend::ResourceLimits::default(),
    );

    let (hub_conn, _hub_peer) = loopal_ipc::duplex_pair();
    let hub_connection = Arc::new(loopal_ipc::Connection::new(hub_conn));

    let tasks_dir = fixture.path().join("tasks");
    let cancel = CancellationToken::new();
    let scheduler_handle = SchedulerHandle::new(scheduler, cancel);

    let shared = Arc::new(AgentShared {
        kernel,
        task_store: Arc::new(TaskStore::new(tasks_dir)),
        hub_connection,
        cwd,
        depth: 0,
        max_depth: 3,
        agent_name: "test".into(),
        parent_event_tx: None,
        cancel_token: None,
        scheduler_handle,
    });

    let shared_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(shared.clone());
    let ctx = ToolContext {
        backend,
        session_id: "test-session".into(),
        shared: Some(shared_any),
        memory_channel: None,
        output_tail: None,
    };

    (ctx, shared)
}
