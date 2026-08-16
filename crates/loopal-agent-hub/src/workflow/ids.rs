use loopal_protocol::{WorkflowAttemptCapability, WorkflowAttemptId, WorkflowRunId};

pub trait WorkflowIdSource: Send + Sync + 'static {
    fn next_run_id(&self) -> WorkflowRunId;
    fn next_attempt_id(&self) -> WorkflowAttemptId;
    fn next_attempt_capability(&self) -> WorkflowAttemptCapability;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemWorkflowIdSource;

impl WorkflowIdSource for SystemWorkflowIdSource {
    fn next_run_id(&self) -> WorkflowRunId {
        WorkflowRunId::generate()
    }

    fn next_attempt_id(&self) -> WorkflowAttemptId {
        WorkflowAttemptId::generate()
    }

    fn next_attempt_capability(&self) -> WorkflowAttemptCapability {
        WorkflowAttemptCapability::generate()
    }
}
