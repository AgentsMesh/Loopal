//! Lock-protocol behavior of `memory_consolidation::trigger_consolidation`.
//! The lock keeps concurrent consolidations from spawning duplicate sub-agents
//! — when `.consolidation_lock` exists with a fresh timestamp, the function
//! must short-circuit without spawning.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use loopal_agent::shared::{AgentShared, SchedulerHandle};
use loopal_agent::task_store::TaskStore;
use loopal_agent_server::testing::trigger_consolidation;
use loopal_config::Settings;
use loopal_ipc::{Connection, Listening};
use loopal_kernel::Kernel;
use loopal_scheduler::CronScheduler;
use loopal_test_support::TestFixture;
use tokio_util::sync::CancellationToken;

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn build_shared(fixture: &TestFixture) -> Arc<AgentShared> {
    let kernel = Arc::new(Kernel::new(Settings::default()).unwrap());
    let cwd = fixture
        .path()
        .canonicalize()
        .unwrap_or_else(|_| fixture.path().to_path_buf());
    // Hub side dropped — spawn_agent will fail; that's intentional: we only
    // care about the lock-protocol observable in the lock-held branch, and
    // the spawn-failure path still exercises release_lock.
    let (conn, _peer) = loopal_test_support::make_duplex_pair();
    let (hub_connection, _rx) = Connection::new(conn).into_listening();
    let scheduler_handle =
        SchedulerHandle::new(Arc::new(CronScheduler::new()), CancellationToken::new());
    Arc::new(AgentShared {
        kernel,
        task_store: Arc::new(TaskStore::with_sessions_root(fixture.path().join("tasks"))),
        hub_connection,
        cwd,
        depth: 0,
        agent_name: "consolidation-test".into(),
        parent_event_tx: None,
        cancel_token: None,
        scheduler_handle,
        message_snapshot: Arc::new(std::sync::RwLock::new(Vec::new())),
        goal_session: None,
    })
}

#[tokio::test]
async fn trigger_consolidation_skips_when_fresh_lock_exists() {
    let fixture = TestFixture::new();
    let shared = build_shared(&fixture);
    let memory_dir = shared.cwd.join(".loopal/memory");
    std::fs::create_dir_all(&memory_dir).unwrap();

    // Pre-create a fresh lock (timestamp = now). trigger_consolidation must
    // refuse to acquire and return without spawning.
    let lock_path = memory_dir.join(".consolidation_lock");
    let original_ts = now_secs();
    std::fs::write(&lock_path, original_ts.to_string()).unwrap();

    trigger_consolidation(&shared, "test-model");

    // Lock unchanged: function early-returned, never wrote its own timestamp.
    let actual = std::fs::read_to_string(&lock_path).unwrap();
    assert_eq!(
        actual.trim(),
        original_ts.to_string(),
        "lock content must be unchanged when trigger short-circuited"
    );

    // .last_consolidation must NOT be touched: the success path never ran.
    assert!(
        !memory_dir.join(".last_consolidation").exists(),
        "marker file must not be written when trigger short-circuited"
    );
}

#[tokio::test]
async fn trigger_consolidation_acquires_lock_when_free() {
    let fixture = TestFixture::new();
    let shared = build_shared(&fixture);
    let memory_dir = shared.cwd.join(".loopal/memory");

    let lock_path = memory_dir.join(".consolidation_lock");
    assert!(
        !lock_path.exists(),
        "precondition: no lock prior to trigger"
    );

    // try_acquire_lock writes the lock file synchronously before tokio::spawn
    // returns. Read immediately so the spawn-failure path can't have released
    // it yet.
    trigger_consolidation(&shared, "test-model");
    assert!(
        lock_path.exists(),
        "trigger must acquire the lock synchronously before the spawned task can release it"
    );

    // Wait for the spawn-task to fail (hub side is dropped) and release the
    // lock via the warn-path. spawn_agent errors immediately on the closed
    // connection.
    let deadline = std::time::Instant::now() + Duration::from_secs(8);
    while std::time::Instant::now() < deadline {
        if !lock_path.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        !lock_path.exists(),
        "lock must be released after spawn_agent fails (connection dropped)"
    );
}

#[tokio::test]
async fn trigger_consolidation_skips_then_unlocked_caller_succeeds() {
    // Sequential trigger: first call holds, releases; second call sees a clean
    // dir again and acquires freshly.
    let fixture = TestFixture::new();
    let shared = build_shared(&fixture);
    let memory_dir = shared.cwd.join(".loopal/memory");
    let lock_path = memory_dir.join(".consolidation_lock");

    trigger_consolidation(&shared, "test-model");

    let deadline = std::time::Instant::now() + Duration::from_secs(8);
    while std::time::Instant::now() < deadline {
        if !lock_path.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(!lock_path.exists());

    trigger_consolidation(&shared, "test-model");
    assert!(
        lock_path.exists(),
        "second trigger must re-acquire the lock synchronously"
    );
}
