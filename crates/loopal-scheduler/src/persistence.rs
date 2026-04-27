//! Cron-task storage types — schema, persisted form, codec, errors.
//!
//! Each cron task marked with `durable = true` is persisted on every
//! mutation (add / remove / tick-fired / expired), so the on-disk file
//! always reflects the latest in-memory state. Rewriting a small JSON
//! document (≤50 tasks) per mutation is acceptable for a feature that
//! fires at most ~50 times a day.
//!
//! A crash window between an in-memory mutation and a successful
//! `save_all` can cause a one-shot durable task to re-fire **exactly
//! once** after restart — see [`CronScheduler`](crate::CronScheduler) docs.
//!
//! Session-scoped trait + impl live in [`crate::persistence_session`]
//! and [`crate::persistence_file_scoped`]. Scheduler integration
//! (persist_locked / load_persisted) lives in
//! [`crate::scheduler_persistence`].

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::io;

use crate::expression::CronExpression;
use crate::task::ScheduledTask;

/// Current on-disk schema version. Bump if the layout changes
/// incompatibly; `load` tolerates unknown future versions by refusing
/// rather than silently misreading.
pub const SCHEMA_VERSION: u32 = 1;

/// Errors surfaced from a [`SessionScopedCronStorage`](crate::SessionScopedCronStorage)
/// implementation.
#[derive(Debug, thiserror::Error)]
pub enum PersistError {
    #[error("cron storage i/o: {0}")]
    Io(#[from] io::Error),
    #[error("cron storage serde: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("cron expression in stored file is invalid: {0}")]
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

/// Build the durable subset of `tasks` as [`PersistedTask`] entries.
pub(crate) fn durable_snapshot(tasks: &[ScheduledTask]) -> Vec<PersistedTask> {
    tasks
        .iter()
        .filter(|t| t.durable)
        .map(PersistedTask::from_task)
        .collect()
}

// ---------------------------------------------------------------------------
// On-disk codec — used by `FileScopedCronStore`
// ---------------------------------------------------------------------------

/// Result of classifying raw bytes against the [`PersistedFile`] schema.
///
/// The store layer translates each variant into either a returned task
/// list or a quarantine + empty result.
pub(crate) enum LoadedPayload {
    Empty,
    Tasks(Vec<PersistedTask>),
    Quarantine(String),
}

/// Decode raw bytes into a [`LoadedPayload`].
///
/// Empty input is treated as a valid empty list (first-ever-use). Bad
/// JSON or unsupported schema versions yield [`LoadedPayload::Quarantine`]
/// with a human-readable reason for the audit log.
pub(crate) fn classify_payload(bytes: &[u8]) -> LoadedPayload {
    if bytes.is_empty() {
        return LoadedPayload::Empty;
    }
    match serde_json::from_slice::<PersistedFile>(bytes) {
        Ok(parsed) if parsed.version == SCHEMA_VERSION => LoadedPayload::Tasks(parsed.tasks),
        Ok(parsed) => {
            LoadedPayload::Quarantine(format!("unsupported schema version {}", parsed.version))
        }
        Err(e) => LoadedPayload::Quarantine(format!("serde: {e}")),
    }
}

/// Encode `tasks` as pretty JSON wrapped in a current-version [`PersistedFile`].
pub(crate) fn encode_payload(tasks: &[PersistedTask]) -> Result<Vec<u8>, serde_json::Error> {
    let file = PersistedFile {
        version: SCHEMA_VERSION,
        tasks: tasks.to_vec(),
    };
    serde_json::to_vec_pretty(&file)
}
