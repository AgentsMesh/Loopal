//! A crash window between an in-memory mutation and a successful
//! `save_all` can cause a one-shot durable task to re-fire **exactly
//! once** after restart — see `CronScheduler` docs.

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::io;

use crate::expression::CronExpression;
use crate::task::ScheduledTask;

/// On-disk schema version. Bump if the layout changes incompatibly;
/// `load` refuses unknown future versions rather than misreading.
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum PersistError {
    #[error("cron storage i/o: {0}")]
    Io(#[from] io::Error),
    #[error("cron storage serde: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("cron expression in stored file is invalid: {0}")]
    BadCron(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct PersistedTask {
    pub id: String,
    pub cron: String,
    pub prompt: String,
    pub recurring: bool,
    pub created_at_unix_ms: i64,
    pub last_fired_unix_ms: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PersistedFile {
    pub(crate) version: u32,
    pub(crate) tasks: Vec<PersistedTask>,
}

impl PersistedTask {
    /// Caller ensures `durable == true`.
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

    /// `parse_reference` must be "now", not the persisted `created_at`,
    /// so the `NoOccurrence` check sees the same forward window the
    /// live `add()` path does. Rejects entries with an empty `id` so
    /// missing-required-field cases land in the drop-on-load filter
    /// rather than silently surfacing as a zero-id task.
    pub(crate) fn into_task(
        self,
        parse_reference: DateTime<Utc>,
    ) -> Result<ScheduledTask, PersistError> {
        if self.id.is_empty() {
            return Err(PersistError::BadCron("missing task id".into()));
        }
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

pub(crate) fn durable_snapshot(tasks: &[ScheduledTask]) -> Vec<PersistedTask> {
    tasks
        .iter()
        .filter(|t| t.durable)
        .map(PersistedTask::from_task)
        .collect()
}

pub(crate) enum LoadedPayload {
    Empty,
    Tasks(Vec<PersistedTask>),
    Quarantine(String),
}

/// Empty input is treated as a valid empty list (first-ever-use). Bad
/// JSON or unsupported schema versions yield `Quarantine` with a
/// human-readable reason.
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

pub(crate) fn encode_payload(tasks: &[PersistedTask]) -> Result<Vec<u8>, serde_json::Error> {
    let file = PersistedFile {
        version: SCHEMA_VERSION,
        tasks: tasks.to_vec(),
    };
    serde_json::to_vec_pretty(&file)
}
