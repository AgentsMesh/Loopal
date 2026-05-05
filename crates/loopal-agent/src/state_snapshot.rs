//! Aggregating snapshot of agent-process observable state for IPC.
//!
//! Hub uses `agent/state_snapshot` to seed `ViewState` on cold start
//! (process restart, agent reconnect, session resume). The reducer then
//! applies the live event stream on top, so this snapshot only needs the
//! state classes that are not derivable from events alone — task list,
//! cron jobs, and running background processes.
//!
//! Token usage / mode / status are excluded on purpose: those are
//! accumulated by the Hub-side reducer from the agent's event stream and
//! would only duplicate fact here.
//!
//! Conversion helpers are public so `task_bridge` / `cron_bridge` and any
//! future observers share one definition (single source of truth).

use loopal_protocol::{CronJobSnapshot, TaskSnapshot, TaskSnapshotStatus};
use loopal_scheduler::CronJobInfo;

use crate::types::{Task, TaskStatus};

/// Convert an agent-internal `Task` to a protocol-level `TaskSnapshot`.
///
/// Newlines are stripped so the snapshot survives line-oriented UIs
/// (panels, log lines) without rendering artifacts.
pub fn task_to_snapshot(task: &Task) -> TaskSnapshot {
    let status = match task.status {
        TaskStatus::Pending => TaskSnapshotStatus::Pending,
        TaskStatus::InProgress => TaskSnapshotStatus::InProgress,
        TaskStatus::Completed | TaskStatus::Deleted => TaskSnapshotStatus::Completed,
    };
    TaskSnapshot {
        id: task.id.clone(),
        subject: task.subject.replace('\n', " ").replace('\r', ""),
        active_form: task.active_form.clone(),
        status,
        blocked_by: task.blocked_by.clone(),
    }
}

/// Convert a scheduler-internal `CronJobInfo` to a protocol-level `CronJobSnapshot`.
pub fn cron_info_to_snapshot(info: CronJobInfo) -> CronJobSnapshot {
    CronJobSnapshot {
        id: info.id,
        cron_expr: info.cron_expr,
        prompt: info.prompt.replace('\n', " ").replace('\r', ""),
        recurring: info.recurring,
        created_at_unix_ms: info.created_at.timestamp_millis(),
        next_fire_unix_ms: info.next_fire.map(|t| t.timestamp_millis()),
        durable: info.durable,
    }
}
