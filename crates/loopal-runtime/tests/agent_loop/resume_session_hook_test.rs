//! Tests that `handle_resume_session` invokes registered
//! `SessionResumeHook` adapters and aggregates their failures into a
//! `SessionResumeWarnings` event.
//!
//! Drives `handle_resume_session` directly to keep the test
//! deterministic — exercising the channel-select `wait_for_input` loop
//! introduces unrelated timing variables.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use loopal_protocol::{AgentEventPayload, ControlCommand};
use loopal_runtime::{
    AgentConfig, AgentDeps, AgentLoopParamsBuilder, InterruptHandle, SessionResumeError,
    SessionResumeHook, UnifiedFrontend,
    agent_loop::AgentLoopRunner,
    frontend::{AutoCancelQuestionHandler, RelayPermissionHandler},
};
use loopal_test_support::TestFixture;
use tokio::sync::mpsc;
use tokio::time::{Duration, timeout};

/// Records every call. Optionally fails on the first invocation so we
/// can assert the warning aggregation path without disturbing later
/// adapters.
struct CountingHook {
    name: &'static str,
    calls: AtomicUsize,
    fail_first_call: bool,
}

impl CountingHook {
    fn ok(name: &'static str) -> Arc<Self> {
        Arc::new(Self {
            name,
            calls: AtomicUsize::new(0),
            fail_first_call: false,
        })
    }
    fn flaky(name: &'static str) -> Arc<Self> {
        Arc::new(Self {
            name,
            calls: AtomicUsize::new(0),
            fail_first_call: true,
        })
    }
    fn count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl SessionResumeHook for CountingHook {
    async fn on_session_changed(&self, _new: &str) -> Result<(), SessionResumeError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if self.fail_first_call && n == 1 {
            return Err(SessionResumeError::new(self.name, "armed failure"));
        }
        Ok(())
    }
}

struct Harness {
    runner: AgentLoopRunner,
    event_rx: mpsc::Receiver<loopal_protocol::AgentEvent>,
    target_id: String,
    _ctrl_tx: mpsc::Sender<ControlCommand>,
    _fixture: TestFixture,
}

fn build_harness(hooks: Vec<Arc<dyn SessionResumeHook>>) -> Harness {
    let fixture = TestFixture::new();
    let primary = fixture.test_session("primary");
    fixture
        .session_manager()
        .create_session(std::path::Path::new(&primary.cwd), &primary.model)
        .ok();
    let target = fixture
        .session_manager()
        .create_session(
            std::path::Path::new(&primary.cwd),
            "claude-sonnet-4-20250514",
        )
        .expect("create target session");
    let (event_tx, event_rx) = mpsc::channel(32);
    let (_mbox_tx, mailbox_rx) = mpsc::channel(16);
    let (ctrl_tx, control_rx) = mpsc::channel(16);
    let (_perm_tx, permission_rx) = mpsc::channel::<bool>(16);
    let frontend = Arc::new(UnifiedFrontend::new(
        None,
        event_tx.clone(),
        mailbox_rx,
        control_rx,
        None,
        Box::new(RelayPermissionHandler::new(event_tx, permission_rx)),
        Box::new(AutoCancelQuestionHandler),
    ));
    let kernel = Arc::new(loopal_kernel::Kernel::new(Default::default()).unwrap());
    let params = AgentLoopParamsBuilder::new(
        AgentConfig::default(),
        AgentDeps {
            kernel,
            frontend,
            session_manager: fixture.session_manager(),
        },
        primary,
        loopal_context::ContextStore::new(super::make_test_budget()),
        InterruptHandle::new(),
    )
    .resume_hooks(hooks)
    .build();
    Harness {
        runner: AgentLoopRunner::new(params),
        event_rx,
        target_id: target.id,
        _ctrl_tx: ctrl_tx,
        _fixture: fixture,
    }
}

async fn drain_until<F>(
    rx: &mut mpsc::Receiver<loopal_protocol::AgentEvent>,
    mut matches: F,
) -> Option<AgentEventPayload>
where
    F: FnMut(&AgentEventPayload) -> bool,
{
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return None;
        }
        match timeout(remaining, rx.recv()).await {
            Ok(Some(evt)) if matches(&evt.payload) => return Some(evt.payload),
            Ok(Some(_)) => continue,
            _ => return None,
        }
    }
}

#[tokio::test]
async fn resume_invokes_every_hook_in_order() {
    let hook_a = CountingHook::ok("hook-a");
    let hook_b = CountingHook::ok("hook-b");
    let hooks: Vec<Arc<dyn SessionResumeHook>> = vec![hook_a.clone(), hook_b.clone()];
    let mut h = build_harness(hooks);
    h.runner
        .handle_resume_session(&h.target_id)
        .await
        .expect("resume must succeed");
    assert_eq!(hook_a.count(), 1, "hook A must fire once");
    assert_eq!(hook_b.count(), 1, "hook B must fire once");
    let payload = drain_until(&mut h.event_rx, |p| {
        matches!(p, AgentEventPayload::SessionResumed { .. })
    })
    .await
    .expect("SessionResumed must be emitted");
    let AgentEventPayload::SessionResumed { session_id, .. } = payload else {
        unreachable!();
    };
    assert_eq!(session_id, h.target_id);
}

#[tokio::test]
async fn hook_failure_emits_warnings_event_and_subsequent_hooks_still_fire() {
    let failing = CountingHook::flaky("hook-flaky");
    let succeeding = CountingHook::ok("hook-ok");
    let hooks: Vec<Arc<dyn SessionResumeHook>> = vec![failing.clone(), succeeding.clone()];
    let mut h = build_harness(hooks);
    h.runner
        .handle_resume_session(&h.target_id)
        .await
        .expect("resume must not abort on hook failure");
    assert_eq!(failing.count(), 1);
    assert_eq!(succeeding.count(), 1, "subsequent hooks must still fire");
    let payload = drain_until(&mut h.event_rx, |p| {
        matches!(p, AgentEventPayload::SessionResumeWarnings { .. })
    })
    .await
    .expect("SessionResumeWarnings must be emitted");
    let AgentEventPayload::SessionResumeWarnings { warnings, .. } = payload else {
        unreachable!();
    };
    assert_eq!(warnings.len(), 1);
    assert!(
        warnings[0].contains("hook-flaky"),
        "warning text must name the failing hook: {warnings:?}"
    );
}

#[tokio::test]
async fn no_warnings_event_when_all_hooks_succeed() {
    let only = CountingHook::ok("only");
    let mut h = build_harness(vec![only.clone() as Arc<dyn SessionResumeHook>]);
    h.runner.handle_resume_session(&h.target_id).await.unwrap();
    while let Ok(Some(evt)) = timeout(Duration::from_millis(100), h.event_rx.recv()).await {
        assert!(
            !matches!(evt.payload, AgentEventPayload::SessionResumeWarnings { .. }),
            "no warnings expected when every hook succeeds"
        );
    }
}

#[tokio::test]
async fn resume_clears_pending_inbox_consumed_ids_from_previous_session() {
    let mut h = build_harness(Vec::new());
    h.runner
        .pending_consumed_ids
        .push("stale-id-from-old-session".into());
    h.runner
        .handle_resume_session(&h.target_id)
        .await
        .expect("resume must succeed");
    assert!(
        h.runner.pending_consumed_ids.is_empty(),
        "resume must drop pending ids; otherwise next turn emits ghost InboxConsumed"
    );
}

#[tokio::test]
async fn empty_hooks_emits_no_warning() {
    let mut h = build_harness(Vec::new());
    h.runner.handle_resume_session(&h.target_id).await.unwrap();
    while let Ok(Some(evt)) = timeout(Duration::from_millis(100), h.event_rx.recv()).await {
        assert!(
            !matches!(evt.payload, AgentEventPayload::SessionResumeWarnings { .. }),
            "no warnings when there are no hooks at all"
        );
    }
}
