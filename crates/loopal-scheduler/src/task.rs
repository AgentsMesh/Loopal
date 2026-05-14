use chrono::{DateTime, Timelike, Utc};
use serde::Serialize;

use crate::expression::CronExpression;

pub(crate) struct ScheduledTask {
    pub id: String,
    pub cron: CronExpression,
    pub prompt: String,
    pub recurring: bool,
    pub created_at: DateTime<Utc>,
    pub last_fired: Option<DateTime<Utc>>,
    /// When `true`, mutations to this task are persisted via the
    /// session-scoped store and survive process restarts.
    pub durable: bool,
}

pub(crate) fn truncate_to_secs(dt: DateTime<Utc>) -> DateTime<Utc> {
    dt.with_nanosecond(0).unwrap_or(dt)
}

impl ScheduledTask {
    pub fn next_fire(&self) -> Option<DateTime<Utc>> {
        let reference = truncate_to_secs(self.last_fired.unwrap_or(self.created_at));
        self.cron.next_after(&reference)
    }

    pub fn should_fire(&self, now: &DateTime<Utc>) -> bool {
        self.next_fire().is_some_and(|next| next <= *now)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CronJobInfo {
    pub id: String,
    pub cron_expr: String,
    pub prompt: String,
    pub recurring: bool,
    pub created_at: DateTime<Utc>,
    pub next_fire: Option<DateTime<Utc>>,
    pub durable: bool,
}
