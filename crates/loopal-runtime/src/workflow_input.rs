use std::future::Future;
use std::pin::Pin;

use loopal_protocol::Envelope;

/// Result of classifying a root human envelope for optional orchestration.
/// `Handled` means the callback has committed the input to an external
/// authority and the normal agent turn must not be started for this envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowInputDisposition {
    Direct,
    Handled,
}

pub trait WorkflowInputHandler: Send + Sync {
    fn handle<'a>(
        &'a self,
        envelope: &'a Envelope,
        recent_context: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<WorkflowInputDisposition, String>> + Send + 'a>>;
}
