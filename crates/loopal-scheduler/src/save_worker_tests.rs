use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use tokio::sync::{Mutex, mpsc, oneshot};

use crate::persistence::{PersistError, PersistedTask};
use crate::persistence_session::SessionScopedCronStorage;
use crate::save_worker::{SaveMessage, SaveRequest, save_worker_loop};

struct Probe {
    saves: Mutex<Vec<Vec<PersistedTask>>>,
    fail_next: AtomicBool,
}

impl Probe {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            saves: Mutex::new(Vec::new()),
            fail_next: AtomicBool::new(false),
        })
    }
    fn arm_failure(&self) {
        self.fail_next.store(true, Ordering::SeqCst);
    }
    async fn save_count(&self) -> usize {
        self.saves.lock().await.len()
    }
}

#[async_trait]
impl SessionScopedCronStorage for Probe {
    async fn load(&self, _session_id: &str) -> Result<Vec<PersistedTask>, PersistError> {
        Ok(Vec::new())
    }
    async fn save_all(
        &self,
        _session_id: &str,
        tasks: &[PersistedTask],
    ) -> Result<(), PersistError> {
        if self.fail_next.swap(false, Ordering::SeqCst) {
            return Err(PersistError::Io(std::io::Error::other("armed failure")));
        }
        self.saves.lock().await.push(tasks.to_vec());
        Ok(())
    }
}

fn make_request(
    storage: Arc<dyn SessionScopedCronStorage>,
    dirty: Arc<AtomicBool>,
    store_disabled: Arc<AtomicBool>,
) -> SaveRequest {
    SaveRequest {
        storage,
        session_id: "test".into(),
        snapshot: Vec::new(),
        dirty,
        store_disabled,
    }
}

#[tokio::test]
async fn barrier_orders_after_pending_saves() {
    let probe = Probe::new();
    let (tx, rx) = mpsc::channel::<SaveMessage>(16);
    tokio::spawn(save_worker_loop(rx));

    let dirty = Arc::new(AtomicBool::new(false));
    let store_disabled = Arc::new(AtomicBool::new(false));
    for _ in 0..3 {
        let req = make_request(probe.clone(), dirty.clone(), store_disabled.clone());
        tx.send(SaveMessage::Save(req)).await.unwrap();
    }
    let (ack_tx, ack_rx) = oneshot::channel();
    tx.send(SaveMessage::Barrier(ack_tx)).await.unwrap();
    ack_rx.await.unwrap();

    // All three saves are guaranteed processed before barrier acks.
    assert_eq!(probe.save_count().await, 3);
    drop(tx);
}

#[tokio::test]
async fn save_failure_sets_dirty() {
    let probe = Probe::new();
    probe.arm_failure();
    let (tx, rx) = mpsc::channel::<SaveMessage>(4);
    tokio::spawn(save_worker_loop(rx));

    let dirty = Arc::new(AtomicBool::new(false));
    let store_disabled = Arc::new(AtomicBool::new(false));
    let req = make_request(probe.clone(), dirty.clone(), store_disabled.clone());
    tx.send(SaveMessage::Save(req)).await.unwrap();

    let (ack_tx, ack_rx) = oneshot::channel();
    tx.send(SaveMessage::Barrier(ack_tx)).await.unwrap();
    ack_rx.await.unwrap();

    assert!(dirty.load(Ordering::Acquire), "failure must latch dirty");
    assert_eq!(
        probe.save_count().await,
        0,
        "save_all errored before recording"
    );
    drop(tx);
}

#[tokio::test]
async fn save_success_clears_dirty() {
    let probe = Probe::new();
    let (tx, rx) = mpsc::channel::<SaveMessage>(4);
    tokio::spawn(save_worker_loop(rx));

    let dirty = Arc::new(AtomicBool::new(true));
    let store_disabled = Arc::new(AtomicBool::new(false));
    let req = make_request(probe.clone(), dirty.clone(), store_disabled.clone());
    tx.send(SaveMessage::Save(req)).await.unwrap();

    let (ack_tx, ack_rx) = oneshot::channel();
    tx.send(SaveMessage::Barrier(ack_tx)).await.unwrap();
    ack_rx.await.unwrap();

    assert!(!dirty.load(Ordering::Acquire), "success must clear dirty");
    drop(tx);
}

#[tokio::test]
async fn store_disabled_skips_save() {
    let probe = Probe::new();
    let (tx, rx) = mpsc::channel::<SaveMessage>(4);
    tokio::spawn(save_worker_loop(rx));

    let dirty = Arc::new(AtomicBool::new(true)); // sentinel: should not be flipped
    let store_disabled = Arc::new(AtomicBool::new(true));
    let req = make_request(probe.clone(), dirty.clone(), store_disabled.clone());
    tx.send(SaveMessage::Save(req)).await.unwrap();

    let (ack_tx, ack_rx) = oneshot::channel();
    tx.send(SaveMessage::Barrier(ack_tx)).await.unwrap();
    ack_rx.await.unwrap();

    assert_eq!(
        probe.save_count().await,
        0,
        "disabled store must not be hit"
    );
    assert!(
        dirty.load(Ordering::Acquire),
        "dirty must not be cleared when save was skipped"
    );
    drop(tx);
}

#[tokio::test]
async fn worker_exits_when_channel_closes() {
    let (tx, rx) = mpsc::channel::<SaveMessage>(4);
    let handle = tokio::spawn(save_worker_loop(rx));
    drop(tx);
    // Worker should exit promptly once the sender drops.
    tokio::time::timeout(std::time::Duration::from_secs(1), handle)
        .await
        .expect("worker did not exit after channel close")
        .expect("worker task panicked");
}
