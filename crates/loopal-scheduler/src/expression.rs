use std::str::FromStr;

use chrono::{DateTime, Utc};
use cron::Schedule;

/// Standard 5-field cron expression. Stored as the 7-field form
/// (`sec min hour dom mon dow year`) the `cron` crate expects.
#[derive(Debug, Clone)]
pub struct CronExpression {
    schedule: Schedule,
    raw: String,
}

impl CronExpression {
    pub fn parse(expr: &str) -> Result<Self, CronParseError> {
        Self::parse_at(expr, Utc::now())
    }

    pub fn parse_at(expr: &str, now: DateTime<Utc>) -> Result<Self, CronParseError> {
        let fields: Vec<&str> = expr.split_whitespace().collect();
        if fields.len() != 5 {
            return Err(CronParseError::InvalidFieldCount(fields.len()));
        }
        let seven_field = format!("0 {expr} *");
        let schedule = Schedule::from_str(&seven_field)
            .map_err(|e| CronParseError::ParseFailed(e.to_string()))?;

        if schedule.after(&now).next().is_none() {
            return Err(CronParseError::NoOccurrence);
        }

        Ok(Self {
            schedule,
            raw: expr.to_string(),
        })
    }

    pub fn next_after(&self, after: &DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.schedule.after(after).next()
    }

    pub fn as_str(&self) -> &str {
        &self.raw
    }
}

impl std::fmt::Display for CronExpression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.raw)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CronParseError {
    InvalidFieldCount(usize),
    ParseFailed(String),
    /// Expression has no future occurrence (e.g. February 30).
    NoOccurrence,
}

impl std::fmt::Display for CronParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFieldCount(n) => {
                write!(f, "expected 5 fields in cron expression, got {n}")
            }
            Self::ParseFailed(msg) => write!(f, "invalid cron expression: {msg}"),
            Self::NoOccurrence => write!(f, "cron expression will never fire"),
        }
    }
}

impl std::error::Error for CronParseError {}
