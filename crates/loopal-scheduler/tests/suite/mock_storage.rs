//! Shared mock `SessionScopedCronStorage` for scheduler tests.
//!
//! One `MockStorage` covers every test-time fault-injection axis:
//! - seed per-session preset for `load`
//! - record save calls (per-session + chronological)
//! - arm a single save failure (any session or specific session)
//! - arm load failure (permanent or once for a specific session)

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use async_trait::async_trait;
use tokio::sync::Mutex;

use loopal_scheduler::{PersistError, PersistedTask, SessionScopedCronStorage};

pub struct MockStorage {
    state: Mutex<HashMap<String, Vec<PersistedTask>>>,
    saves: Mutex<Vec<(String, Vec<PersistedTask>)>>,
    fail_save_next: AtomicBool,
    fail_save_attempts: AtomicUsize,
    fail_save_for: Mutex<Option<String>>,
    fail_load_always: AtomicBool,
    fail_load_once_for: Mutex<Option<String>>,
}

impl MockStorage {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(HashMap::new()),
            saves: Mutex::new(Vec::new()),
            fail_save_next: AtomicBool::new(false),
            fail_save_attempts: AtomicUsize::new(0),
            fail_save_for: Mutex::new(None),
            fail_load_always: AtomicBool::new(false),
            fail_load_once_for: Mutex::new(None),
        })
    }

    pub async fn seed(&self, session_id: &str, tasks: Vec<PersistedTask>) {
        self.state.lock().await.insert(session_id.into(), tasks);
    }

    pub fn arm_save_failure(&self) {
        self.fail_save_next.store(true, Ordering::SeqCst);
    }

    pub async fn arm_save_failure_for(&self, session_id: &str) {
        *self.fail_save_for.lock().await = Some(session_id.into());
    }

    pub fn arm_load_failure_always(&self) {
        self.fail_load_always.store(true, Ordering::SeqCst);
    }

    pub async fn arm_load_failure_once_for(&self, session_id: &str) {
        *self.fail_load_once_for.lock().await = Some(session_id.into());
    }

    pub async fn save_count(&self) -> usize {
        self.saves.lock().await.len()
    }

    pub fn fail_save_attempts(&self) -> usize {
        self.fail_save_attempts.load(Ordering::SeqCst)
    }

    pub async fn saves_for(&self, session_id: &str) -> usize {
        self.saves
            .lock()
            .await
            .iter()
            .filter(|(s, _)| s == session_id)
            .count()
    }

    pub async fn last_save(&self) -> Vec<PersistedTask> {
        self.saves
            .lock()
            .await
            .last()
            .map(|(_, t)| t.clone())
            .unwrap_or_default()
    }

    pub async fn last_ids(&self) -> Vec<String> {
        self.last_save().await.into_iter().map(|t| t.id).collect()
    }
}

#[async_trait]
impl SessionScopedCronStorage for MockStorage {
    async fn load(&self, session_id: &str) -> Result<Vec<PersistedTask>, PersistError> {
        if self.fail_load_always.load(Ordering::SeqCst) {
            return Err(PersistError::Io(std::io::Error::other("unreadable")));
        }
        let armed = self.fail_load_once_for.lock().await.clone();
        if let Some(target) = armed
            && target == session_id
        {
            *self.fail_load_once_for.lock().await = None;
            return Err(PersistError::BadCron("forced".into()));
        }
        Ok(self
            .state
            .lock()
            .await
            .get(session_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn save_all(
        &self,
        session_id: &str,
        tasks: &[PersistedTask],
    ) -> Result<(), PersistError> {
        if self.fail_save_next.swap(false, Ordering::SeqCst) {
            self.fail_save_attempts.fetch_add(1, Ordering::SeqCst);
            return Err(PersistError::Io(std::io::Error::other("armed failure")));
        }
        let armed = self.fail_save_for.lock().await.clone();
        if let Some(target) = armed
            && target == session_id
        {
            *self.fail_save_for.lock().await = None;
            self.fail_save_attempts.fetch_add(1, Ordering::SeqCst);
            return Err(PersistError::Io(std::io::Error::other("forced")));
        }
        self.saves
            .lock()
            .await
            .push((session_id.into(), tasks.to_vec()));
        self.state
            .lock()
            .await
            .insert(session_id.into(), tasks.to_vec());
        Ok(())
    }
}
