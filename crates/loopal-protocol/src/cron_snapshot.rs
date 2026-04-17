//! Cron job snapshot types for TUI / IPC observation.
//!
//! Defined in protocol so that presentation layers (TUI, ACP) can display
//! scheduled cron jobs without depending on the scheduler-level store.
//!
//! Unix ms is used for timestamps instead of `DateTime<Utc>` so serde
//! round-trips through JSON without requiring chrono's serde feature.
//!
//! ## Field-extension rules
//!
//! [`CronJobSnapshot`] crosses the IPC boundary (agent-server ↔ TUI, ACP).
//! When adding a new field:
//!
//! 1. Annotate it with `#[serde(default)]` so older senders remain
//!    compatible with newer receivers. The test
//!    `missing_new_fields_deserialize_with_defaults` locks this rule.
//! 2. If the field participates in identity (i.e. two snapshots with
//!    different values should be treated as "different jobs"), extend
//!    `CronIdentity::From<&CronJobSnapshot>` in `loopal-agent-server`.
//! 3. If the field is a scalar with no natural "unknown" value, prefer
//!    `Option<T>` over magic numbers (e.g. `0` or `""`).

use serde::{Deserialize, Serialize};

/// Read-only snapshot of a scheduled cron job for panel display.
///
/// Produced by `cron_bridge` from `loopal_scheduler::CronJobInfo` on each
/// poll tick. `PartialEq` enables the bridge's diff-skip optimization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CronJobSnapshot {
    /// 8-char scheduler-generated task ID.
    pub id: String,
    /// Cron expression (5-field form, e.g. `"*/5 * * * *"`). Reserved for
    /// future UI use; the current TUI panel does not display this field.
    #[serde(default)]
    pub cron_expr: String,
    /// Prompt that will be enqueued on fire. Newlines stripped.
    pub prompt: String,
    /// Whether the job repeats or is one-shot.
    pub recurring: bool,
    /// Creation time as Unix milliseconds UTC. `0` indicates a legacy
    /// sender that predates this field; treat it as "unknown" rather
    /// than literal 1970-01-01 in the UI.
    #[serde(default)]
    pub created_at_unix_ms: i64,
    /// Next fire time as Unix milliseconds UTC. `None` when exhausted.
    pub next_fire_unix_ms: Option<i64>,
    /// Whether the job is backed by a durable store and therefore
    /// persists across process restarts. Defaults to `false` for
    /// legacy senders that predate this field.
    #[serde(default)]
    pub durable: bool,
}
