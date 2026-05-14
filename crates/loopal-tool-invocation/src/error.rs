use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TransitionError {
    #[error("invalid transition from {state} via {cmd}")]
    Invalid {
        state: &'static str,
        cmd: &'static str,
    },
}
