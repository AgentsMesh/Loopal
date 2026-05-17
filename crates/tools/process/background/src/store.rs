use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use loopal_protocol::BgTaskSnapshot;
use loopal_tool_api::BgTaskConfig;
use parking_lot::Mutex as PlMutex;
use tokio::sync::{broadcast, oneshot};

use crate::control::{ControlSignal, StatusFilter, StopOutcome, StoreError, TaskStatus};
use crate::task::BackgroundTask;

// reason: bridge reconciles on Lagged, so this is a soft cap not a correctness bound.
const SPAWN_BROADCAST_CAP: usize = 64;

// reason: at most one stop in flight + retry headroom; cheap channel.
pub(crate) const CONTROL_QUEUE_CAP: usize = 4;

#[derive(Debug, Clone)]
pub struct SpawnNotification {
    pub task_id: String,
    pub description: String,
    pub created_at_unix_ms: u64,
}

pub struct BackgroundTaskStore {
    tasks: PlMutex<HashMap<String, BackgroundTask>>,
    counter: AtomicU64,
    spawn_tx: broadcast::Sender<SpawnNotification>,
    config: BgTaskConfig,
}

impl BackgroundTaskStore {
    pub fn new() -> Arc<Self> {
        Self::with_config(BgTaskConfig::default())
    }

    pub fn with_config(config: BgTaskConfig) -> Arc<Self> {
        let (spawn_tx, _) = broadcast::channel(SPAWN_BROADCAST_CAP);
        Arc::new(Self {
            tasks: PlMutex::new(HashMap::new()),
            counter: AtomicU64::new(1),
            spawn_tx,
            config,
        })
    }

    pub fn config(&self) -> &BgTaskConfig {
        &self.config
    }

    pub fn generate_task_id(&self) -> String {
        format!("bg_{}", self.counter.fetch_add(1, Ordering::Relaxed))
    }

    pub fn subscribe_spawns(&self) -> broadcast::Receiver<SpawnNotification> {
        self.spawn_tx.subscribe()
    }

    pub(crate) fn insert(&self, task: BackgroundTask) {
        let notif = SpawnNotification {
            task_id: task.common.id.clone(),
            description: task.common.description.clone(),
            created_at_unix_ms: task.common.created_at_unix_ms,
        };
        let id = task.common.id.clone();
        self.tasks.lock().insert(id, task);
        let _ = self.spawn_tx.send(notif);
    }

    pub fn read_task<R>(&self, id: &str, f: impl FnOnce(&BackgroundTask) -> R) -> Option<R> {
        self.tasks.lock().get(id).map(f)
    }

    pub fn send_stop(&self, id: &str, ack: oneshot::Sender<StopOutcome>) -> Result<(), StoreError> {
        let guard = self.tasks.lock();
        let Some(task) = guard.get(id) else {
            return Err(StoreError::NotFound);
        };
        let status = task.status();
        if status.is_terminal() {
            return Err(StoreError::AlreadyTerminal {
                status,
                exit_code: task.exit_code(),
            });
        }
        let tx = task.control_tx.clone();
        drop(guard);
        tx.try_send(ControlSignal::Stop { ack })
            .map_err(|_| StoreError::ChannelClosed)
    }

    pub fn snapshot(&self, filter: StatusFilter) -> Vec<BgTaskSnapshot> {
        let guard = self.tasks.lock();
        let mut out: Vec<BgTaskSnapshot> = guard
            .values()
            .filter_map(|t| {
                let status = t.status();
                let pass = match filter {
                    StatusFilter::Running => status == TaskStatus::Running,
                    StatusFilter::Terminal => status.is_terminal(),
                    StatusFilter::All => true,
                };
                if !pass {
                    return None;
                }
                Some(BgTaskSnapshot {
                    id: t.id().to_string(),
                    description: t.description().to_string(),
                    status: status.to_bg(),
                    exit_code: t.exit_code(),
                    created_at_unix_ms: t.created_at_unix_ms(),
                })
            })
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    pub fn evict_terminal(&self, older_than: Duration) -> usize {
        let now = Instant::now();
        let mut victims: Vec<PathBuf> = Vec::new();
        let removed_count;
        {
            let mut guard = self.tasks.lock();
            let before = guard.len();
            guard.retain(|_, task| {
                let keep =
                    !task.is_terminal() || now.duration_since(task.created_at()) < older_than;
                if !keep {
                    victims.push(task.log_path.clone());
                }
                keep
            });
            removed_count = before - guard.len();
        }
        // reason: unlink outside the lock; NotFound is benign (cleanup ran first).
        for p in victims {
            tokio::spawn(async move {
                if let Err(e) = tokio::fs::remove_file(&p).await
                    && e.kind() != std::io::ErrorKind::NotFound
                {
                    tracing::warn!(error = %e, path = %p.display(), "evict unlink failed");
                }
            });
        }
        removed_count
    }
}
