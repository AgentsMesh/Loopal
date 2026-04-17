use chrono::{DateTime, Timelike, Utc};
use serde::Serialize;

use crate::expression::CronExpression;

/// Maximum lifetime of a scheduled task (3 days).
pub const MAX_LIFETIME_SECS: i64 = 3 * 24 * 60 * 60;

/// A scheduled task managed by [`CronScheduler`](crate::CronScheduler).
pub(crate) struct ScheduledTask {
    pub id: String,
    pub cron: CronExpression,
    pub prompt: String,
    pub recurring: bool,
    pub created_at: DateTime<Utc>,
    pub last_fired: Option<DateTime<Utc>>,
    /// When `true`, mutations to this task are persisted via the
    /// scheduler's [`DurableStore`](crate::persistence::DurableStore)
    /// so it survives across process restarts. Non-durable tasks live
    /// only in memory.
    pub durable: bool,
}

/// Truncate a timestamp to whole seconds to avoid sub-second precision issues
/// with the `cron` crate, which operates at second-level granularity.
pub(crate) fn truncate_to_secs(dt: DateTime<Utc>) -> DateTime<Utc> {
    dt.with_nanosecond(0).unwrap_or(dt)
}

impl ScheduledTask {
    /// Check whether the task should fire at `now`.
    ///
    /// Returns `true` when the next cron occurrence after `last_fired`
    /// (or `created_at` if never fired) is at or before `now`.
    pub fn should_fire(&self, now: &DateTime<Utc>) -> bool {
        let reference = truncate_to_secs(self.last_fired.unwrap_or(self.created_at));
        self.cron
            .next_after(&reference)
            .is_some_and(|next| next <= *now)
    }

    /// Whether this task has aged past its maximum lifetime.
    ///
    /// Durable tasks (backed by a `DurableStore`) are exempt from the
    /// lifetime cap: users who explicitly opt into persistence expect
    /// their schedules to survive indefinitely across restarts.
    /// Non-durable (in-memory) tasks still expire after
    /// [`MAX_LIFETIME_SECS`] to keep ephemeral entries from piling up.
    pub fn is_expired(&self, now: &DateTime<Utc>) -> bool {
        if self.durable {
            return false;
        }
        now.signed_duration_since(self.created_at).num_seconds() > MAX_LIFETIME_SECS
    }
}

/// Read-only snapshot of a scheduled cron job for listing.
#[derive(Debug, Clone, Serialize)]
pub struct CronJobInfo {
    pub id: String,
    pub cron_expr: String,
    pub prompt: String,
    pub recurring: bool,
    pub created_at: DateTime<Utc>,
    pub next_fire: Option<DateTime<Utc>>,
    /// Whether this task's state is backed by a [`DurableStore`](crate::persistence::DurableStore)
    /// and therefore persists across session restarts.
    pub durable: bool,
}
