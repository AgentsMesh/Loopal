//! Durable storage for cron tasks — types, trait, and serialization.
//!
//! Only tasks marked `durable = true` are persisted. The scheduler
//! writes the full durable-task set on every mutation (add / remove /
//! tick-fired / expired) so the on-disk file always reflects the
//! latest scheduler state. This keeps the implementation trivially
//! correct at the cost of rewriting a small JSON document (≤50 tasks)
//! per mutation — acceptable for a feature that fires at most 50
//! times a day.
//!
//! A crash window between the in-memory mutation and a successful
//! `save_all` can cause a one-shot durable task to be re-fired
//! exactly once after restart — see [`CronScheduler`](crate::CronScheduler)
//! docs.
//!
//! The file-backed implementation lives in [`crate::persistence_file`].
//! Scheduler integration (persist_locked / load_persisted) lives in
//! [`crate::scheduler_persistence`].

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::io;

use crate::expression::CronExpression;
use crate::task::ScheduledTask;

/// Current on-disk schema version. Bump if the layout changes
/// incompatibly; `load` tolerates unknown future versions by refusing
/// rather than silently misreading.
pub const SCHEMA_VERSION: u32 = 1;

/// Errors surfaced from a [`DurableStore`] implementation.
#[derive(Debug, thiserror::Error)]
pub enum PersistError {
    #[error("durable store i/o: {0}")]
    Io(#[from] io::Error),
    #[error("durable store serde: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("unsupported schema version {0}")]
    UnsupportedVersion(u32),
    #[error("cron expression in durable file is invalid: {0}")]
    BadCron(String),
}

// ---------------------------------------------------------------------------
// On-disk shape
// ---------------------------------------------------------------------------

/// Serializable form of a [`ScheduledTask`]. Uses Unix ms timestamps to
/// avoid requiring chrono's serde feature.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistedTask {
    pub id: String,
    pub cron: String,
    pub prompt: String,
    pub recurring: bool,
    pub created_at_unix_ms: i64,
    #[serde(default)]
    pub last_fired_unix_ms: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PersistedFile {
    pub(crate) version: u32,
    pub(crate) tasks: Vec<PersistedTask>,
}

impl PersistedTask {
    /// Build from an in-memory task. Caller ensures `durable == true`.
    pub(crate) fn from_task(task: &ScheduledTask) -> Self {
        Self {
            id: task.id.clone(),
            cron: task.cron.as_str().to_string(),
            prompt: task.prompt.clone(),
            recurring: task.recurring,
            created_at_unix_ms: task.created_at.timestamp_millis(),
            last_fired_unix_ms: task.last_fired.map(|t| t.timestamp_millis()),
        }
    }

    /// Reconstruct an in-memory task. `durable` is always `true` for
    /// entries coming off disk.
    ///
    /// `parse_reference` is the clock value used when revalidating the
    /// cron expression. It must be "now" (not the persisted
    /// `created_at`), otherwise an expression like `"0 9 * * *"` saved
    /// 2.9 days ago would fail to parse as "no occurrence within the
    /// 3-day lifetime from created_at" even though it has many
    /// occurrences going forward.
    pub(crate) fn into_task(
        self,
        parse_reference: DateTime<Utc>,
    ) -> Result<ScheduledTask, PersistError> {
        let created_at = unix_ms_to_utc(self.created_at_unix_ms);
        let cron = CronExpression::parse_at(&self.cron, parse_reference)
            .map_err(|e| PersistError::BadCron(format!("{e}")))?;
        Ok(ScheduledTask {
            id: self.id,
            cron,
            prompt: self.prompt,
            recurring: self.recurring,
            created_at,
            last_fired: self.last_fired_unix_ms.map(unix_ms_to_utc),
            durable: true,
        })
    }
}

fn unix_ms_to_utc(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms)
        .single()
        .unwrap_or_else(Utc::now)
}

// ---------------------------------------------------------------------------
// Store trait
// ---------------------------------------------------------------------------

/// Abstract storage for durable cron tasks.
///
/// Implementations must be safe to share via `Arc` and callable from
/// any task. `save_all` fully replaces the stored set.
#[async_trait]
pub trait DurableStore: Send + Sync {
    /// Load all previously persisted tasks. A missing file returns an
    /// empty vector — first-ever use is not an error.
    async fn load(&self) -> Result<Vec<PersistedTask>, PersistError>;

    /// Replace the on-disk set with `tasks`. Must be atomic: a reader
    /// either sees the previous contents or the new contents, never
    /// partial.
    async fn save_all(&self, tasks: &[PersistedTask]) -> Result<(), PersistError>;
}

/// Build the durable subset of `tasks` as [`PersistedTask`] entries.
pub(crate) fn durable_snapshot(tasks: &[ScheduledTask]) -> Vec<PersistedTask> {
    tasks
        .iter()
        .filter(|t| t.durable)
        .map(PersistedTask::from_task)
        .collect()
}
