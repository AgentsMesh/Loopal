use crate::expression::CronParseError;

/// Scheduler-level errors.
#[derive(Debug, Clone, PartialEq)]
pub enum SchedulerError {
    /// Invalid cron expression.
    InvalidCron(CronParseError),
    /// Task limit reached.
    TooManyTasks(usize),
}

impl std::fmt::Display for SchedulerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCron(e) => write!(f, "{e}"),
            Self::TooManyTasks(max) => {
                write!(f, "maximum number of scheduled tasks ({max}) reached")
            }
        }
    }
}

impl std::error::Error for SchedulerError {}

/// Generate an 8-character random alphanumeric task ID.
pub(crate) fn generate_task_id() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    (0..8)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}
