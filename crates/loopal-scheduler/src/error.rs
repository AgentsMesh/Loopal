use crate::expression::CronParseError;

#[derive(Debug, Clone, PartialEq)]
pub enum SchedulerError {
    InvalidCron(CronParseError),
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
