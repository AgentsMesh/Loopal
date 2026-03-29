use std::str::FromStr;

use chrono::{DateTime, Utc};
use cron::Schedule;

/// Parsed cron expression wrapper.
///
/// Uses the 7-field `cron` crate format internally (seconds + 5-field + year),
/// but accepts standard 5-field input by prepending `0` (seconds) and
/// appending `*` (year).
#[derive(Debug, Clone)]
pub struct CronExpression {
    schedule: Schedule,
    /// Original 5-field expression string for display.
    raw: String,
}

impl CronExpression {
    /// Parse a standard 5-field cron expression (minute hour dom month dow).
    pub fn parse(expr: &str) -> Result<Self, CronParseError> {
        Self::parse_at(expr, Utc::now())
    }

    /// Parse with an explicit reference time (for deterministic testing).
    pub fn parse_at(expr: &str, now: DateTime<Utc>) -> Result<Self, CronParseError> {
        let fields: Vec<&str> = expr.split_whitespace().collect();
        if fields.len() != 5 {
            return Err(CronParseError::InvalidFieldCount(fields.len()));
        }
        // Convert 5-field → 7-field for the `cron` crate: "sec min hour dom mon dow year"
        let seven_field = format!("0 {expr} *");
        let schedule = Schedule::from_str(&seven_field)
            .map_err(|e| CronParseError::ParseFailed(e.to_string()))?;

        // Validate that at least one occurrence exists before the task expires.
        let limit = now + chrono::Duration::seconds(crate::task::MAX_LIFETIME_SECS);
        if schedule.after(&now).next().is_none_or(|t| t > limit) {
            return Err(CronParseError::NoOccurrence);
        }

        Ok(Self {
            schedule,
            raw: expr.to_string(),
        })
    }

    /// Return the next occurrence strictly after `after`.
    pub fn next_after(&self, after: &DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.schedule.after(after).next()
    }

    /// Original 5-field expression string.
    pub fn as_str(&self) -> &str {
        &self.raw
    }
}

impl std::fmt::Display for CronExpression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.raw)
    }
}

/// Errors when parsing a cron expression.
#[derive(Debug, Clone, PartialEq)]
pub enum CronParseError {
    /// Expected exactly 5 fields.
    InvalidFieldCount(usize),
    /// Underlying cron parser error.
    ParseFailed(String),
    /// Expression never matches within the task lifetime (3 days).
    NoOccurrence,
}

impl std::fmt::Display for CronParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFieldCount(n) => {
                write!(f, "expected 5 fields in cron expression, got {n}")
            }
            Self::ParseFailed(msg) => write!(f, "invalid cron expression: {msg}"),
            Self::NoOccurrence => {
                write!(
                    f,
                    "expression has no occurrence within the task lifetime (3 days)"
                )
            }
        }
    }
}

impl std::error::Error for CronParseError {}
